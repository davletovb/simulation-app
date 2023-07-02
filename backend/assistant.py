from simulation import Simulation, Transition
from langchain import LangChainAssistant

class AssistantError(Exception):
    pass

class Assistant:
    def __init__(self):
        self.assistant = LangChainAssistant()
        self.state = "idle"
        self.memory = {}
        self.commands = {
            "start_simulation": self.start_simulation,
            "stop_simulation": self.stop_simulation,
            "get_simulation_state": self.get_state,
        }

    async def send_command(self, command):
        if command in self.commands:
            return await self.commands[command]()
        else:
            raise ValueError(f"Invalid command: {command}")

    async def start_simulation(self):
        self.state = "active"
        return self.state

    async def stop_simulation(self):
        self.state = "idle"
        return self.state

    async def get_state(self):
        state = await self.assistant.get_state()
        return state

    async def remember(self, item_type: str, item):
        """Remember an item of a certain type."""
        if item_type not in self.memory:
            self.memory[item_type] = []
        self.memory[item_type].append(item)

    def get_personality(self):
        personality = "friendly" if len(self.history) % 2 == 0 else "grumpy"
        return personality
    
    async def interact_with_user(self, command: str):
        """Interact with the user based on a command."""
        # Here you would use the LangChainAssistant to generate a response to the user input
        response = await self.assistant.generate_response(command)
        return response
    
    async def make_decision(self, simulation: Simulation) -> Transition:
        try:
            decision = await self.assistant.generate_decision(simulation.current_state)
        except Exception as e:
            raise AssistantError("Failed to make decision") from e
        return decision