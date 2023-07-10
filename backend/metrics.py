class Metric:
    def __init__(self, name: str, calculation_function):
        self.name = name
        self.calculation_function = calculation_function

    def calculate(self, state):
        return self.calculation_function(state)

def calculate_overall_country_health(state):
    return sum(state.parameters[name].value for name in ['Economy', 'Healthcare', 'Defense', 'Education', 'Infrastructure', 'Environment', 'Technology']) - state.parameters['Public Unrest'].value + state.parameters['Diplomacy'].value

def calculate_economic_stability(state):
    return state.parameters['Economy'].value - state.parameters['Public Unrest'].value

def calculate_public_welfare(state):
    return sum(state.parameters[name].value for name in ['Healthcare', 'Education', 'Infrastructure'])

def calculate_defense_readiness(state):
    return state.parameters['Defense'].value - 0.5 * state.parameters['Public Unrest'].value

def calculate_educational_quality(state):
    return state.parameters['Education'].value + 0.25 * state.parameters['Technology'].value

def calculate_infrastructure_development(state):
    return state.parameters['Infrastructure'].value + 0.25 * state.parameters['Economy'].value

def calculate_environmental_health(state):
    return state.parameters['Environment'].value - state.parameters['Economy'].value

def calculate_technological_progress(state):
    return state.parameters['Technology'].value + 0.25 * state.parameters['Education'].value

def calculate_diplomatic_relations(state):
    return state.parameters['Diplomacy'].value - state.parameters['Defense'].value

def calculate_public_sentiment(state):
    return 100 - state.parameters['Public Unrest'].value

def calculate_religious_harmony(state):
    return state.parameters['Religion'].value - state.parameters['Public Unrest'].value

def calculate_media_influence(state):
    return state.parameters['Media'].value + 0.5 * state.parameters['Public Unrest'].value

def calculate_social_cohesion(state):
    return sum(state.parameters[name].value for name in ['Religion', 'Education', 'Media']) - state.parameters['Public Unrest'].value

def calculate_quality_of_life(state):
    return sum(state.parameters[name].value for name in ['Healthcare', 'Economy', 'Education', 'Environment', 'Infrastructure']) / 5

def calculate_income_equality(state):
    return state.parameters['Economy'].value - state.parameters['Public Unrest'].value

def calculate_health_index(state):
    return state.parameters['Healthcare'].value + 0.25 * state.parameters['Environment'].value

def calculate_literacy_rate(state):
    return state.parameters['Education'].value + 0.25 * state.parameters['Media'].value

def calculate_internet_access(state):
    return state.parameters['Technology'].value + 0.25 * state.parameters['Infrastructure'].value

def calculate_research_and_development(state):
    return state.parameters['Technology'].value + 0.5 * state.parameters['Education'].value

def calculate_air_and_water_quality(state):
    return state.parameters['Environment'].value + 0.25 * state.parameters['Infrastructure'].value

# Define instances for each metric
metric_objects = [
    Metric("Overall Country Health", calculate_overall_country_health),
    Metric("Economic Stability", calculate_economic_stability),
    Metric("Public Welfare", calculate_public_welfare),
    Metric("Defense Readiness", calculate_defense_readiness),
    Metric("Educational Quality", calculate_educational_quality),
    Metric("Infrastructure Development", calculate_infrastructure_development),
    Metric("Environmental Health", calculate_environmental_health),
    Metric("Technological Progress", calculate_technological_progress),
    Metric("Diplomatic Relations", calculate_diplomatic_relations),
    Metric("Public Sentiment", calculate_public_sentiment),
    Metric("Religious Harmony", calculate_religious_harmony),
    Metric("Media Influence", calculate_media_influence),
    Metric("Social Cohesion", calculate_social_cohesion),
    Metric("Quality of Life", calculate_quality_of_life),
    Metric("Income Equality", calculate_income_equality),
    Metric("Health Index", calculate_health_index),
    Metric("Literacy Rate", calculate_literacy_rate),
    Metric("Internet Access", calculate_internet_access),
    Metric("Research and Development", calculate_research_and_development),
    Metric("Air and Water Quality", calculate_air_and_water_quality)
]

def set_metrics_values(state):
    state.set_metrics({metric.name: metric for metric in metric_objects})


