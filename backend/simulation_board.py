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
    
async def load_countries():
    async with httpx.AsyncClient() as client:
        response = await client.get("http://localhost:8000/load_countries")
        return response.json()["countries"] if response.status_code == 200 else None

async def set_assistant(choice):
    async with httpx.AsyncClient() as client:
        response = await client.get(f"http://localhost:8000/set_assistant/{choice}")
        return response.status_code == 200

async def set_narrative(choice):
    async with httpx.AsyncClient() as client:
        response = await client.get(f"http://localhost:8000/set_narrative/{choice}")
        return response.status_code == 200

async def set_country(choice):
    async with httpx.AsyncClient() as client:
        response = await client.get(f"http://localhost:8000/set_country/{choice}")
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
    
async def get_vote_share():
    async with httpx.AsyncClient() as client:
        resp = await client.get("http://localhost:8000/simulation/get_vote_share")
        return resp.json() if resp.status_code == 200 else None

async def submit_decision(decision):
    async with httpx.AsyncClient() as client:
        response = await client.post("http://localhost:8000/simulation/decision", json={"decision_name": decision})
        return response.json() if response.status_code == 200 else None

async def generate_response(query):
    async with httpx.AsyncClient() as client:
        response = await client.post("http://localhost:8000/simulation/generate_response", json={"query": query})
        return response.json()["response"] if response.status_code == 200 else None

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
countries = run_async(load_countries())

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

# Check if the chat visibility has been set in the session state
if "show_chat" not in st.session_state:
    st.session_state.show_chat = False

for message in st.session_state.messages:
    with st.chat_message(message["role"]):
        st.markdown(message["content"])
    
def button_start_callback():
    st.session_state.simulation_started=True

def button_stop_callback():
    st.session_state.simulation_started=False

if not st.session_state.simulation_started:
    st.markdown("<h1 style='text-align: center; color: blue;'>Myth Maker</h1>", unsafe_allow_html=True)

    st.markdown("<h6 style='text-align: center; color: black;'>Make history. Become myth.</h6>", unsafe_allow_html=True)
    if assistants is not None and narratives is not None:
        country_names = [country["name"] for country in countries]
        narrative_names = [""] + [narrative["name"] for narrative in narratives]
        assistant_names = [assistant["name"] for assistant in assistants]

        chosen_country_name = st.selectbox("Choose country:", country_names)
        chosen_country = next((country for country in countries if country["name"] == chosen_country_name), None)

        chosen_narrative_name = st.selectbox("Choose a narrative:", narrative_names)
        chosen_narrative = next((narrative for narrative in narratives if narrative["name"] == chosen_narrative_name), None)

        if chosen_narrative:
            st.write(f"{chosen_narrative['description']}")

        chosen_assistant_name = st.selectbox("Choose an assistant:", assistant_names)
        chosen_assistant = next((assistant for assistant in assistants if assistant["name"] == chosen_assistant_name), None)

        if chosen_assistant:
            col1, col2 = st.columns([1,2])
            with col1:
                st.image("../data/img/"+chosen_assistant['name']+".jpeg")
            with col2:
                st.write(f"Age: {chosen_assistant['age']}")
                st.write(f"Style: {chosen_assistant['style']}")
                st.write(f"Traits: {chosen_assistant['traits']}")
                st.write(f"{chosen_assistant['backstory']}")

        # Set the assistant and narrative when the user presses the "Start Simulation" button
        if st.button("Start Simulation"):
            country_choice = country_names.index(chosen_country_name) + 1
            narrative_choice = narrative_names.index(chosen_narrative_name) if chosen_narrative_name else ""
            assistant_choice = assistant_names.index(chosen_assistant_name) + 1
            if run_async(set_assistant(assistant_choice)) and run_async(set_country(country_choice)) and (narrative_choice == '' or run_async(set_narrative(narrative_choice))):
                st.write(run_async(start_simulation()))
                st.session_state.simulation_started = True
            else:
                st.write("Error starting simulation")
        
        st.write("---")
        st.write("<h6 style='text-align: center; color: gray;'>'A myth is far truer than a history, for a history only gives a story of the shadows, whereas a myth gives a story of the substances that cast the shadows.' -Annie Besant</h6>", unsafe_allow_html=True)
        # Most people live in a myth and grow violently angry if anyone dares to tell them the truth about themselves. Robert Anton Wilson
        # The great enemy of the truth is very often not the lie, deliberate, contrived and dishonest, but the myth, persistent, persuasive and unrealistic. John F. Kennedy
        # A myth is an image in terms of which we try to make sense of the world. Alan Watts
        # A myth is a way of making sense in a senseless world. Myths are narrative patterns that give significance to our existence. Rollo May
        # 'A myth is far truer than a history, for a history only gives a story of the shadows, whereas a myth gives a story of the substances that cast the shadows.' -Annie Besant
        # Myths which are believed in tend to become true. -Geonge Orwell
    else:
        st.write("Error loading assistants or narratives. Please check the backend service.")

# Display the rest of the dashboard after the game has been started
if st.session_state.simulation_started:

    simulation_state = run_async(get_simulation_state())['state']

    # Sidebar
    if simulation_state:
        # show a sidebar with the app title and description
        st.sidebar.markdown("<h1 style='text-align: left; color: blue;'>Myth Maker</h1>", unsafe_allow_html=True)
        st.sidebar.markdown("<h5 style='text-align: left; color: gray;'>Most people live in a myth and grow violently angry if anyone dares to tell them the truth about themselves.<br>--Robert Anton Wilson</h5>", unsafe_allow_html=True)


        # Sidebar for Assistant, Narrative, and Influence
        st.sidebar.markdown("### Simulation")
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
            ('Assistant', 'Policies', 'Ministers', 'Citizen Groups', 'Economic Sectors', 'Parameters', 'Metrics', 'Progression')
        )

    if st.sidebar.button("Stop Simulation"):
        st.sidebar.write(run_async(stop_simulation()))
        st.session_state.simulation_started = False  

    # Main Area
    if simulation_state:
        if view == 'Ministers':
            st.session_state.show_chat = False
            st.subheader('Ministers')
            st.write(simulation_state['ministers'])
        elif view == 'Citizen Groups':
            st.session_state.show_chat = False
            st.subheader('Citizen Groups')
            st.write(simulation_state['citizen_groups'])
        elif view == 'Economic Sectors':
            st.session_state.show_chat = False
            st.subheader('Economic Sectors')
            st.write(simulation_state['economic_sectors'])
        elif view == 'Parameters':
            st.session_state.show_chat = False
            st.subheader('Parameters')
            st.write(simulation_state['parameters'])
        elif view == 'Metrics':
            st.session_state.show_chat = False
            st.subheader('Metrics')
            st.write(simulation_state['metrics'])

            public_sentiment = run_async(get_vote_share())
            st.text("Public sentiment:")
            st.write(public_sentiment)

        elif view == 'Policies':
            st.session_state.show_chat = False
            st.subheader('Policies')
            decisions = simulation_state['decisions']
            selected_decision = st.selectbox("Choose a policy to implement:", decisions)
            if st.button("Implement Selected Policy"):
                decision_response = run_async(submit_decision(selected_decision))
                if decision_response:
                    st.write(decision_response)
        elif view == 'Progression':
            st.session_state.show_chat = False
            state_history = run_async(load_states())
            
            # Serializing json
            #json_object = json.dumps(state_history, indent=4)
            
            # Writing to simulation.json
            #with open("simulation.json", "w") as outfile:
            #    outfile.write(json_object)

            # Initialize the dictionary
            data_metrics = {"Cycle": [], "Overall Country Health": []}
            data_parameters = {"Cycle": [], "Economy": [], "Education": [], "Environment": [],"Healthcare": [], "Public Unrest": []}

            # Iterate over cycles
            for cycle in range(len(state_history['status'])):
                # Add cycle number
                data_metrics["Cycle"].append(cycle)
                data_parameters["Cycle"].append(cycle)

                # Extract metrics values for each cycle
                metrics = state_history['status'][cycle][1]['metrics']
                data_metrics["Overall Country Health"].append(metrics["Overall Country Health"])
                
                # Extract parameter values for each cycle
                parameters = state_history['status'][cycle][1]['parameters']
                data_parameters["Economy"].append(parameters["Economy"]["value"])
                data_parameters["Education"].append(parameters["Education"]["value"])
                data_parameters["Healthcare"].append(parameters["Healthcare"]["value"])
                data_parameters["Environment"].append(parameters["Environment"]["value"])
                #data_parameters["Quality of Life"].append(parameters["Quality of Life"]["value"])
                data_parameters["Public Unrest"].append(parameters["Public Unrest"]["value"])

            # Convert the dictionary to a pandas DataFrame
            df_metrics = pd.DataFrame(data_metrics)
            df_parameters = pd.DataFrame(data_parameters)

            # create a line chart with cycle on the x-axis and the metrics on the y-axis
            st.write("<center>Overall Country Health</center>", unsafe_allow_html=True)
            st.line_chart(df_metrics.set_index("Cycle"))
            st.write("<center>Specific parameters</center>", unsafe_allow_html=True)
            st.line_chart(df_parameters.set_index("Cycle"))

            st.json(state_history)

        elif view == 'Assistant':
            st.session_state.show_chat = True
            if st.session_state.show_chat:
                prompt = st.text_input("Enter your message:")
                if prompt:
                    st.session_state.messages.append({"role": "user", "content": prompt})
                    #with st.chat_message("user"):
                    st.markdown(prompt)
                    #with st.chat_message(simulation_state['assistant']['name']):
                    full_response = run_async(generate_response(prompt))
                    st.markdown(full_response)
                    st.session_state.messages.append({"role": "assistant", "content": full_response})
    else:
        st.write("Error loading simulation state. Please check the backend service.")
    


