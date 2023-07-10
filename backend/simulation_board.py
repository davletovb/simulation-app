import streamlit as st
import pandas as pd
import httpx
import asyncio

def run_async(func):
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    return loop.run_until_complete(func)

# Define new functions
async def load_assistants():
    async with httpx.AsyncClient() as client:
        response = await client.get("http://localhost:8000/load_assistants")
        return response.json()["assistants"] if response.status_code == 200 else None

async def load_narratives():
    async with httpx.AsyncClient() as client:
        response = await client.get("http://localhost:8000/load_narratives")
        return response.json()["narratives"] if response.status_code == 200 else None

async def set_assistant(choice):
    async with httpx.AsyncClient() as client:
        response = await client.get(f"http://localhost:8000/set_assistant/{choice}")
        return response.status_code == 200

async def set_narrative(choice):
    async with httpx.AsyncClient() as client:
        response = await client.get(f"http://localhost:8000/set_narrative/{choice}")
        return response.status_code == 200

async def get_simulation_status():
    async with httpx.AsyncClient() as client:
        return await client.get("http://localhost:8000/")

async def start_simulation():
    async with httpx.AsyncClient() as client:
        return await client.get("http://localhost:8000/simulation/start")

async def stop_simulation():
    async with httpx.AsyncClient() as client:
        return await client.post("http://localhost:8000/simulation/stop")

async def get_simulation_state():
    async with httpx.AsyncClient() as client:
        resp = await client.get("http://localhost:8000/simulation/state")
        return resp.json() if resp.status_code == 200 else None

async def submit_decision(decision):
    async with httpx.AsyncClient() as client:
        response = await client.post("http://localhost:8000/simulation/decision", json={"decision_name": decision})
        return response.json() if response.status_code == 200 else None

# Streamlit code
#st.title("Simulation")

# Load and display the lists of assistants and narratives
assistants = run_async(load_assistants())
narratives = run_async(load_narratives())

# Check if the simulation has started
if "simulation_started" not in st.session_state:
    st.session_state.simulation_started = False

def button_start_callback():
    st.session_state.simulation_started=True

def button_stop_callback():
    st.session_state.simulation_started=False

if not st.session_state.simulation_started:
    if assistants is not None and narratives is not None:
        assistant_names = [assistant["name"] for assistant in assistants]
        narrative_names = [""] + [narrative["name"] for narrative in narratives]

        chosen_assistant_name = st.selectbox("Choose an assistant:", assistant_names)
        chosen_assistant = next((assistant for assistant in assistants if assistant["name"] == chosen_assistant_name), None)

        if chosen_assistant:
            st.text("Assistant Details:")
            st.text(f"Age: {chosen_assistant['age']}")
            st.text(f"Status: {chosen_assistant['status']}")
            st.text(f"Traits: {chosen_assistant['traits']}")
        
        chosen_narrative_name = st.selectbox("Choose a narrative:", narrative_names)
        chosen_narrative = next((narrative for narrative in narratives if narrative["name"] == chosen_narrative_name), None)

        # Set the assistant and narrative when the user presses the "Start Simulation" button
        if st.button("Start Simulation", on_click=button_start_callback):
            assistant_choice = assistant_names.index(chosen_assistant_name) + 1
            narrative_choice = narrative_names.index(chosen_narrative_name) if chosen_narrative_name else ""
            if run_async(set_assistant(assistant_choice)) and (narrative_choice == '' or run_async(set_narrative(narrative_choice))):
                st.write(run_async(start_simulation()))
                #st.session_state.simulation_started = True
            else:
                st.write("Error starting simulation")
    else:
        st.write("Error loading assistants or narratives. Please check the backend service.")

# Display the rest of the dashboard after the game has been started
if st.session_state.simulation_started:

    simulation_state = run_async(get_simulation_state())['state']

    # Sidebar
    if simulation_state:
        # show a sidebar with the app title and description
        st.sidebar.title('AImpact')
        st.sidebar.markdown('This is a demo political simulation for modeling the impact of decision making.')
        if st.sidebar.button("Stop Simulation", on_click=button_stop_callback):
            st.sidebar.write(run_async(stop_simulation()))
        #st.session_state.simulation_started = False
        #st.sidebar.markdown('---')
        # Sidebar for Assistant, Narrative, and Influence
        st.sidebar.markdown('### Simulation')
        if 'assistant' in simulation_state:
            st.sidebar.text(f"Assistant: {simulation_state['assistant']['name']}, {simulation_state['assistant']['age']}")
        if 'narrative' in simulation_state:
            st.sidebar.text(f"Narrative: {simulation_state['narrative']}")
        if 'influence' in simulation_state:
            st.sidebar.text(f"Influence: {simulation_state['influence']}")
        
        #st.json(simulation_state)
        # Sidebar
        st.sidebar.markdown('### Dashboards')
        view = st.sidebar.radio(
            'Select a view',
            ('Ministers', 'Citizen Groups', 'Economic Sectors', 'Parameters', 'Metrics', 'Policies')
        )

    # Main Area
    if simulation_state:
        if view == 'Ministers':
            st.subheader('Ministers')
            st.write(simulation_state['ministers'])
        elif view == 'Citizen Groups':
            st.subheader('Citizen Groups')
            st.write(simulation_state['citizen_groups'])
        elif view == 'Economic Sectors':
            st.subheader('Economic Sectors')
            st.write(simulation_state['economic_sectors'])
        elif view == 'Parameters':
            st.subheader('Parameters')
            st.write(simulation_state['parameters'])
        elif view == 'Metrics':
            st.subheader('Metrics')
            st.write(simulation_state['metrics'])
        elif view == 'Policies':
            st.subheader('Policies')
            st.write(simulation_state['decisions'])
            decision = st.text_input("Make a Decision")
            if st.button("Submit Decision"):
                decision_response = run_async(submit_decision(decision))
                if decision_response:
                    st.write(decision_response)
    else:
        st.write("Error loading simulation state. Please check the backend service.")

    

    


