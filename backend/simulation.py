from abc import ABC, abstractmethod

import json
from dataclasses import dataclass
from enum import Enum
import random

from .database import Base, SimulationState, SessionLocal
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

@dataclass
class Parameter:
    value: float
    type: ParameterType
    initial_value: float
    dependencies: list = None
    update_rule: callable = None

    def update(self, decision):
        if self.update_rule:
            self.value = self.update_rule(self.value, decision)
        else:
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

    def get_dependencies(self):
        return self.dependencies


class State(ABC):
    @abstractmethod
    def get_value(self):
        pass

    def to_json(self):
        return json.dumps(self.__dict__)

    @classmethod
    def from_json(cls, json_str):
        return cls(**json.loads(json_str))

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

class SimulationError(Exception):
    pass

class Simulation:
    def __init__(self, initial_state: State, assistant: Assistant):
        self.current_state = initial_state
        self.assistant = assistant

    async def update(self, transition: Transition):
        self.current_state = await transition.apply(self.current_state)
        await self.assistant.remember("transitions", transition)

    async def interact_with_assistant(self, command):
        response = await self.assistant.process_command(command)
        return response
    
    def get_state(self):
        return {
            "primary_parameters": [param.value for param in self.primary_parameters.values()],
            "secondary_parameters": [param.value for param in self.secondary_parameters.values()],
            "tertiary_parameters": [param.value for param in self.tertiary_parameters.values()],
            "assistant_state": self.assistant.get_state(),
        }

    def set_state(self, state):
        for param, value in zip(self.primary_parameters.values(), state["primary_parameters"]):
            param.value = value
        for param, value in zip(self.secondary_parameters.values(), state["secondary_parameters"]):
            param.value = value
        for param, value in zip(self.tertiary_parameters.values(), state["tertiary_parameters"]):
            param.value = value
        self.assistant.set_state(state["assistant_state"])

    async def save_state(self):
        db = SessionLocal()
        state_json = self.current_state.to_json()
        state_record = SimulationState(state_json=state_json)
        db.add(state_record)
        await db.commit()

    async def load_state(self, id: int):
        db = SessionLocal()
        state_record = await db.query(SimulationState).get(id)
        state_dict = json.loads(state_record.state_json)
        self.current_state = State.from_json(state_dict)
    

