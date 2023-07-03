from sqlalchemy import Column, Integer, String, create_engine, Text, Boolean
from sqlalchemy.orm import sessionmaker
from sqlalchemy.ext.declarative import declarative_base
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
    primary_parameters = Column(Text)
    secondary_parameters = Column(Text)
    tertiary_parameters = Column(Text)
    assistant_state = Column(String)
    state = Column(String, default="{}")

    def get_state(self):
        return json.loads(self.state)

    def set_state(self, state):
        self.state = json.dumps(state)


class User(Base):
    __tablename__ = "users"
    id = Column(Integer, primary_key=True, index=True)
    username = Column(String, unique=True, index=True)
    password = Column(String)
    is_active = Column(Boolean, default=True)
    is_superuser = Column(Boolean, default=False)
