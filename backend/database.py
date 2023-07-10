from sqlalchemy import Column, Integer, String, create_engine, Text, Boolean, JSON
from sqlalchemy.orm import sessionmaker, Session
from sqlalchemy.ext.declarative import declarative_base
from typing import Dict

import logging
import csv
import json

Base = declarative_base()

SQLALCHEMY_DATABASE_URL = "sqlite:///./test.db"

engine = create_engine(
    SQLALCHEMY_DATABASE_URL, connect_args={"check_same_thread": False}
)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

Base.metadata.create_all(engine)

class SimulationState(Base):
    __tablename__ = "simulation_state"

    id = Column(Integer, primary_key=True)
    state = Column(JSON, default={})

    def get_state(self):
        return self.state

    def set_state(self, state):
        self.state = state


class User(Base):
    __tablename__ = "users"
    id = Column(Integer, primary_key=True, index=True)
    username = Column(String, unique=True, index=True)
    password = Column(String)
    is_active = Column(Boolean, default=True)
    is_superuser = Column(Boolean, default=False)


"""
def save_state(state: State, db: Session):
    try:
        state_dict = {
            "simulation_state": state,
            "assistant_state": state.assistant.get_state(),
        }
        state_record = SimulationState(state=state_dict)
        db.add(state_record)
        db.commit()
    except Exception as e:
        state.logger.error(f"Error saving state: {e}")

def load_state(id: int, state: State, db: Session):
    try:
        state_record = db.query(SimulationState).get(id)
        state_dict = state_record.get_state()
        state.current_state.set_value(state_dict["simulation_state"])
        state.assistant.set_state(state_dict["assistant_state"])
    except Exception as e:
        state.logger.error(f"Error loading state: {e}")


def load_parameters(filename: str) -> Dict[str, Dict[str, Parameter]]:
    parameters = {
        "primary": {},
        "secondary": {},
        "tertiary": {}
    }
    with open(filename, 'r') as f:
        reader = csv.reader(f)
        next(reader)  # Skip the header row
        for row in reader:
            name, value, type, initial_value, dependencies, category = row
            dependencies = dependencies.split(',') if dependencies else []
            parameter = Parameter(name, value, ParameterType[type.upper()], initial_value, dependencies)
            parameters[category][name] = parameter

    # Now that all parameters are loaded, replace dependency names with actual Parameter objects
    for category in parameters.values():
        for parameter in category.values():
            parameter.dependencies = [parameters[dep_category][name] for dep_category in parameters for name in parameter.dependencies if name in parameters[dep_category]]

    return parameters


def save_parameters(parameters: Dict[str, Parameter], filename: str):
    with open(filename, 'w') as f:
        writer = csv.writer(f)
        writer.writerow(["name", "value", "type", "initial_value"])  # Write the header row
        for name, parameter in parameters.items():
            writer.writerow([name, parameter.value, parameter.type.name, parameter.initial_value])
"""