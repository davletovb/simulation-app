from simulation import State, Parameter, ParameterType, Narrative, Decision, Minister, CitizenGroup, EconomicSector
from metrics import set_metrics_values, update_metrics_values
from database import DatabaseManager
from assistant import Assistant
import logging
import json


class SimulationController:
    def __init__(self):
        self.assistant = None
        self.state = None
        self.narrative = None
        self.country = None
        self.db_manager = DatabaseManager('simulation.db')  # specify the name of database
    
    def start_simulation(self):
        # Check if an assistant has been set
        if self.assistant is None:
            raise ValueError("An assistant must be assigned before starting the game")

        # start a new game, by initializing or resetting the state.
        self.state = State(assistant=self.assistant)

        # Load all default entities into the state
        self.load_parameters("data/parameters.json")
        self.load_ministers("data/ministers.json")
        self.load_decisions("data/decisions.json")
        self.load_citizen_groups("data/citizen_groups.json")
        self.load_economic_sectors("data/economic_sectors.json")
        self.load_metrics()

        # If a narrative has been set, apply it to the state
        if self.narrative is not None:
            self.state.set_narrative(self.narrative)
            update_metrics_values(self.state)

    def get_state(self):
        # return a representation of the current state of the game
        # Check if the state is None
        if self.state is None:
            return None
        
        try:
            # If the state is not None, return a representation of the current state of the game
            return {
                "id": self.state.id,
                "assistant": {"name": self.state.assistant.name, "age": self.state.assistant.age, "style": self.state.assistant.style, "traits": self.state.assistant.traits, "backstory": self.state.assistant.backstory} if self.state.assistant else None,
                "narrative": self.state.narrative.name if self.state.narrative else None,
                "influence": self.state.influence,
                "parameters": {name: param.value for name, param in self.state.parameters.items()},
                "decisions": [decision.name for decision in self.state.decisions.values()],
                "ministers": [minister.title for minister in self.state.ministers.values()],
                "citizen_groups": [group.name for group in self.state.citizen_groups.values()],
                "economic_sectors": [sector.name for sector in self.state.economic_sectors.values()],
                "metrics": self.state.get_metrics(),
                "country": self.state.country
            }
        except AttributeError:
            # This will catch the error when trying to access an attribute of None
            return None
    
    def load_countries(self, filename="data/countries.json"):
        # Load countries from a file
        with open(filename, 'r') as f:
            countries_list = json.load(f)
        countries = [country["name"] for country in countries_list]
        return countries
    
    def set_country(self, choice):
        # Load countries
        countries = self.load_countries()

        # Check if the choice is valid
        if choice < 1 or choice > len(countries):
            raise ValueError("Invalid country number")

        # Set the chosen country
        self.state.country = countries[choice-1]

    def load_assistants(self, filename="data/assistants.json"):
        # Load assistant attributes from a file
        with open(filename, 'r') as f:
            assistant_list = json.load(f)
        # Create Assistant objects for all assistants
        assistants = [Assistant(**assistant_attributes) for assistant_attributes in assistant_list]
        return assistants
    
    def set_assistant(self, choice):
        # Load the list of assistants
        assistants = self.load_assistants()

        # Check if the choice is valid
        if choice < 1 or choice > len(assistants):
            raise ValueError("Invalid assistant number")

        # Set the chosen assistant
        self.assistant = assistants[choice-1]
    
    def load_narratives(self, narratives_file="data/narratives.json"):
        # Load narratives from a local file
        with open(narratives_file, 'r') as f:
            narratives_data = json.load(f)

        # Create Narrative objects
        narratives = [Narrative(narrative['name'], narrative['description'], narrative['effects']) for narrative in narratives_data]
        return narratives
    
    def set_narrative(self, narrative_choice):
        # Load the list of narratives
        narratives = self.load_narratives()

        # Set the narrative if a choice was made
        if narrative_choice is not None:
            self.narrative = narratives[narrative_choice-1]
    
    def load_parameters(self, parameters_file):
        # Load parameters from a local file into the state
        with open(parameters_file, 'r') as f:
            parameters_data = json.load(f)

        parameters_instances = {
            name: Parameter(
                name, 
                info['initial_value'], 
                ParameterType[info['parameter_type'].upper()], 
                [dep for dep in info['dependencies']]
            )
            for name, info in parameters_data.items()
        }

        self.state.set_parameters(parameters_instances)


    def load_decisions(self, decisions_file):
        # Load decisions from a local file into the state
        with open(decisions_file, 'r') as f:
            decisions_data = json.load(f)

        decisions_instances = {}
        for decision in decisions_data:
            effects = {}
            for param_name, effect in decision['effects'].items():
                if param_name in self.state.parameters:
                    effects[self.state.parameters[param_name]] = effect
                else:
                    print(f"Warning: Unknown parameter '{param_name}' in decision '{decision['name']}'")
            decisions_instances[decision['name']] = Decision(decision['name'], effects, decision['influence_cost'], decision['economic_cost'])

        self.state.set_decisions(decisions_instances)


    def load_ministers(self, ministers_file):
        # Load ministers from a local file into the state
        with open(ministers_file, 'r') as f:
            ministers_data = json.load(f)
        
        ministers_instances = {
            minister['title']: Minister(minister['title'], minister['personal_name'], minister['loyalty'], minister['influence'], minister['backstory'])
            for minister in ministers_data
        }

        self.state.set_ministers(ministers_instances)

    def load_citizen_groups(self, citizen_groups_file):
        # Load citizen groups from a local file into the state
        with open(citizen_groups_file, 'r') as f:
            citizen_groups_data = json.load(f)
        
        citizen_groups_instances = {
            group['name']: CitizenGroup(**group)
            for group in citizen_groups_data
        }

        self.state.set_citizen_groups(citizen_groups_instances)

    
    def load_economic_sectors(self, economic_sectors_file):
        # Load economic sectors from a local file into the state
        with open(economic_sectors_file, 'r') as f:
            economic_sectors_data = json.load(f)
        
        economic_sectors_instances = {
            sector['name']: EconomicSector(sector['name'], sector['importance'])
            for sector in economic_sectors_data
        }

        self.state.set_economic_sectors(economic_sectors_instances)

    def load_metrics(self):
        set_metrics_values(self.state)
    
    def generate_response(self, query):
        respone = self.state.assistant.generate_decision(query)
        return respone

    def fetch_news(self):
        # Fetch news and return it
        news = self.assistant.fetch_news()
        return news
    
    def make_decision(self, decision_name: str):
        # Here, you would apply the given decision and return the new state of the game.
        decision = self.state.get_decision(decision_name)
        if decision:
            self.state.add_decision_to_apply(decision)
        else:
            raise ValueError(f"No decision named '{decision_name}' exists.")
    
    def next_cycle(self):
        changes = self.state.next_cycle()
        self.save_state(self.state, changes)
        # Update the metrics in the state
        update_metrics_values(self.state)

    def get_vote_share(self):
        result = self.state.calculate_vote_share()
        return result
    
    def save_state(self, state, changes):
        self.db_manager.save_state(state, changes)

    def load_states(self, simulation_id):
        return self.db_manager.load_states(simulation_id)
    
    def save_game_state_to_json(self, filename="data/game_state.json"):
        if self.state is None:
            raise ValueError("No game in progress")
        with open(filename, 'w') as f:
            json.dump(self.state.to_dict(), f)

    def load_game_state_from_json(self, filename="data/game_state.json"):
        with open(filename, 'r') as f:
            state_dict = json.load(f)
        self.state = State.from_dict(state_dict)
    
    def load_game_from_file(self, filename):
        # Load game from saved file
        self.state = State.load_game(filename)
    
    def stop_simulation(self):
        # self.save_game_state() TypeError: Object of type Parameter is not JSON serializable
        self.state = None
        self.assistant = None
        self.narrative = None

