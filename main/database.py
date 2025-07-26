import os
import google.generativeai as gen
from datetime import datetime
from dotenv import load_dotenv
from supabase import create_client, Client

load_dotenv()
SUPABASE_URL = os.getenv("SUPABASE_URL")
SUPABASE_KEY = os.getenv("SUPABASE_KEY")
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")

if not SUPABASE_URL or not SUPABASE_KEY:
    raise ValueError("Supabase credentials not found in .env")
if not GEMINI_API_KEY:
    raise ValueError("Gemini API key not found in .env")

supabase: Client = create_client(SUPABASE_URL, SUPABASE_KEY)
gen.configure(api_key=GEMINI_API_KEY)

conversation_buffers = {}
CHUNK_SIZE = 10

def generate_embedding(text):
    response = gen.embed_content(
        model="models/text-embedding-004",
        content=text,
        task_type="retrieval_document"
    )
    return response["embedding"]

def get_server_db(server_id):
    """Get messages for a specific server from Supabase"""
    response = supabase.table("message_chunks")\
        .select("*")\
        .eq("server_id", str(server_id))\
        .order("timestamp", desc=True)\
        .execute()
    
    if hasattr(response, "error") and response.error:
        print("❌ Error fetching from Supabase:", response.error)
        return []
    print(f'response.data: {response.data}')
    return response.data

def merge_conversation(server_id, channel_id, category, buffer_key):
    """Merge a conversation into a single message and store it in Supabase"""
    message_list = conversation_buffers.get(buffer_key, [])
    conversation_lines = []

    for msg in message_list:
        ts_str = msg["timestamp"].strftime("%Y-%m-%d %H:%M:%S") if isinstance(msg["timestamp"], datetime) else str(msg["timestamp"])
        conversation_lines.append(
            f"{msg['author']} (user_id: {msg['user_id']}) at {ts_str} said: {msg['content']}"
        )

    text_message = "\n".join(conversation_lines)
    embedding = generate_embedding(text_message)
    print(f'embedding: {embedding}')

    earliest_ts = message_list[0]["timestamp"] if message_list else datetime.utcnow()

    chunk_doc = {
        "server_id": str(server_id),
        "channel_id": channel_id,
        "text_message": text_message,
        "embedding": list(embedding),
        "timestamp": earliest_ts.isoformat(),
        "category": category,
        "message_count": len(message_list),
    }
    try:
        response = supabase.table("message_chunks").insert(chunk_doc).execute()
        if hasattr(response, "error") and response.error:
            print("❌ Error inserting into Supabase:", response.error)
    except Exception as e:
        print("❌ Error inserting into Supabase:", e)

    conversation_buffers[buffer_key] = []

def store_message(server_id, author, user_id, content, category, channel, server, timestamp):
    if not content.strip():
        return

    channel_id = str(channel.id)
    buffer_key = (server_id, channel_id)

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

    if len(conversation_buffers[buffer_key]) >= CHUNK_SIZE:
        merge_conversation(server_id, channel_id, category, buffer_key)
        print("✅ Conversation merged and stored in Supabase")
    else:
        print(f"did not merge, {len(conversation_buffers[buffer_key])} messages in buffer")
