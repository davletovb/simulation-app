from .simulation import Simulation, Transition, ParameterTransition
from langchain import LangChainAssistant

import csv

class AssistantError(Exception):
    pass

class Assistant:
    def __init__(self, simulation: Simulation):
        self.simulation = simulation
        self.conversation_history = []
        self.personality_traits = self.load_personality_traits("personality_traits.csv")
        self.backstory = self.load_backstory("backstory.txt")
        self.speech_style = "formal"
        self.emotional_state = "neutral"
    
    def load_personality_traits(self, filename):
        personality_traits = {}
        with open(filename, 'r') as file:
            reader = csv.reader(file)
            next(reader)  # Skip the header row
            for row in reader:
                trait, value = row
                personality_traits[trait] = value
        return personality_traits

    def load_backstory(self, filename):
        with open(filename, 'r') as file:
            backstory = file.read()
        return backstory

    def adjust_speech_style(self, style):
        self.speech_style = style

    def adjust_emotional_state(self, emotion):
        self.emotional_state = emotion

    def analyze_speech_style(self, command):
        # Analyze the command and return the appropriate speech style
        pass

    def analyze_emotional_state(self, command):
        # Analyze the command and return the appropriate emotional state
        pass

    async def process_command(self, command: str):
        # Add command to conversation history
        self.conversation_history.append({"user": command})
        
        # Adjust speech style and emotional state based on command
        self.adjust_speech_style(self.analyze_speech_style(command))
        self.adjust_emotional_state(self.analyze_emotional_state(command))

        # Parse the command
        parsed_command = self.parse_command(command)

        # Perform the action
        if parsed_command == "stop_simulation":
            response = await self.stop_simulation()
        elif parsed_command == "get_simulation_state":
            response = await self.get_state()
        elif parsed_command == "make_decision":
            response = await self.generate_decision()
        else:
            raise ValueError(f"Invalid command: {command}")

        # Add response to conversation history
        self.conversation_history.append({"assistant": response})

        # Return the response
        return response

    async def stop_simulation(self):
        self.state["status"] = "idle"
        self.current_task = "stop_simulation"
        await self.simulation.stop_simulation()

    def get_state(self, key=None):
        if key is None:
            return self.state
        else:
            return self.state.get(key)

    def set_state(self, key, value):
        self.state[key] = value

    async def generate_decision(self, simulation: Simulation) -> Transition:
        # Analyze the current state of the simulation
        state = simulation.get_state()

        """
        Use the assistant's decision-making capabilities to make a decision
        This could involve machine learning, rule-based systems, etc.
        For now, let's assume that the assistant has a method called analyze_state
        that takes a state and returns a decision
        The assistant needs to understand the current state of the simulation to make an informed decision. 
        This could involve extracting and analyzing various features from the state.
        It may also involve the state and narrative. Assistant will generate a list of decisions based on
        the current state and the narrative of the simulation.
        """
        decisions = await self.assistant.analyze_state(state)

        # Create a ParameterTransition for each decision
        transitions = []
        for decision in decisions:
            for category in ["primary", "secondary", "tertiary"]:
                if decision.parameter_name in state.parameters[category]:
                    transition = ParameterTransition({decision.parameter_name: decision})
                    transitions.append(transition)

        # Return the list of transitions
        return transitions

