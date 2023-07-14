from langchain import LLMChain, OpenAI, ConversationChain, SQLDatabase, SQLDatabaseChain
from langchain.agents import initialize_agent, load_tools, Tool
from langchain.agents import AgentType
from langchain.chat_models import ChatOpenAI
from langchain.memory import ConversationBufferMemory, SQLiteEntityStore
from langchain.prompts import PromptTemplate


class Agent:
    def __init__(self):
        self.llm = ChatOpenAI(temperature=0, model="gpt-3.5-turbo")
        self.llm_chain = LLMChain.from_llm(llm=self.llm, verbose=True)
        #self.db = SQLDatabase.from_uri("sqlite:///../simulation.db")
        #self.db_chain = SQLDatabaseChain.from_llm(self.llm, self.db, verbose=True)
    
    # Prompt the LLM to generate a response
    def generate_response(self, message, chat_context):

        # Format the chat history as a string
        formatted_chat_history = "\n".join([f"{k}: {v}" for entry in chat_context for k, v in entry.items()])

        # Create a prompt template
        template = f"""
        Chat History:
        {formatted_chat_history}

        Human: {message}
        AI:"""

        # Create a prompt template
        #prompt_template = PromptTemplate(template=template, validate_template=False)

        # A conversation buffer (memory) & import llm of choice
        #memory = ConversationBufferMemory(memory_key="chat_history", input_key="input")

        # Create a model, chain and tool for the language model
        llm = ChatOpenAI(temperature=0, 
                        streaming=True,
                        max_retries=3)

        #llm_chain = ConversationChain(llm=llm, prompt=prompt_template)

        # Provide access to a list of tools that the agents will use
        # add 'open-meteo-api' to the list of tools later
        tools = load_tools(['llm-math','wikipedia'], llm=llm)
        
        tools.extend([
            Tool(
                name="database",
                func=self.db_chain.run,
                description="useful for when you need to answer questions about simulation state, history and data."
            )
        ])
        

        # initialise the agents & make all the tools and llm available to it
        agent = initialize_agent(tools=tools,
                                llm=llm, 
                                agent=AgentType.CONVERSATIONAL_REACT_DESCRIPTION, 
                                verbose=True,
                                handle_parsing_errors="Check your output and make sure it conforms!")

        try:
            answer = agent.run(input=template, chat_history=formatted_chat_history, return_only_outputs=True)
            return answer
        except Exception as e:
            return "An error occurred while generating the response."