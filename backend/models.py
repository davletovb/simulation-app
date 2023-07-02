from langchain import LangChainAssistant
import json
from .database import Base, SimulationState, Session


class Parameter:
    def __init__(self, initial_value):
        self.value = initial_value

    def update(self, decision):
        if decision.action not in ["invest", "cut"]:
            raise ValueError("Invalid action: must be 'invest' or 'cut'")
        if decision.action == "invest":
            self.value += 10
        elif decision.action == "cut":
            self.value -= 10


class Simulation(Base):

    def __init__(self):
        self.primary_parameters = {
            "economy": Parameter(50),
            # ... initialize the rest of the primary parameters ...
        }
        self.secondary_parameters = self.calculate_secondary_parameters()
        self.assistant = Assistant()
        self.session = Session()
        self.is_running = False

    def start(self):
        if self.is_running:
            raise ValueError("Simulation is already running")
        self.is_running = True

    def stop(self):
        if not self.is_running:
            raise ValueError("Simulation is not running")
        self.is_running = False

    def get_state(self):
        if not self.is_running:
            raise ValueError("Simulation is not running")
        return {
            "is_running": self.is_running,
            "primary_parameters": self.primary_parameters,
            "secondary_parameters": self.secondary_parameters,
            "tertiary_parameters": self.tertiary_parameters,
            "assistant_state": self.assistant.state,
        }

    def update(self, decision):
        if decision.parameter not in self.primary_parameters:
            raise ValueError(f"Invalid parameter: {decision.parameter}")
        self.primary_parameters[decision.parameter].update(decision)
        self.secondary_parameters = self.calculate_secondary_parameters()
        self.assistant.process_command(decision.command)

    def interact_with_assistant(self, command):
        response = self.assistant.interact_with_user(command)
        return response

    def calculate_secondary_parameters(self):
        employment_rate = self.primary_parameters["economy"].value * 0.5
        quality_of_life = self.primary_parameters["healthcare"].value * \
            0.3 + self.primary_parameters["economy"].value * 0.2
        # ... calculate the rest of the secondary parameters ...
        return {"employment_rate": employment_rate, "quality_of_life": quality_of_life}

    def save_state(self):
        state = SimulationState(
            primary_parameters=json.dumps(self.primary_parameters),
            secondary_parameters=json.dumps(self.secondary_parameters),
            tertiary_parameters=json.dumps(self.tertiary_parameters),
            assistant_state=self.assistant.state,
        )
        self.session.add(state)
        self.session.commit()

    def load_state(self, id):
        state = self.session.query(SimulationState).get(id)
        self.primary_parameters = json.loads(state.primary_parameters)
        self.secondary_parameters = json.loads(state.secondary_parameters)
        self.tertiary_parameters = json.loads(state.tertiary_parameters)
        self.assistant.state = state.assistant_state


class Assistant:
    def __init__(self):
        self.assistant = LangChainAssistant()
        self.state = "idle"

    def get_state(self):
        if self.state == "idle":
            raise ValueError("Assistant is not active")
        return self.state

    def send_command(self, command):
        if command == "start":
            self.state = "active"
        elif command == "stop":
            self.state = "idle"
        else:
            raise ValueError(f"Invalid command: {command}")
        return self.state

    # more detailed example of the above function
    def process_command(self, command):
        # Here we would process the command and determine the appropriate action
        if command == "start_simulation":
            # Start the simulation
            self.state = "active"
        elif command == "stop_simulation":
            # Stop the simulation
            self.state = "idle"
        elif command == "get_simulation_state":
            # Return the current state of the simulation
            self.get_state()
        else:
            raise ValueError(f"Invalid command: {command}")

    def interact_with_user(self, user_input):
        # Here we would use the LangChainAssistant to generate a response to the user input
        response = self.assistant.generate_response(user_input)
        return response
