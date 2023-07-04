from sqlalchemy.orm import Session
from .simulation import Simulation
from .database import save_state, load_state, load_parameters
import logging

class SimulationLogic:
    def __init__(self, db: Session):
        self.simulation = Simulation()
        self.db = db
        self.logger = logging.getLogger(__name__)

    async def start_simulation(self):
        self.simulation.start_simulation()

    async def stop_simulation(self):
        self.simulation.stop_simulation()

    async def get_simulation_state(self):
        return self.simulation.get_state()

    async def update_simulation_state(self, decision):
        self.simulation.update_state(decision)

    async def get_assistant_state(self):
        return self.simulation.assistant.get_state()
    
    async def interact_with_assistant(self, command):
        return await self.simulation.assistant.process_command(command)
    
    async def save_state(self):
        save_state(self.simulation, self.db)

    async def load_state(self, id: int):
        load_state(id, self.simulation, self.db)

    async def load_parameters(self, filename: str):
        parameters = load_parameters(filename)
        self.simulation.current_state.parameters = parameters
