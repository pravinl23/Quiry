import google.generativeai as gen
import os
from dotenv import load_dotenv
load_dotenv()
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")

gen.configure(api_key=GEMINI_API_KEY)

models = gen.list_models()

print("Available Models:")
for model in models:
    print(model.name)
