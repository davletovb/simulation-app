from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import Union, Callable, List, Dict
import asyncio
import copy
import json
import random
import logging

from .database import SimulationState, SessionLocal
from .assistant import Assistant

class ParameterType(Enum):
    INTEGER = "integer"
    FLOAT = "float"
    BOOLEAN = "boolean"
    STRING = "string"

class ActionType(Enum):
    INCREASE = "increase"
    DECREASE = "decrease"
    SET = "set"
    TOGGLE = "toggle"
    SCALE = "scale"
    RESET = "reset"
    RANDOMIZE = "randomize"

class Parameter:
    def __init__(self, name: str, value: Union[int, float, bool, str], type: ParameterType, initial_value: Union[int, float, bool, str], dependencies: List = None, update_rule: Callable = None):
        
        if not isinstance(value, type.value):
            self.logger.error(f"Invalid value: {value} is not of type {type}")
            raise ValueError(f"Invalid value: {value} is not of type {type}")
        self.name = name
        self.value = value
        self.type = type
        self.initial_value = initial_value
        self.dependencies = dependencies if dependencies else []
        self.update_rule = update_rule
        self.logger = logging.getLogger(__name__)

    def update_value(self, decision):
        if self.update_rule:
            try:
                self.value = self.update_rule(self.value, decision)
            except Exception as e:
                self.logger.error(f"Error applying update rule: {e}")
                raise SimulationError(f"Error applying update rule: {e}")
        else:
            try:
                if decision.action == ActionType.INCREASE:
                    self.value += 1
                elif decision.action == ActionType.DECREASE:
                    self.value -= 1
                elif decision.action == ActionType.SET:
                    self.value = decision.value
                elif decision.action == ActionType.TOGGLE:
                    if self.type != ParameterType.BOOLEAN:
                        raise ValueError("Invalid action: 'toggle' can only be applied to boolean parameters")
                    self.value = not self.value
                elif decision.action == ActionType.SCALE:
                    self.value *= decision.value
                elif decision.action == ActionType.RESET:
                    self.value = self.initial_value
                elif decision.action == ActionType.RANDOMIZE:
                    self.value = random.uniform(decision.value[0], decision.value[1])
                else:
                    raise ValueError(f"Invalid action: {decision.action}")
            except Exception as e:
                self.logger.error(f"Error updating parameter: {e}")
                raise SimulationError(f"Error updating parameter: {e}")
        
        # Adjust the value based on the values of the dependencies
        for dependency in self.dependencies:
            self.value += dependency.value

    def get_dependencies(self):
        return self.dependencies


class State(ABC):
    def __init__(self):
        self.parameters = {
            "primary": {},
            "secondary": {},
            "tertiary": {}
        }

    def get_value(self):
        return {
            "parameters": {
                "primary": [param.value for param in self.parameters["primary"].values()],
                "secondary": [param.value for param in self.parameters["secondary"].values()],
                "tertiary": [param.value for param in self.parameters["tertiary"].values()]
            }
        }

    def set_value(self, state):
        for category in ["primary", "secondary", "tertiary"]:
            for param, value in zip(self.parameters[category].values(), state["parameters"][category]):
                param.value = value

    def to_json(self):
        return json.dumps(self.get_value())

    @classmethod
    def from_json(cls, json_str):
        state = cls()
        state.set_value(json.loads(json_str))
        return state

class Transition(ABC):
    @abstractmethod
    async def apply(self, state: State) -> State:
        pass

    def to_dict(self):
        return {
            "class": self.__class__.__name__,
            # Include any other attributes that should be serialized here.
            # For example, if the Transition has an 'action' attribute, you could include it like this:
            # "action": self.action,
        }

class ParameterTransition(Transition):
    def __init__(self, changes: Dict[str, float]):
        self.changes = changes

    async def apply(self, state: State) -> State:
        new_state = copy.deepcopy(state)  # Create a copy of the current state
        for parameter_name, change in self.changes.items():
            # Get the current value of the parameter
            current_value = new_state.get_value()[parameter_name]
            # Update the value of the parameter in the new state
            new_state.set_value(parameter_name, current_value + change)
        return new_state  # Return the new state

class SimulationError(Exception):
    pass

class Simulation:
    def __init__(self, initial_state: State, assistant: Assistant):
        self.current_state = initial_state
        self.assistant = assistant
        self.running = False
        self.logger = logging.getLogger(__name__)
    
    async def start_simulation(self, saved_state_id=None):
        self.running = True
        if saved_state_id is not None:
            # Load the saved state
            await self.load_state(saved_state_id)
        else:
            # Load the default state
            self.current_state = self.initial_state
        # The simulation is now ready to start running

    async def stop_simulation(self):
        self.running = False
    
    async def get_state(self):
        self.current_state.get_value()

    async def update_state(self, transition: Transition):
        try:
            self.current_state = await transition.apply(self.current_state)
        except Exception as e:
            self.logger.error(f"Error applying transition: {e}")
            raise SimulationError(f"Error applying transition: {e}")

    async def save_state(self):
        try:
            db = SessionLocal()
            state_dict = {
                "simulation_state": self.current_state.get_value(),
                "assistant_state": self.assistant.get_state(),
            }
            state_record = SimulationState(state=state_dict)
            db.add(state_record)
            await db.commit()
        except Exception as e:
            self.logger.error(f"Error saving state: {e}")
            raise SimulationError(f"Error saving state: {e}")

    async def load_state(self, id: int):
        try:
            db = SessionLocal()
            state_record = await db.query(SimulationState).get(id)
            state_dict = state_record.get_state()
            self.current_state.set_value(state_dict["simulation_state"])
            self.assistant.set_state(state_dict["assistant_state"])
        except Exception as e:
            self.logger.error(f"Error loading state: {e}")
            raise SimulationError(f"Error loading state: {e}")