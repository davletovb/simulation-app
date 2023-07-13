import streamlit as st
import openai
import pandas as pd
import httpx
import asyncio
import os
import json

openai.api_key = os.environ["OPENAI_API_KEY"]

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

async def next_cycle():
    async with httpx.AsyncClient() as client:
        return await client.get("http://localhost:8000/simulation/next_cycle")

async def load_states():
    async with httpx.AsyncClient() as client:
        resp = await client.get(f"http://localhost:8000/simulation/load/{simulation_state['id']}")
        return resp.json() if resp.status_code == 200 else None

# Streamlit code

# Load and display the lists of assistants and narratives
assistants = run_async(load_assistants())
narratives = run_async(load_narratives())

# Check if the simulation has started
if "simulation_started" not in st.session_state:
    try:
        st.session_state.simulation_started
    except AttributeError:
        st.session_state.simulation_started = False

if "openai_model" not in st.session_state:
    st.session_state["openai_model"] = "gpt-3.5-turbo"

if "messages" not in st.session_state:
    st.session_state.messages = []

for message in st.session_state.messages:
    with st.chat_message(message["role"]):
        st.markdown(message["content"])
    
def button_start_callback():
    st.session_state.simulation_started=True

def button_stop_callback():
    st.session_state.simulation_started=False

if not st.session_state.simulation_started:
    st.markdown("<h1 style='text-align: center; color: grey;'>Myth Maker</h1>", unsafe_allow_html=True)

    st.markdown("<h6 style='text-align: center; color: black;'>This is a demo political simulation for modeling the impact of decision making.</h2>", unsafe_allow_html=True)
    if assistants is not None and narratives is not None:
        assistant_names = [assistant["name"] for assistant in assistants]
        narrative_names = [""] + [narrative["name"] for narrative in narratives]

        chosen_assistant_name = st.selectbox("Choose an assistant:", assistant_names)
        chosen_assistant = next((assistant for assistant in assistants if assistant["name"] == chosen_assistant_name), None)

        if chosen_assistant:
            st.text("Assistant Details:")
            st.text(f"Age: {chosen_assistant['age']}")
            st.text(f"Style: {chosen_assistant['style']}")
            st.text(f"Traits: {chosen_assistant['traits']}")
            st.text(f"Backstory: {chosen_assistant['backstory']}")
        
        chosen_narrative_name = st.selectbox("Choose a narrative:", narrative_names)
        chosen_narrative = next((narrative for narrative in narratives if narrative["name"] == chosen_narrative_name), None)

        if chosen_narrative:
            st.text(f"Narrative: {chosen_narrative['description']}")

        # Set the assistant and narrative when the user presses the "Start Simulation" button
        if st.button("Start Simulation"):
            assistant_choice = assistant_names.index(chosen_assistant_name) + 1
            narrative_choice = narrative_names.index(chosen_narrative_name) if chosen_narrative_name else ""
            if run_async(set_assistant(assistant_choice)) and (narrative_choice == '' or run_async(set_narrative(narrative_choice))):
                st.write(run_async(start_simulation()))
                st.session_state.simulation_started = True
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
        st.sidebar.title('Myth Maker')
        st.sidebar.markdown('This is a demo political simulation for modeling the impact of decision making.')
        if st.sidebar.button("Stop Simulation"):
            st.sidebar.write(run_async(stop_simulation()))
            st.session_state.simulation_started = False

        # Sidebar for Assistant, Narrative, and Influence
        st.sidebar.markdown('### Simulation')
        if 'assistant' in simulation_state:
            st.sidebar.text(f"Assistant: {simulation_state['assistant']['name']}, {simulation_state['assistant']['age']}")
        if 'narrative' in simulation_state:
            st.sidebar.text(f"Narrative: {simulation_state['narrative']}")
        if 'influence' in simulation_state:
            st.sidebar.text(f"Influence: {simulation_state['influence']}")
        if 'cycle' in simulation_state:
            st.sidebar.text(f"Cycle: {simulation_state['cycle']}")
        if st.sidebar.button("Next Cycle"):
            st.sidebar.write(run_async(next_cycle()))
        
        #st.json(simulation_state)

        # Sidebar
        st.sidebar.markdown('### Dashboards')
        view = st.sidebar.radio(
            'Select a view',
            ('Assistant','Ministers', 'Citizen Groups', 'Economic Sectors', 'Parameters', 'Metrics', 'Policies', 'Progression')
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
            decisions = simulation_state['decisions']
            selected_decision = st.selectbox("Choose a decision to implement:", decisions)
            if st.button("Implement Selected Policy"):
                decision_response = run_async(submit_decision(selected_decision))
                if decision_response:
                    st.write(decision_response)
        elif view == 'Progression':
            state_history = run_async(load_states())
            #st.json(state_history)
            
            # Serializing json
            #json_object = json.dumps(state_history, indent=4)
            
            # Writing to simulation.json
            #with open("simulation.json", "w") as outfile:
            #    outfile.write(json_object)

            # Initialize the dictionary
            data_dict = {"Cycle": [], "Economy": [], "Education": [], "Healthcare": [], "Quality of Life": [], "Public Unrest": [], "Overall Country Health": []}

            # Iterate over cycles
            for cycle in range(len(state_history['status'])):
                # Add cycle number
                data_dict["Cycle"].append(cycle)
                
                # Extract parameter values for each cycle
                parameters = state_history['status'][cycle][1]['parameters']
                metrics = state_history['status'][cycle][1]['metrics']
                data_dict["Economy"].append(parameters["Economy"]["value"])
                data_dict["Education"].append(parameters["Education"]["value"])
                data_dict["Healthcare"].append(parameters["Healthcare"]["value"])
                data_dict["Quality of Life"].append(parameters["Quality of Life"]["value"])
                data_dict["Public Unrest"].append(parameters["Public Unrest"]["value"])
                data_dict["Overall Country Health"].append(metrics["Overall Country Health"])

            # Convert the dictionary to a pandas DataFrame
            df = pd.DataFrame(data_dict)

            # create a line chart with cycle on the x-axis and the metrics on the y-axis
            st.line_chart(df.set_index("Cycle"))

        elif view == 'Assistant':
            if prompt := st.chat_input("Hello 👋"):
                st.session_state.messages.append({"role": "user", "content": prompt})
                with st.chat_message("user"):
                    st.markdown(prompt)

                with st.chat_message(simulation_state['assistant']['name']):
                    message_placeholder = st.empty()
                    full_response = ""
                    for response in openai.ChatCompletion.create(
                        model=st.session_state["openai_model"],
                        messages=[
                            {"role": m["role"], "content": m["content"]}
                            for m in st.session_state.messages
                        ],
                        stream=True,
                    ):
                        full_response += response.choices[0].delta.get("content", "")
                        message_placeholder.markdown(full_response + "▌")
                    message_placeholder.markdown(full_response)
                st.session_state.messages.append({"role": "assistant", "content": full_response})
    else:
        st.write("Error loading simulation state. Please check the backend service.")
    


