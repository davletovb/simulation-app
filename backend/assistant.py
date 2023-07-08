from .simulation import Simulation, Transition, ParameterTransition, State


from langchain import OpenAI
from langchain.chat_models import ChatOpenAI
from langchain.experimental.generative_agents import GenerativeAgent, GenerativeAgentMemory
from langchain.retrievers import TimeWeightedVectorStoreRetriever
from langchain.vectorstores import Chroma
from langchain.embeddings import OpenAIEmbeddings
from chromadb.config import Settings

# Set Chroma settings
CHROMA_SETTINGS = Settings(
    chroma_db_impl="duckdb+parquet",
    persist_directory="db",
    anonymized_telemetry=False)

class Assistant:
    def __init__(self, name: str, age: int, status: str, traits: str):
        self.name = name
        self.age = age
        self.status = status
        self.traits = traits
        self.llm = OpenAI(model="text-davinci-003",max_tokens=1500)
        self.embeddings = OpenAIEmbeddings()
        self.vectorstore = Chroma(embedding_function=self.embeddings, client_settings=CHROMA_SETTINGS, persist_directory="db")
        self.memory_retriever = TimeWeightedVectorStoreRetriever(vectorstore=self.vectorstore, other_score_keys=["importance"], k=15)
        self.memory = GenerativeAgentMemory(llm=self.llm, reflection_threshold=8, memory_retriever=self.memory_retriever)
        self.agent = GenerativeAgent(
            name=self.name,
            age=self.age,
            status=self.status,
            traits=self.traits,
            llm=self.llm,
            memory=self.memory
        )

    def fetch_news(self):
        # Fetch news from the agent's memory
        relevant_events = self.agent.memory.retrieve_recent_memories()

        return relevant_events

    def generate_decision(self, news_event):
        # Generate a decision based on a news event
        decision = self.agent.generate_reaction(news_event)[1]

        return decision

    def generate_response(self, state: State, news_event: str = None):
        # Generate a different response based on the current narrative
        response = self.agent.generate_dialogue_response(news_event)[1]

        return response

    def process_input(self, input_text: str, state: State):
        # Use the agent to process the input
        _, response = self.agent.generate_dialogue_response(input_text)
        print(response)

    def __repr__(self) -> str:
        return str(self.agent.get_summary())

