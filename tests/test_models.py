from backend.models import Simulation, Decision


def test_simulation_update():
    simulation = Simulation()
    decision = Decision(parameter="economy", action="invest",
                        command="start_simulation")
    simulation.update(decision)
    assert simulation.primary_parameters["economy"].value == 60

# ... more tests for the Simulation and Assistant classes ...
