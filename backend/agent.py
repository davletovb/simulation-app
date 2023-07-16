from langchain import LLMChain, OpenAI, ConversationChain, SQLDatabase, SQLDatabaseChain
from langchain.agents import initialize_agent, load_tools, AgentType, Tool, ZeroShotAgent
from langchain.chat_models import ChatOpenAI
from langchain.memory import ConversationBufferMemory, SQLiteEntityStore
from langchain.prompts import PromptTemplate


class Agent:
    def __init__(self, assistant_details={"name": "Ava", "age": "27", "style": "funny, excited, disciplined", "traits": "methodical, disciplined, concise", "backstory": "Ava was raised in a small town."}):

        self.assistant_details = assistant_details

        #self.llm_chain = LLMChain(llm=self.llm, prompt=self.prompt, memory=self.memory, verbose=True)
        #self.db = SQLDatabase.from_uri("sqlite:///Users/user/Documents/simulation-app/simulation.db")
        #self.db_chain = SQLDatabaseChain.from_llm(self.llm, self.db, verbose=True)
    
    # Prompt the LLM to generate a response
    def generate_response(self, query):

        llm = OpenAI(model="text-davinci-003", temperature=0)

        tools = load_tools(['llm-math','wikipedia'], llm=llm)

        template = """
        You are an assistant to the leader of the country. 
        Your name is {name}. 
        You are {age} years old. 
        You are {style}. 
        Your traits are {traits}.
        Your backstory: {backstory}
        Chat history: {chat_history} 
        Leader: {input} 
        {name}:""".format(name = self.assistant_details["name"],
                          age = self.assistant_details["age"],
                          style = self.assistant_details["style"],
                          traits = self.assistant_details["traits"],
                          backstory = self.assistant_details["backstory"],
                          input="{input}",
                          chat_history="{chat_history}")

        prompt = PromptTemplate(template=template, input_variables=['chat_history','input'])
        
        memory = ConversationBufferMemory(memory_key="chat_history",
                                          input_key="input",
                                          ai_prefix=self.assistant_details["name"],
                                          human_prefix="Leader")

        llm_chain = ConversationChain(llm=llm, prompt=prompt, memory=memory, output_key="output")

        """
        tools.extend([
            Tool(
                name="database",
                func=self.db_chain.run,
                description="useful for when you need to answer questions about simulation state, history and data."
            )
        ])
        """

        # initialise the agents & make all the tools and llm available to it
        agent = initialize_agent(tools=tools,
                                llm=llm, 
                                agent=AgentType.CONVERSATIONAL_REACT_DESCRIPTION, 
                                verbose=True,
                                prompt=prompt,
                                memory=memory,
                                handle_parsing_errors="Check your output and make sure it conforms!")

        try:
            #answer = agent({"input": query})["output"]
            answer = llm_chain({'input': query})["output"]
            return answer
        except Exception as e:
            return "An error occurred while generating the response. "+str(e)