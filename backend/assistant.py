from agent import Agent

class Assistant:
    def __init__(self, name: str, age: int, style: str, traits: str, backstory: str):
        self.name = name
        self.age = age
        self.style = style
        self.traits = traits
        self.backstory = backstory
        self.agent = Agent({"name":self.name,
                            "age": self.age,
                            "style": self.style,
                            "traits": self.traits,
                            "backstory": self.backstory})

    def fetch_news(self):
        # Fetch news from the agent's memory
        relevant_events = self.agent.generate_response("Fetch news")

        return relevant_events

    def generate_decision(self, news_event):
        # Generate a decision based on a news event
        decision = self.agent.generate_response(news_event)

        return decision

    def generate_response(self, state_history, query):
        # Generate a different response based on the current narrative
        response = self.agent.generate_response(state_history, query)
        return response

    def process_input(self, input_text: str):
        # Use the agent to process the input
        response = self.agent.generate_response(input_text)
        print(response)

    def __repr__(self) -> str:
        #return str(self.agent.get_summary())
        return self.name
    
    def to_dict(self):
        # Create a dictionary with the same attributes as the State object
        state_dict = self.__dict__.copy()
        
        state_dict["agent"]={}

        return state_dict
    
    @classmethod
    def from_dict(cls, assistant_dict):
        # create a new instance of the class
        assistant = cls()

        # populate the attributes of the assistant object from the dictionary
        # this assumes that the keys in the dictionary match the attribute names of the Assistant object
        for key, value in assistant_dict.items():
            setattr(assistant, key, value)

        return assistant

