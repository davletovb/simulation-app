from simulation import State, Parameter, ParameterType, Narrative, Decision, Minister, CitizenGroup, EconomicSector
from metrics import set_metrics_values
from assistant import Assistant
import logging
import json


class SimulationController:
    def __init__(self):
        self.assistant = None
        self.state = None
    
    def load_assistant(self, filename):
        # Load assistant attributes from a file
        with open(filename, 'r') as f:
            assistant_list = json.load(f)
        # Print all assistants and let the user choose one
        for i, assistant_attributes in enumerate(assistant_list):
            print(f"{i+1}. {assistant_attributes['name']}")
        choice = int(input("Choose an assistant by entering its number: "))
        assistant = Assistant(**assistant_list[choice-1])
        return assistant
    
    def start_game(self):
        # Here, you would typically start a new game, perhaps by initializing or resetting your state.
        self.assistant = self.load_assistant("data/assistants.json")
        self.state = State(assistant=self.assistant)
        # Load all default entities into the state
        self.load_parameters("data/parameters.json")  # replace with your actual filenames
        self.load_ministers("data/ministers.json")
        self.load_decisions("data/decisions.json")
        self.load_citizen_groups("data/citizen_groups.json")
        self.load_economic_sectors("data/economic_sectors.json")
        self.load_metrics()
        narratives = self.load_narratives('data/narratives.json')
        self.choose_narrative(narratives)
        return self.get_state()

    def make_decision(self, decision_name: str):
        # Here, you would apply the given decision and return the new state of the game.
        decision = self.state.get_decision(decision_name)
        if decision:
            self.state.apply_decision(decision)
            return self.get_state()
        else:
            raise ValueError(f"No decision named '{decision_name}' exists.")

    def get_state(self):
        # Here, you would return a representation of the current state of the game.
        return {
            "parameters": {name: param.value for name, param in self.state.parameters.items()},
            "decisions": [decision.name for decision in self.state.decisions.values()],
            "ministers": [minister.name for minister in self.state.ministers.values()],
            "citizen_groups": [group.name for group in self.state.citizen_groups.values()],
            "economic_sectors": [sector.name for sector in self.state.economic_sectors.values()],
            "metrics": self.state.get_metrics_values(),
            "influence": self.state.influence,
            "narrative": self.state.narrative.name if self.state.narrative else None
        }

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
            minister['name']: Minister(minister['name'], minister['loyalty'], minister['influence'])
            for minister in ministers_data
        }

        self.state.set_ministers(ministers_instances)

    def load_citizen_groups(self, citizen_groups_file):
        # Load citizen groups from a local file into the state
        with open(citizen_groups_file, 'r') as f:
            citizen_groups_data = json.load(f)
        
        citizen_groups_instances = {
            group['name']: CitizenGroup(group['name'], group['size'], group['political_view'])
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

    def load_narratives(self, narratives_file):
        # Load narratives from a local file
        with open(narratives_file, 'r') as f:
            narratives_data = json.load(f)

        # Create Narrative objects
        narratives = [Narrative(narrative['name'], narrative['effects']) for narrative in narratives_data]
        return narratives
    
    def choose_narrative(self, narratives):
        print("Available narratives:")
        for i, narrative in enumerate(narratives, start=1):
            print(f"{i}. {narrative.name}")
        choice = int(input("Choose a narrative by number: ")) - 1
        chosen_narrative = narratives[choice]
        self.state.set_narrative(chosen_narrative)
        self.state.update_metrics()
        print(self.state.metrics)
        print(f"You have chosen the {chosen_narrative.name} narrative.")

    def fetch_news(self):
        # Fetch news and return it
        news = self.assistant.fetch_news()
        return news

    def load_game(self, filename):
        # Load game from saved file
        self.state = State.load_game(filename)

    def play_game(self):
        # Start the game loop
        while True:
            # Load a saved game if the player wants to
            if input("Do you want to load a saved game? (yes/no) ") == 'yes':
                filename = 'data/game_state.pkl' #input("Enter the filename of the saved game: ")
                self.load_game(filename)
            #else:
            #    # If not loading a saved game, allow the player to choose a narrative
            #    narratives = self.load_narratives('data/narratives.json')
            #    self.choose_narrative(narratives)

            # Print the current state of the game
            print(f"Current influence: {self.state.influence}")
            for name, parameter in self.state.parameters.items():
                print(f"{name}: {parameter.value}")

            # Fetch news and generate decision
            news_event = self.fetch_news()
            decision = self.assistant.generate_decision(news_event)

            # Apply the decision to the state
            self.state.apply_decision(decision)
            self.state.update_metrics()

            # Generate the assistant's response
            response = self.assistant.generate_response(self.state, news_event)
            print(response)

            # Adjust all parameters for their dependencies after a round, delayed update
            for parameter in self.state.parameters.values():
                parameter.adjust_for_dependency()

            # Ask the player if they want to save the game
            if input("Do you want to save the game? (yes/no) ") == 'yes':
                self.state.save_game('data/game_state.pkl')

            # End the game if the player wants to quit
            if input("Do you want to quit the game? (yes/no) ") == 'yes':
                break

