from database import SessionLocal
from assistant import Assistant
from metrics import Metric

from enum import Enum
from typing import Union, List, Dict
import json
import logging
import pickle

class ParameterValueType(Enum):
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

class ParameterType(Enum):
    PRIMARY = 1
    SECONDARY = 2
    TERTIARY = 3

class Parameter:
    def __init__(self, name: str, initial_value: float, parameter_type: ParameterType, dependencies: List = None):
        self.name = name
        self.value = initial_value
        self.parameter_type = parameter_type
        self.dependencies = dependencies if dependencies else []

    def update_value(self, decisions: List):
        """Update the value of the parameter based on the effects of all decisions."""
        for decision in decisions:
            self.value += decision.effect

    def adjust_for_dependency(self, parameters):
        """Adjust the value of the parameter based on the values of its dependencies."""
        if self.dependencies:
            total_dependency_value = sum(parameters[dep_name].value for dep_name in self.dependencies)
            average_dependency_value = total_dependency_value / len(self.dependencies)
            self.value += average_dependency_value
        
    def to_dict(self):
        return {"name": self.name, "value": self.value}

class Decision:
    def __init__(self, name: str, effects: Dict[Parameter, float], economic_cost: int, influence_cost: int):
        self.name = name
        self.effects = effects
        self.economic_cost = economic_cost
        self.influence_cost = influence_cost
    
    def to_dict(self):
        return {"name": self.name, "economic_cost": self.economic_cost, "influence_cost": self.influence_cost}

class Minister:
    def __init__(self, name: str, loyalty: float, influence: float):
        self.name = name
        self.loyalty = loyalty
        self.influence = influence
    
    def to_dict(self):
        return {"name": self.name, "loyalty": self.loyalty, "influence": self.influence}

class CitizenGroup:
    def __init__(self, name: str, size: int, political_view: str):
        self.name = name
        self.size = size
        self.political_view = political_view
    
    def to_dict(self):
        return {"name": self.name, "size": self.size, "political_view": self.political_view}

class EconomicSector:
    def __init__(self, name: str, importance: float):
        self.name = name
        self.importance = importance
    
    def to_dict(self):
        return {"name": self.name, "importance": self.importance}

class Narrative:
    def __init__(self, name: str, effects: dict):
        self.name = name
        self.effects = effects
    
    def to_dict(self):
        return {"name": self.name}

"""
Narrative class will be changed to this definition later
class Narrative2:
    def __init__(self, name: str, initial_parameters: Dict[str, Parameter], initial_decisions: Dict[str, Decision], initial_ministers: Dict[str, Minister], initial_citizen_groups: Dict[str, CitizenGroup], initial_economic_sectors: Dict[str, EconomicSector]):
        self.name = name
        self.initial_parameters = initial_parameters
        self.initial_decisions = initial_decisions
        self.initial_ministers = initial_ministers
        self.initial_citizen_groups = initial_citizen_groups
        self.initial_economic_sectors = initial_economic_sectors

    def start(self):
        # Create a state with the initial entities defined by this narrative
        return State(parameters=self.initial_parameters, decisions=self.initial_decisions, ministers=self.initial_ministers, citizen_groups=self.initial_citizen_groups, economic_sectors=self.initial_economic_sectors)
"""    

class State:
    def __init__(self, parameters: Dict[str, Parameter] = None, decisions: Dict[str, Decision] = None, ministers: Dict[str, Minister] = None, citizen_groups: Dict[str, CitizenGroup] = None, economic_sectors: Dict[str, EconomicSector] = None, metrics: Dict[str, float] = None, assistant: Assistant = None, narrative: Narrative = None):
        self.parameters = parameters if parameters else {}
        self.decisions = decisions if decisions else {}
        self.ministers = ministers if ministers else {}
        self.citizen_groups = citizen_groups if citizen_groups else {}
        self.economic_sectors = economic_sectors if economic_sectors else {}
        self.metrics = metrics if metrics else {}
        self.influence = 0  # Player's influence, to be increased by negotiating with ministers
        self.assistant = assistant
        self.narrative = narrative

    def set_parameters(self, parameters: dict):
        self.parameters = parameters

    def set_decisions(self, decisions: dict):
        self.decisions = decisions
    
    def set_citizen_groups(self, citizen_groups: dict):
        self.citizen_groups = citizen_groups
    
    def set_ministers(self, ministers: dict):
        self.ministers = ministers

    def set_economic_sectors(self, economic_sectors: dict):
        self.economic_sectors = economic_sectors
    
    def set_narrative(self, narrative):
        self.narrative = narrative
        # Apply narrative effects to the game state
        for parameter_name, effect in narrative.effects.items():
            self.parameters[parameter_name].value += effect
    
    def set_metrics(self, metrics: dict):
        self.metrics = metrics
    
    def get_metrics(self):
        return self.metrics

    def get_parameter(self, name: str):
        return self.parameters.get(name)

    def get_decision(self, name: str):
        return self.decisions.get(name)
    
    def apply_decision(self, decision):
        # Decrease the influence score by the decision's influence cost
        self.influence -= decision.influence_cost

        # Apply the effects of the decision to the relevant parameters
        for parameter, effect in decision.effects.items():
            self.parameters[parameter.name].value += effect
            self.parameters[parameter.name].adjust_for_dependency(self.parameters)  # adjust for dependency immediately after the decision, no delay

        """
        # It may also have effects on ministers, citizen groups, etc.
        # For example:
        for minister, effect in decision.minister_effects.items():
            self.ministers[minister].status += effect
        for citizen_group, effect in decision.citizen_group_effects.items():
            self.citizen_groups[citizen_group].opinion += effect
        """

    def negotiate_with_minister(self, minister_name: str):
        minister = self.ministers.get(minister_name)
        if minister is not None:
            self.influence += minister.loyalty * 10
        else:
            print(f"No minister named '{minister_name}' exists.")

    def make_public_statement(self, citizen_group_name: str, statement: str):
        citizen_group = self.citizen_groups.get(citizen_group_name)
        if citizen_group is not None:
            self.influence += len(statement) / 100
        else:
            print(f"No citizen group named '{citizen_group_name}' exists.")
    
    def ask_assistant(self):
        print(self.assistant.generate_response(self))

    def save_game(self, file_path: str):
        with open(file_path, 'wb') as f:
            pickle.dump(self, f)

    @staticmethod
    def load_game(file_path: str):
        with open(file_path, 'rb') as f:
            return pickle.load(f)
    
    def to_dict(self):
        # Create a dictionary with the same attributes as the State object
        state_dict = self.__dict__.copy()

        # If State object contains other custom objects, it will need to convert these to dictionaries as well
        # For example, if State object contains an Assistant object:
        if hasattr(self, 'assistant') and self.assistant is not None:
            state_dict['assistant'] = self.assistant.to_dict()

        return state_dict
        
    @classmethod
    def from_dict(cls, state_dict):
        # create a new instance of the class
        state = cls()

        # populate the attributes of the state object from the dictionary
        for key, value in state_dict.items():
            if key == 'assistant':
                # create an Assistant object from the dictionary
                state.assistant = Assistant.from_dict(value)
            else:
                setattr(state, key, value)

        return state
    