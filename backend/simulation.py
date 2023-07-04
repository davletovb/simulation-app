from abc import ABC, abstractmethod
from enum import Enum
from typing import Union, List, Dict
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
    CHANGE = "change"
    SET = "set"
    TOGGLE = "toggle"
    SCALE = "scale"
    RESET = "reset"
    RANDOMIZE = "randomize"
    CHANGE_PERCENTAGE = "change_percentage"
    COMPLEX = "complex"

class Decision:
    def __init__(self, action: ActionType, value: Union[int, float, bool, str, dict], priority: int = 1):
        self.action = action
        self.value = value
        self.priority = priority

        # Validate the decision
        self.validate()

    def validate(self):
        # Check that the action is a valid ActionType
        if not isinstance(self.action, ActionType):
            raise ValueError(f"Invalid action: {self.action}")

        # Check that the value is appropriate for the action
        if self.action in [ActionType.CHANGE, ActionType.CHANGE_PERCENTAGE, ActionType.SCALE] and not isinstance(self.value, (int, float)):
            raise ValueError(f"Invalid value for {self.action}: {self.value}. Value must be an integer or float.")
        elif self.action == ActionType.SET and not isinstance(self.value, (int, float, bool, str)):
            raise ValueError(f"Invalid value for {self.action}: {self.value}. Value must be an integer, float, boolean, or string.")
        elif self.action == ActionType.TOGGLE and not isinstance(self.value, bool):
            raise ValueError(f"Invalid value for {self.action}: {self.value}. Value must be a boolean.")
        elif self.action == ActionType.COMPLEX and not isinstance(self.value, dict):
            raise ValueError(f"Invalid value for {self.action}: {self.value}. Value must be a dictionary for complex decisions.")

        # Check that the priority is a positive integer
        if not isinstance(self.priority, int) or self.priority <= 0:
            raise ValueError(f"Invalid priority: {self.priority}. Priority must be a positive integer.")

    def __repr__(self):
        return f"Decision(action={self.action}, value={self.value}, priority={self.priority})"


class Parameter:
    def __init__(self, name: str, value: Union[int, float, bool, str], type: ParameterType, initial_value: Union[int, float, bool, str], dependencies: List = None):
        
        if not isinstance(value, type.value):
            self.logger.error(f"Invalid value: {value} is not of type {type}")
            raise ValueError(f"Invalid value: {value} is not of type {type}")
        self.name = name
        self.value = value
        self.type = type
        self.initial_value = initial_value
        self.dependencies = dependencies if dependencies else []
        self.logger = logging.getLogger(__name__)

    def get_value(self):
        return self.value

    def update_value(self, decision):
        # Use the update rule to update the value
        try:
            self.value = self.update_rule(self.value, decision)
        except Exception as e:
            self.logger.error(f"Error applying update rule: {e}")
            raise SimulationError(f"Error applying update rule: {e}")

        # Adjust the value based on the values of the dependencies
        for dependency in self.dependencies:
            self.value = self.adjust_for_dependency(self.value, dependency.value)

    def update_rule(self, value, decision):
        if decision.action == ActionType.CHANGE:
            return value + decision.value
        elif decision.action == ActionType.SET:
            return decision.value
        elif decision.action == ActionType.TOGGLE:
            return not value
        elif decision.action == ActionType.SCALE:
            return value * decision.value
        elif decision.action == ActionType.RESET:
            return self.initial_value
        elif decision.action == ActionType.RANDOMIZE:
            return random.uniform(decision.value[0], decision.value[1])
        elif decision.action == ActionType.CHANGE_PERCENTAGE:
            return value * (1 + decision.value / 100)
        else:
            raise ValueError(f"Invalid action: {decision.action}")

    def adjust_for_dependency(self, value, dependency_value):
        # Adjust the value based on the value of a dependency
        # This is just an example. The exact adjustment will depend on the rules of your simulation.
        return value + dependency_value
    
    def get_dependencies(self):
        return self.dependencies
    
    def __repr__(self):
        return f"Parameter(name={self.name}, value={self.value}, type={self.type})"

class State:
    def __init__(self):
        self.parameters = {
            "primary": {},
            "secondary": {},
            "tertiary": {}
        }

    def get_value(self):
        return {
            "parameters": {
                "primary": {name: param.value for name, param in self.parameters["primary"].items()},
                "secondary": {name: param.value for name, param in self.parameters["secondary"].items()},
                "tertiary": {name: param.value for name, param in self.parameters["tertiary"].items()}
            }
        }

    def set_value(self, state):
        for category in ["primary", "secondary", "tertiary"]:
            for name, value in state["parameters"][category].items():
                self.parameters[category][name].value = value
    
    def update_value(self, decision):
        for category in ["primary", "secondary", "tertiary"]:
            for name, param in self.parameters[category].items():
                param.update_value(decision)

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