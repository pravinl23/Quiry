import os
import google.generativeai as gen
from datetime import datetime
from dotenv import load_dotenv
from supabase import create_client, Client
import ast
import numpy as np

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

def store_individual_message(server_id, channel_id, author, user_id, content, category, server, timestamp):
    """Store an individual message in the database"""
    try:
        message_doc = {
            "server_id": str(server_id),
            "channel_id": str(channel_id),
            "author": author,
            "user_id": str(user_id),
            "content": content,
            "category": category or "No category",
            "server": server,
            "timestamp": timestamp.isoformat() if hasattr(timestamp, 'isoformat') else str(timestamp)
        }
        
        response = supabase.table("individual_messages").insert(message_doc).execute()
        if hasattr(response, "error") and response.error:
            print(f"❌ Error storing individual message: {response.error}")
            return False
        
        return True
        
    except Exception as e:
        print(f"❌ Error storing individual message: {e}")
        return False

def get_individual_messages_for_chunk(server_id, channel_id, limit=CHUNK_SIZE):
    """Get individual messages for creating a chunk"""
    try:
        response = supabase.table("individual_messages")\
            .select("*")\
            .eq("server_id", str(server_id))\
            .eq("channel_id", str(channel_id))\
            .order("timestamp", desc=False)\
            .limit(limit)\
            .execute()
        
        if hasattr(response, "error") and response.error:
            print(f"❌ Error fetching individual messages: {response.error}")
            return []
        
        return response.data or []
        
    except Exception as e:
        print(f"❌ Error fetching individual messages: {e}")
        return []

def delete_individual_messages(message_ids):
    """Delete individual messages by their IDs"""
    try:
        for msg_id in message_ids:
            response = supabase.table("individual_messages")\
                .delete()\
                .eq("id", msg_id)\
                .execute()
            
            if hasattr(response, "error") and response.error:
                print(f"❌ Error deleting individual message {msg_id}: {response.error}")
                return False
        
        return True
        
    except Exception as e:
        print(f"❌ Error deleting individual messages: {e}")
        return False

def count_individual_messages(server_id, channel_id):
    """Count individual messages for a specific server and channel"""
    try:
        response = supabase.table("individual_messages")\
            .select("id", count="exact")\
            .eq("server_id", str(server_id))\
            .eq("channel_id", str(channel_id))\
            .execute()
        
        if hasattr(response, "error") and response.error:
            print(f"❌ Error counting individual messages: {response.error}")
            return 0
        
        return response.count or 0
        
    except Exception as e:
        print(f"❌ Error counting individual messages: {e}")
        return 0

def clear_all_db():
    """Clear ALL message chunks and individual messages from both databases (all servers)"""
    try:
        print("🗑️ Starting to clear ALL data from both databases...")
        
        # Clear all message chunks
        response1 = supabase.table("message_chunks")\
            .delete()\
            .neq("id", 0)\
            .execute()  # Delete all rows (neq id 0 means all rows since id is never 0)
        
        # Clear all individual messages
        response2 = supabase.table("individual_messages")\
            .delete()\
            .neq("id", 0)\
            .execute()  # Delete all rows
        
        if (hasattr(response1, "error") and response1.error) or (hasattr(response2, "error") and response2.error):
            print(f"❌ Error clearing all databases")
            if hasattr(response1, "error") and response1.error:
                print(f"❌ Message chunks error: {response1.error}")
            if hasattr(response2, "error") and response2.error:
                print(f"❌ Individual messages error: {response2.error}")
            return False
        
        print(f"🗑️ Successfully cleared ALL data from both databases")
        print(f"   - Cleared message_chunks table")
        print(f"   - Cleared individual_messages table")
        return True
        
    except Exception as e:
        print(f"❌ Error clearing all databases: {e}")
        return False

def clear_server_db(server_id):
    """Clear all message chunks and individual messages for a specific server from Supabase"""
    try:
        # Clear message chunks
        response1 = supabase.table("message_chunks")\
            .delete()\
            .eq("server_id", str(server_id))\
            .execute()
        
        # Clear individual messages
        response2 = supabase.table("individual_messages")\
            .delete()\
            .eq("server_id", str(server_id))\
            .execute()
        
        if (hasattr(response1, "error") and response1.error) or (hasattr(response2, "error") and response2.error):
            print(f"❌ Error clearing database for server {server_id}")
            return False
        
        print(f"🗑️ Cleared all existing data for server {server_id}")
        return True
        
    except Exception as e:
        print(f"❌ Error clearing database for server {server_id}: {e}")
        return False

def get_individual_messages_for_retrieval(server_id):
    """Get individual messages that haven't been chunked yet for retrieval"""
    try:
        response = supabase.table("individual_messages")\
            .select("*")\
            .eq("server_id", str(server_id))\
            .order("timestamp", desc=False)\
            .execute()
        
        if hasattr(response, "error") and response.error:
            print(f"❌ Error fetching individual messages: {response.error}")
            return []
        
        return response.data or []
        
    except Exception as e:
        print(f"❌ Error fetching individual messages: {e}")
        return []

def get_server_db(server_id):
    """Get messages for a specific server from Supabase (chunks + individual messages)"""
    # Get chunked messages
    chunks_response = supabase.table("message_chunks")\
        .select("*")\
        .eq("server_id", str(server_id))\
        .order("timestamp", desc=True)\
        .execute()
    
    if hasattr(chunks_response, "error") and chunks_response.error:
        print("❌ Error fetching chunks from Supabase:", chunks_response.error)
        chunks_data = []
    else:
        chunks_data = chunks_response.data or []
    
    # Process chunk embeddings
    if chunks_data:
        for doc in chunks_data:
            if isinstance(doc["embedding"], str):
                doc["embedding"] = ast.literal_eval(doc["embedding"])
    
    # Get individual messages that haven't been chunked yet
    individual_messages = get_individual_messages_for_retrieval(server_id)
    
    # Convert individual messages to chunk-like format for retrieval
    individual_chunks = []
    if individual_messages:
        for msg in individual_messages:
            # Create a text message in the same format as chunks
            ts_str = msg["timestamp"] if isinstance(msg["timestamp"], str) else msg["timestamp"].strftime("%Y-%m-%d %H:%M:%S")
            text_message = f"{msg['author']} (user_id: {msg['user_id']}) at {ts_str} said: {msg['content']}"
            
            # Generate embedding for this individual message
            try:
                embedding = generate_embedding(text_message)
                individual_chunk = {
                    "id": f"individual_{msg['id']}",  # Prefix to distinguish from chunks
                    "server_id": msg["server_id"],
                    "channel_id": msg["channel_id"],
                    "text_message": text_message,
                    "embedding": embedding,
                    "timestamp": msg["timestamp"],
                    "category": msg["category"],
                    "message_count": 1
                }
                individual_chunks.append(individual_chunk)
            except Exception as e:
                print(f"❌ Error generating embedding for individual message: {e}")
                continue
    
    # Combine chunks and individual messages
    all_messages = chunks_data + individual_chunks
    
    print(f"📊 Retrieved {len(chunks_data)} chunks and {len(individual_chunks)} individual messages for server {server_id}")
    
    return all_messages

async def fetch_server_history(bot, server_id, limit):
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
    
    # Store the individual message first
    if not store_individual_message(server_id, channel_id, author, user_id, content, category, server, timestamp):
        print(f"❌ Failed to store individual message")
        return
    
    # Count how many individual messages we have for this channel
    message_count = count_individual_messages(server_id, channel_id)
    print(f"📝 Message stored individually: {message_count}/{CHUNK_SIZE} messages in channel")
    
    # If we have reached exactly the chunk size, create a chunk and delete individual messages
    if message_count == CHUNK_SIZE:
        # Get the individual messages for this chunk
        individual_messages = get_individual_messages_for_chunk(server_id, channel_id, CHUNK_SIZE)
        
        if len(individual_messages) == CHUNK_SIZE:
            # Convert individual messages to the format expected by store_message_chunk
            message_chunk = []
            message_ids_to_delete = []
            
            for msg in individual_messages:  # Take all CHUNK_SIZE messages
                message_chunk.append({
                    "author": msg["author"],
                    "user_id": int(msg["user_id"]),
                    "content": msg["content"],
                    "timestamp": datetime.fromisoformat(msg["timestamp"].replace('Z', '+00:00')) if isinstance(msg["timestamp"], str) else msg["timestamp"],
                    "category": msg["category"],
                    "server": msg["server"]
                })
                message_ids_to_delete.append(msg["id"])
            
            # Create the chunk synchronously (since we're not in an async context)
            if create_chunk_from_messages(server_id, channel_id, message_chunk):
                # Only delete individual messages if chunk creation was successful
                if delete_individual_messages(message_ids_to_delete):
                    print(f"✅ Chunk of {CHUNK_SIZE} messages created and {len(message_ids_to_delete)} individual messages wiped from database")
                else:
                    print(f"⚠️ Chunk created but failed to wipe some individual messages from database")
            else:
                print(f"❌ Failed to create chunk, keeping individual messages")

def create_chunk_from_messages(server_id, channel_id, message_chunk):
    """Create a chunk from individual messages (synchronous version)"""
    try:
        # Create conversation lines
        conversation_lines = []
        for msg in message_chunk:
            ts_str = msg["timestamp"].strftime("%Y-%m-%d %H:%M:%S") if hasattr(msg["timestamp"], 'strftime') else str(msg["timestamp"])
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
            "timestamp": earliest_ts.isoformat() if hasattr(earliest_ts, 'isoformat') else str(earliest_ts),
            "category": message_chunk[0]["category"],
            "message_count": len(message_chunk),
        }
        
        # Insert the chunk
        response = supabase.table("message_chunks").insert(chunk_doc).execute()
        if hasattr(response, "error") and response.error:
            print(f"❌ Error inserting chunk: {response.error}")
            return False
        else:
            print(f"✅ Stored chunk: {len(message_chunk)} messages from channel {channel_id}")
            return True
            
    except Exception as e:
        print(f"❌ Error creating chunk from messages: {e}")
        return False
