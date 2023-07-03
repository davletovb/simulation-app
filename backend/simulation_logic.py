from .simulation import Simulation

class SimulationLogic:
    def __init__(self):
        self.simulation = Simulation()

    async def start_simulation(self):
        self.simulation.start()

    async def stop_simulation(self):
        self.simulation.stop()

    async def get_simulation_state(self):
        return self.simulation.get_state()

    async def update_simulation(self, decision):
        self.simulation.update(decision)

    async def get_assistant_state(self):
        return self.simulation.assistant.get_state()

    async def interact_with_assistant(self, command):
        return self.simulation.assistant.process_command(command)
