from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.security import OAuth2PasswordBearer, OAuth2PasswordRequestForm
from jose import JWTError, jwt
from datetime import timedelta
from sqlalchemy.orm import Session
from pydantic import BaseModel
import uvicorn

from .simulation_logic import SimulationLogic
from .database import Session, User, engine, SessionLocal, Base
from .auth import create_access_token, get_password_hash, verify_password, Token, TokenData, ACCESS_TOKEN_EXPIRE_MINUTES, ALGORITHM, SECRET_KEY


app = FastAPI()

simulation_logic = SimulationLogic()


class Decision(BaseModel):
    parameter: str
    action: str
    command: str


class Command(BaseModel):
    command: str

class UserIn(BaseModel):
    username: str
    password: str

oauth2_scheme = OAuth2PasswordBearer(tokenUrl="token")

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
    # Here you would return a list of all users
    pass


@app.post("/simulation/start")
async def start_simulation(current_user: User = Depends(get_current_user)):
    await simulation_logic.start_simulation()
    return {"status": "Simulation started"}


@app.post("/simulation/stop")
async def stop_simulation(current_user: User = Depends(get_current_user)):
    await simulation_logic.stop_simulation()
    return {"status": "Simulation stopped"}


@app.get("/simulation/state")
async def get_simulation_state(current_user: User = Depends(get_current_user)):
    state = await simulation_logic.get_simulation_state()
    return {"state": state}


@app.post("/simulation/update")
async def update_simulation(decision: Decision, current_user: User = Depends(get_current_user)):
    await simulation_logic.update_simulation(decision)
    return {"message": "Simulation updated"}


@app.post("/simulation/decision")
async def make_decision(decision: Decision, current_user: User = Depends(get_current_user)):
    # Here you would update the simulation based on the decision
    transition = await simulation_logic.simulation.assistant.make_decision(simulation_logic.simulation)
    return {"status": f"Decision made: {decision.parameter} - {decision.action}", "transition": transition}


@app.get("/assistant/state")
async def get_assistant_state(current_user: User = Depends(get_current_user)):
    state = await simulation_logic.get_assistant_state()
    return {"state": state}


@app.post("/assistant/command")
async def send_command_to_assistant(command: Command, current_user: User = Depends(get_current_user)):
    try:
        response = await simulation_logic.interact_with_assistant(command.command)
        return {"response": response}
    except ValueError as e:
        return {"error": str(e)}


if __name__ == "__main__":
    Base.metadata.create_all(bind=engine)
    uvicorn.run(app, host="localhost", port=8000)
