from .simulation import Simulation
import logging

class SimulationLogic:
    def __init__(self):
        self.simulation = Simulation()
        self.logger = logging.getLogger(__name__)

    async def start_simulation(self):
        self.simulation.start_simulation()

    async def stop_simulation(self):
        self.simulation.stop_simulation()

    async def get_simulation_state(self):
        return self.simulation.get_state()

    async def update_simulation(self, decision):
        self.simulation.update_state(decision)

    async def get_assistant_state(self):
        return self.simulation.assistant.get_state()
    
    async def interact_with_assistant(self, command):
        try:
            response = await self.simulation.assistant.process_command(command)
            return response
        except Exception as e:
            self.logger.error(f"Error processing command: {e}")
