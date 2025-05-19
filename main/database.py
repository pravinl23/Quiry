import os
import certifi
# import google.generativeai as gen
import ollama
from pymongo import MongoClient
from dotenv import load_dotenv
from datetime import datetime

load_dotenv()
MONGO_URI = os.getenv("MONGO_URI")
# GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")

"""
# Checking if URI is valid 
if not MONGO_URI:
    raise ValueError("MONGO_URI not found")
if not GEMINI_API_KEY:
    raise ValueError("GEMINI_API_KEY not found")
"""

mongo_client = MongoClient(MONGO_URI, tlsCAFile=certifi.where(), serverSelectionTimeoutMS=5000)

conversation_buffers = {}
CHUNK_SIZE = 10

def get_server_db(server_id):
    return mongo_client[f"discord_server_{server_id}"]

def generate_embedding(text):
    response = ollama.embeddings(model='nomic-embed-text', prompt=text)
    return response['embedding']


# Merge the messages in the buffer into one, and add it to the mongoDB database
def merge_conversation(server_id, channel_id, category, buffer_key):

    message_list = conversation_buffers.get(buffer_key, [])
    
    # Loop through each message in the buffer
    conversation_lines = []
    for msg in message_list:
        # Format the time
        if isinstance(msg["timestamp"], datetime):
            ts_str = msg["timestamp"].strftime("%Y-%m-%d %H:%M:%S")
        else:
            ts_str = str(msg["timestamp"])
        # Example export: "pppravin" user_id: 3948390258081581 at timestamp: 3:45 said: hi how are you!
        conversation_lines.append(
            f"{msg['author']} (user_id: {msg['user_id']}) at timestamp:{ts_str} said: {msg['content']}"
        )

    text_message = "\n".join(conversation_lines)

    # Embed the chunk as one text message
    embedding = generate_embedding(text_message)

    db = get_server_db(server_id)
    collection = db["messages"]

    # Get the earliest timestamp for timestamp
    earliest_ts = message_list[0]["timestamp"] if message_list else datetime.utcnow()

    chunk_doc = {
        "server_id": server_id,
        "channel_id": channel_id,
        "text_message": text_message,
        "embedding": embedding,
        "timestamp": earliest_ts,
        "category": category,
        "message_count": len(message_list),
    }
    collection.insert_one(chunk_doc)

    # Clear the buffer for this server and channel
    conversation_buffers[buffer_key] = []

# Stores messages into the conversation buffer
'''***Need to implement filter for harmful words***'''
def store_message(server_id, author, user_id, content, category, channel, server, timestamp):
    # Ignore whitespaces
    if not content.strip():
        return

    # Channel ID is a unique number to the specific channel in that specific server 
    channel_id = str(channel.id)
    buffer_key = (server_id, channel_id)

    # Initialize conversation buffer
    if buffer_key not in conversation_buffers:
        conversation_buffers[buffer_key] = []

    conversation_buffers[buffer_key].append({
        "author": author,
        "user_id": user_id,
        "content": content,
        "timestamp": timestamp,
        "category": category or "No category",
        "server": server
    })

    # Check if the number of messages in the buffer has reached or exceeded the CHUNK_SIZE.
    if len(conversation_buffers[buffer_key]) >= CHUNK_SIZE:
        #print("Buffer reached CHUNK_SIZE for", buffer_key, "- flushing chunk")
        # Now combine the buffer into one message to store on mongodb
        merge_conversation(server_id, channel_id, category, buffer_key)
