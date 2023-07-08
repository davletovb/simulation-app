from .simulation import State, Parameter, Minister, EconomicSector, CitizenGroup, ParameterType, Narrative, Decision, Metric
from .database import save_state, load_state, load_parameters
from .assistant import Assistant
import logging
import json


class SimulationController:
    def __init__(self):
        self.assistant = self.load_assistant("assistant.json")
        # Load all default entities into the state
        self.state = State(assistant=self.assistant)
        self.load_parameters("parameters.json")  # replace with your actual filenames
        self.load_ministers("ministers.json")
        self.load_decisions("decisions.json")
        self.load_citizen_groups("citizen_groups.json")
        self.load_economic_sectors("economic_sectors.json")
    
    def load_assistant(self, filename):
        # Load assistant attributes from a file
        with open(filename, 'r') as f:
            assistant_attributes = json.load(f)
        assistant = Assistant(**assistant_attributes)
        return assistant

    def load_parameters(self, parameters_file):
        # Load parameters from a local file into the state
        with open(parameters_file, 'r') as f:
            parameters_data = json.load(f)

        primary_parameters_instances = {name: Parameter(name, 0, ParameterType.PRIMARY) 
                                        for name in parameters_data['primary_parameters']}

        secondary_parameters_instances = {
            name: Parameter(name, 0, ParameterType.SECONDARY, 
                            [primary_parameters_instances[dep] for dep in dependencies])
            for name, dependencies in parameters_data['secondary_parameters_dependencies'].items()
        }

        combined_parameters = {**primary_parameters_instances, **secondary_parameters_instances}

        tertiary_parameters_instances = {
            name: Parameter(name, 0, ParameterType.TERTIARY, 
                            [combined_parameters[dep] for dep in dependencies])
            for name, dependencies in parameters_data['tertiary_parameters_dependencies'].items()
        }

        all_parameters_instances = {**primary_parameters_instances, **secondary_parameters_instances, 
                                    **tertiary_parameters_instances}

        self.state.set_parameters(all_parameters_instances)

    def load_decisions(self, decisions_file):
        # Load decisions from a local file into the state
        with open(decisions_file, 'r') as f:
            decisions = json.load(f)
        self.state.set_decisions(decisions)

    def load_ministers(self, ministers_file):
        # Load ministers from a local file into the state
        with open(ministers_file, 'r') as f:
            ministers = json.load(f)
        self.state.set_ministers(ministers)

    def load_citizen_groups(self, citizen_groups_file):
        # Load citizen groups from a local file into the state
        with open(citizen_groups_file, 'r') as f:
            citizen_groups = json.load(f)
        self.state.set_citizen_groups(citizen_groups)
    
    def load_economic_sectors(self, economic_sectors_file):
        # Load economic sectors from a local file into the state
        with open(economic_sectors_file, 'r') as f:
            economic_sectors = json.load(f)
        self.state.set_economic_sectors(economic_sectors)

    def load_narratives(self, narratives_file):
        # Load economic sectors from a local file into the state
        with open(narratives_file, 'r') as f:
            narratives = json.load(f)
        return narratives
    
    def choose_narrative(self, narratives):
        print("Available narratives:")
        for i, narrative in enumerate(narratives, start=1):
            print(f"{i}. {narrative['name']}")
        choice = int(input("Choose a narrative by number: ")) - 1
        chosen_narrative = narratives[choice]
        self.state.set_narrative(chosen_narrative)
        print(f"You have chosen the {chosen_narrative['name']} narrative.")

    def fetch_news(self):
        # Fetch news and return it
        news = self.assistant.fetch_news()
        return news

    def load_game(self, filename):
        self.state = State.load_game(filename)

    def play_game(self):
        # Start the game loop
        while True:
            # Load a saved game if the player wants to
            if input("Do you want to load a saved game? (yes/no) ") == 'yes':
                filename = input("Enter the filename of the saved game: ")
                self.load_game(filename)
            else:
                # If not loading a saved game, allow the player to choose a narrative
                narratives = self.load_narratives('narratives.json')
                self.choose_narrative(narratives)

            # Print the current state of the game
            print(f"Current influence: {self.state.influence}")
            for name, parameter in self.state.parameters.items():
                print(f"{name}: {parameter.value}")

            # Fetch news and generate decision
            news_event = self.fetch_news()
            decision = self.assistant.generate_decision(news_event)

            # Apply the decision to the state
            self.state.apply_decision(decision)

            # Generate the assistant's response
            response = self.assistant.generate_response(self.state, news_event)
            print(response)

            # Adjust all parameters for their dependencies after a round, delayed update
            for parameter in self.state.parameters.values():
                parameter.adjust_for_dependency()

            # Ask the player if they want to save the game
            if input("Do you want to save the game? (yes/no) ") == 'yes':
                self.state.save_game('game_state.pkl')

            # End the game if the player wants to quit
            if input("Do you want to quit the game? (yes/no) ") == 'yes':
                break

