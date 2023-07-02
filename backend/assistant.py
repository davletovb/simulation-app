from simulation import Simulation, State, Transition
from langchain import LangChainAssistant

class AssistantError(Exception):
    pass

class Assistant:
    def __init__(self):
        self.assistant = LangChainAssistant()
        self.state = "idle"
        self.history = []
        self.commands = {
            "start_simulation": self.start_simulation,
            "stop_simulation": self.stop_simulation,
            "get_simulation_state": self.get_state,
        }

    def send_command(self, command):
        if command in self.commands:
            return self.commands[command]()
        else:
            raise ValueError(f"Invalid command: {command}")

    def start_simulation(self):
        self.state = "active"
        return self.state

    def stop_simulation(self):
        self.state = "idle"
        return self.state

    async def get_state(self):
        state = await self.assistant.get_state()
        return state

    def remember_interaction(self, interaction):
        self.history.append(interaction)

    def get_personality(self):
        personality = "friendly" if len(self.history) % 2 == 0 else "grumpy"
        return personality
    
    def make_decision(self, simulation: Simulation) -> Transition:
        try:
            decision = self.assistant.generate_decision(simulation.current_state)
        except Exception as e:
            raise AssistantError("Failed to make decision") from e
        return decision