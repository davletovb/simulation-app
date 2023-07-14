from assistant import Assistant
from metrics import Metric

from enum import Enum
from typing import Union, List, Dict
import json
import logging
import pickle
import uuid

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
        return {
            'name': self.name,
            'value': self.value,
            'parameter_type': self.parameter_type.value,
            'dependencies': self.dependencies
        }

class Decision:
    def __init__(self, name: str, effects: Dict[Parameter, float], economic_cost: int, influence_cost: int):
        self.name = name
        self.effects = effects
        self.economic_cost = economic_cost
        self.influence_cost = influence_cost
    
    def to_dict(self):
        return {
            'name': self.name,
            'effects': {param.name: effect for param, effect in self.effects.items()},  # Convert Parameter objects to their names (strings)
            'economic_cost': self.economic_cost,
            'influence_cost': self.influence_cost
        }

class Minister:
    def __init__(self, title: str, personal_name: str, loyalty: float, influence: float, backstory: str):
        self.title = title
        self.personal_name = personal_name
        self.loyalty = loyalty
        self.influence = influence
        self.backstory = backstory
    
    def to_dict(self):
        return {"title": self.title, 
                "personal_name": self.personal_name, 
                "loyalty": self.loyalty, 
                "influence": self.influence, 
                "backstory": self.backstory}

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
    def __init__(self, name: str, description: str, effects: dict):
        self.name = name
        self.description = description
        self.effects = effects

    def to_dict(self):
        return {"name": self.name, 
                "description": self.description,
                "effects": self.effects.copy()}

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
    def __init__(self, parameters: Dict[str, Parameter] = None, decisions: Dict[str, Decision] = None, ministers: Dict[str, Minister] = None, citizen_groups: Dict[str, CitizenGroup] = None, economic_sectors: Dict[str, EconomicSector] = None, metrics: Dict[str, float] = None, country: str = None, assistant: Assistant = None, narrative: Narrative = None):
        self.id = str(uuid.uuid4())  # generate a unique ID for each simulation
        self.parameters = parameters if parameters else {}
        self.decisions = decisions if decisions else {}
        self.ministers = ministers if ministers else {}
        self.citizen_groups = citizen_groups if citizen_groups else {}
        self.economic_sectors = economic_sectors if economic_sectors else {}
        self.metrics = metrics if metrics else {}
        self.influence = 1000  # Player's influence, to be increased by negotiating with ministers
        self.assistant = assistant
        self.narrative = narrative
        self.country = country
        self.cycle = 0  # start at cycle 0
        self.decisions_to_apply = []
        self.changes = {}  # dictionary to keep track of changes
    
    def next_cycle(self):
        # Apply the decisions
        for decision in self.decisions_to_apply:
            self.apply_decision(decision)

        # Increment the cycle number
        self.cycle += 1

        # Save the changes and clear them
        changes = self.changes.copy()
        self.changes.clear()

        # Clear the decisions to apply
        self.decisions_to_apply.clear()

        # Return the changes
        return changes
        
        """
        # Update any other state information based on the game rules
        # This could involve calling methods on the other objects in the state
        # For example, you might have a method on the Minister class that adjusts their loyalty based on the decisions made
        for minister in self.ministers.values():
            minister.update_loyalty(self)

        # You could also update the influence, economic state, or other metrics here
        self.update_influence()
        self.update_economic_state()
        """

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
            # Before updating the value, store the change in self.changes
            old_value = self.parameters[parameter.name].value
            self.parameters[parameter.name].value += effect
            new_value = self.parameters[parameter.name].value
            self.changes[parameter.name] = new_value - old_value

            self.parameters[parameter.name].adjust_for_dependency(self.parameters)  # adjust for dependency immediately after the decision

        """
        # It may also have effects on ministers, citizen groups, etc.
        # For example:
        for minister, effect in decision.minister_effects.items():
            self.ministers[minister].status += effect
        for citizen_group, effect in decision.citizen_group_effects.items():
            self.citizen_groups[citizen_group].opinion += effect
        """
    
    def add_decision_to_apply(self, decision: Decision):
        self.decisions_to_apply.append(decision)

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
    
    def get_state(self):
        # Create a dictionary with the same attributes as the State object
        state_dict = self.__dict__.copy()
        # Convert custom objects to dictionaries
        state_dict["parameters"] = {name: param.to_dict() for name, param in self.parameters.items()}
        state_dict["decisions"] = {name: decision.to_dict() for name, decision in self.decisions.items()}
        state_dict["ministers"] = {name: minister.to_dict() for name, minister in self.ministers.items()}
        state_dict["citizen_groups"] = {name: group.to_dict() for name, group in self.citizen_groups.items()}
        state_dict["economic_sectors"] = {name: sector.to_dict() for name, sector in self.economic_sectors.items()}
        state_dict["assistant"] = self.assistant.to_dict()
        state_dict["narrative"] = self.narrative.to_dict()

        # Do the same for other custom objects in the state
        return state_dict

    @staticmethod
    def load_game(file_path: str):
        with open(file_path, 'rb') as f:
            return pickle.load(f)
    
    def to_dict(self):
        # Create a dictionary with the same attributes as the State object
        state_dict = self.__dict__.copy()

        # If State object contains other custom objects, convert these to dictionaries as well
        for attr, value in state_dict.items():
            if isinstance(value, dict):
                # Convert the values of the dictionary to dictionaries if they are custom objects
                # If they are not objects, keep the original value
                state_dict[attr] = {k: v.to_dict() if hasattr(v, 'to_dict') else v for k, v in value.items()}
            elif hasattr(value, 'to_dict'):
                # If the value is a custom object, convert it to a dictionary
                state_dict[attr] = value.to_dict()

        return state_dict
        
    @classmethod
    def from_dict(cls, state_dict):
        # create a new instance of the class
        state = cls()

        # populate the attributes of the state object from the dictionary
        for key, value in state_dict.items():
            if key == 'assistant':
                state.assistant = Assistant.from_dict(value)
            if key == "parameters":
                state.parameters = {name: Parameter.from_dict(param_dict) for name, param_dict in value.items()}
            elif key == "decisions":
                state.decisions = {name: Decision.from_dict(decision_dict) for name, decision_dict in value.items()}
            if key == "ministers":
                state.ministers = {name: Minister.from_dict(minister_dict) for name, minister_dict in value.items()}
            if key == "citizen_groups":
                state.citizen_groups = {name: CitizenGroup.from_dict(group_dict) for name, group_dict in value.items()}
            if key == "economic_sectors":       
                state.economic_sectors = {name: EconomicSector.from_dict(sector_dict) for name, sector_dict in value.items()}
            if key == "narrative":         
                state.narrative = Narrative.from_dict(value)
            if key == "cycle": 
                state.cycle = value
            if key == "country":
                state.country = value
            else:
                setattr(state, key, value)

        return state
    