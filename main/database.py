import os
import google.generativeai as gen
from datetime import datetime
from dotenv import load_dotenv
from supabase import create_client, Client
import ast
import numpy as np
import discord

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

    if response.data:
        for doc in response.data:
            if isinstance(doc["embedding"], str):
                doc["embedding"] = ast.literal_eval(doc["embedding"])
                print(f'embedding type: {type(doc["embedding"])}')
        print(f'embedding type: {type(response.data[0]["embedding"])}')

    return response.data

async def fetch_server_history(bot, server_id, limit=1000):
    """
    Fetch all previous messages from a Discord server and store them in chunks.
    
    Args:
        bot: Discord bot instance
        server_id: ID of the server to fetch history from
        limit: Maximum number of messages to fetch per channel (default 1000)
    """
    try:
        # Get the server (guild)
        guild = bot.get_guild(int(server_id))
        if not guild:
            print(f"❌ Server {server_id} not found")
            return
        
        print(f"🔄 Starting to fetch history for server: {guild.name}")
        
        total_messages = 0
        total_chunks = 0
        
        # Iterate through all text channels
        for channel in guild.text_channels:
            try:
                print(f"📝 Fetching messages from #{channel.name}...")
                
                # Fetch messages from this channel
                messages = []
                async for message in channel.history(limit=limit):
                    # Skip bot messages and empty messages
                    if message.author.bot or not message.content.strip():
                        continue
                    
                    messages.append({
                        "author": str(message.author),
                        "user_id": message.author.id,
                        "content": message.content,
                        "timestamp": message.created_at,
                        "category": channel.category.name if channel.category else "No Category",
                        "server": str(guild.name)
                    })
                
                print(f"📊 Found {len(messages)} messages in #{channel.name}")
                
                # Process messages in chunks of CHUNK_SIZE
                for i in range(0, len(messages), CHUNK_SIZE):
                    chunk = messages[i:i + CHUNK_SIZE]
                    if len(chunk) == CHUNK_SIZE:  # Only store complete chunks
                        await store_message_chunk(server_id, channel.id, chunk)
                        total_chunks += 1
                
                total_messages += len(messages)
                
            except Exception as e:
                print(f"❌ Error fetching from #{channel.name}: {e}")
                continue
        
        print(f"✅ Completed! Processed {total_messages} messages in {total_chunks} chunks")
        
    except Exception as e:
        print(f"❌ Error fetching server history: {e}")

async def store_message_chunk(server_id, channel_id, message_chunk):
    """
    Store a chunk of messages in the database.
    
    Args:
        server_id: Discord server ID
        channel_id: Discord channel ID
        message_chunk: List of message dictionaries
    """
    try:
        # Create conversation text from chunk
        conversation_lines = []
        for msg in message_chunk:
            ts_str = msg["timestamp"].strftime("%Y-%m-%d %H:%M:%S")
            conversation_lines.append(
                f"{msg['author']} (user_id: {msg['user_id']}) at {ts_str} said: {msg['content']}"
            )
        
        text_message = "\n".join(conversation_lines)
        embedding = generate_embedding(text_message)
        
        # Use timestamp of first message in chunk
        earliest_ts = message_chunk[0]["timestamp"]
        
        chunk_doc = {
            "server_id": str(server_id),
            "channel_id": str(channel_id),
            "text_message": text_message,
            "embedding": list(embedding),
            "timestamp": earliest_ts.isoformat(),
            "category": message_chunk[0]["category"],
            "message_count": len(message_chunk),
        }
        
        # Check if this chunk already exists (avoid duplicates)
        existing = supabase.table("message_chunks")\
            .select("id")\
            .eq("server_id", str(server_id))\
            .eq("channel_id", str(channel_id))\
            .eq("timestamp", earliest_ts.isoformat())\
            .execute()
        
        if not existing.data:
            response = supabase.table("message_chunks").insert(chunk_doc).execute()
            if hasattr(response, "error") and response.error:
                print(f"❌ Error inserting chunk: {response.error}")
            else:
                print(f"✅ Stored chunk: {len(message_chunk)} messages from channel {channel_id}")
        else:
            print(f"⏭️ Skipped duplicate chunk: {len(message_chunk)} messages from channel {channel_id}")
            
    except Exception as e:
        print(f"❌ Error storing message chunk: {e}")

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

    # Initialize buffer if it doesn't exist
    if buffer_key not in conversation_buffers:
        conversation_buffers[buffer_key] = []

    # Add message to buffer
    conversation_buffers[buffer_key].append({
        "author": author,
        "user_id": user_id,
        "content": content,
        "timestamp": timestamp,
        "category": category or "No category",
        "server": server
    })

    # Only store when we have exactly 10 messages
    if len(conversation_buffers[buffer_key]) == CHUNK_SIZE:
        merge_conversation(server_id, channel_id, category, buffer_key)
        print(f"✅ Chunk of {CHUNK_SIZE} messages stored in Supabase")
    else:
        print(f"📝 Message buffered: {len(conversation_buffers[buffer_key])}/{CHUNK_SIZE} in chunk")
