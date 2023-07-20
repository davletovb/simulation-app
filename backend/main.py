from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.security import OAuth2PasswordBearer, OAuth2PasswordRequestForm
from jose import JWTError, jwt
from datetime import timedelta
from sqlalchemy.orm import Session
from pydantic import BaseModel
import uvicorn
import logging
import os

from simulation_logic import SimulationController
from database import Session, User, engine, SessionLocal, Base
from auth import create_access_token, get_password_hash, verify_password, Token, TokenData, ACCESS_TOKEN_EXPIRE_MINUTES, ALGORITHM, SECRET_KEY

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Allows all origins
    allow_credentials=True,
    allow_methods=["*"],  # Allows all methods
    allow_headers=["*"],  # Allows all headers
)

simulation_controller = SimulationController()

# Define a Pydantic model for Decision
class DecisionModel(BaseModel):
    decision_name: str

class QueryModel(BaseModel):
    query: str

"""
oauth2_scheme = OAuth2PasswordBearer(tokenUrl="token")

class UserIn(BaseModel):
    username: str
    password: str

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

@app.post("/register", response_model=UserIn)
def register(user: UserIn, db: Session = Depends(get_db)):
    hashed_password = get_password_hash(user.password)
    db_user = User(username=user.username, hashed_password=hashed_password)
    db.add(db_user)
    db.commit()
    db.refresh(db_user)
    return db_user

@app.post("/token", response_model=Token)
def login_for_access_token(form_data: OAuth2PasswordRequestForm = Depends(), db: Session = Depends(get_db)):
    user = db.query(User).filter(User.username == form_data.username).first()
    if not user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
            headers={"WWW-Authenticate": "Bearer"},
        )
    if not verify_password(form_data.password, user.hashed_password):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
            headers={"WWW-Authenticate": "Bearer"},
        )
    access_token_expires = timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES)
    access_token = create_access_token(
        data={"sub": user.username}, expires_delta=access_token_expires
    )
    return {"access_token": access_token, "token_type": "bearer"}


def get_current_user(db: Session = Depends(get_db), token: str = Depends(oauth2_scheme)):
    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        username: str = payload.get("sub")
        if username is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid authentication credentials",
                headers={"WWW-Authenticate": "Bearer"},
            )
        token_data = TokenData(username=username)
    except JWTError:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid authentication credentials",
            headers={"WWW-Authenticate": "Bearer"},
        )
    user = db.query(User).filter(User.username == token_data.username).first()
    if user is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="User not found",
        )
    return user


def get_current_superuser(current_user: User = Depends(get_current_user)):
    if not current_user.is_superuser:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="The user doesn't have enough privileges",
        )
    return current_user


@app.get("/users")
async def read_users(current_user: User = Depends(get_current_superuser)):
    # returns a list of all users for admin
    pass
"""

@app.get("/")
async def index():
    return {"status": "ok"}

@app.get("/simulation/start")
async def start_simulation():
    simulation_controller.start_simulation()
    return {"status": "Simulation started"}

@app.post("/simulation/stop")
async def stop_simulation():
    simulation_controller.stop_simulation()
    return {"status": "Simulation stopped"}

@app.get("/load_assistants")
async def load_assistants():
    assistants = simulation_controller.load_assistants()
    # Convert the assistants to a form that can be sent as JSON
    assistant_dicts = [{"name": assistant.name, "age": assistant.age, "style": assistant.style, "traits": assistant.traits, "backstory": assistant.backstory} for assistant in assistants]
    return {"assistants": assistant_dicts}

@app.get("/load_narratives")
async def load_narratives():
    narratives = simulation_controller.load_narratives()
    # Convert the narratives to a form that can be sent as JSON
    narrative_dicts = [{"name": narrative.name, "description": narrative.description, "effects": narrative.effects} for narrative in narratives]
    return {"narratives": narrative_dicts}

@app.get("/load_countries")
async def load_countries():
    countries = simulation_controller.load_countries()
    countries_dict = [{"name": country} for country in countries]
    return {"countries": countries_dict}

@app.get("/set_assistant/{assistant_choice}")
async def set_assistant(assistant_choice: int):
    simulation_controller.set_assistant(assistant_choice)
    return {"message": "Assistant set"}

@app.get("/set_narrative/{narrative_choice}")
async def set_narrative(narrative_choice: int):
    simulation_controller.set_narrative(narrative_choice)
    return {"message": "Narrative set"}

@app.get("/set_country/{country_choice}")
async def set_country(country_choice: int):
    simulation_controller.set_country(country_choice)
    return {"message": "Country set"}

@app.get("/simulation/state")
async def get_simulation_state():
    state = simulation_controller.get_state()
    return {"state": state}

@app.post("/simulation/decision")
async def make_decision(decision: DecisionModel):
    decision_name = decision.decision_name
    simulation_controller.make_decision(decision_name)
    return {"message": f"Decision {decision_name} submitted"}

@app.post("/simulation/save")
async def save_state():
    simulation_controller.save_game_state()
    return {"status": "Simulation state saved"}

@app.get("/simulation/load/{state_id}")
async def load_states(state_id: str):
    state_history = simulation_controller.load_states(state_id)
    return {"status": state_history}

@app.get("/simulation/next_cycle")
async def next_cycle():
    simulation_controller.next_cycle()
    return {"status": "Next cycle started"}

@app.get("/simulation/news")
async def fetch_news():
    news_event = simulation_controller.fetch_news()
    return {"news_event": news_event}

# Route to generate a decision based on news
@app.get("/simulation/generate_decision")
async def generate_decision():
    news_event = simulation_controller.fetch_news()
    decision = simulation_controller.assistant.generate_decision(news_event)
    return {"decision": decision}

# Route to get calculated vote share
@app.get("/simulation/get_vote_share")
async def get_vote_share():
    result = simulation_controller.get_vote_share()
    return {"result": result}

# Route to generate assistant's response
@app.post("/simulation/generate_response")
async def generate_response(query_model: QueryModel):
    response = simulation_controller.generate_response(query_model.query)
    return {"response": response}

if __name__ == "__main__":
    #Base.metadata.create_all(bind=engine)
    uvicorn.run(app, host="localhost", port=8000)
