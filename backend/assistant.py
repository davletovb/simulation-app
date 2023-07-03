from simulation import Simulation, Transition
from langchain import LangChainAssistant

class AssistantError(Exception):
    pass

class Assistant:
    def __init__(self):
        self.assistant = LangChainAssistant()
        self.state = {
            "status": "idle",
            "conversation_history": [],
            "current_task": None,
            "knowledge_base": {},
            "personality_traits": {},
            "mood": "neutral",
            "emotional_state": None
        }

    async def process_command(self, command: str):
        # Add command to conversation history
        self.conversation_history.append({"user": command})
        
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
        self.state = "idle"
        self.current_task = "stop_simulation"
        return self.state

    def get_state(self, key=None):
        if key is None:
            return self.state
        else:
            return self.state.get(key)

    def set_state(self, key, value):
        self.state[key] = value

    async def generate_decision(self, simulation: Simulation) -> Transition:
        # Analyze the current state of the simulation
        state = simulation.current_state

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
        decision = await self.assistant.analyze_state(state)

        # Return the decision
        return decision
