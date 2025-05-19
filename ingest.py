import os
import json
import uuid
from datetime import datetime
import psycopg2
from psycopg2.extras import Json
from qdrant_client import QdrantClient
from qdrant_client.http import models
import openai
from dotenv import load_dotenv

# Load environment variables
load_dotenv()

# Initialize clients
pg_conn = psycopg2.connect(
    dbname="messages",
    user="postgres",
    password="postgres",
    host="localhost",
    port="5432"
)
qdrant_client = QdrantClient("localhost", port=6333)
openai.api_key = os.getenv("OPENAI_API_KEY")

def generate_embedding(text: str) -> list:
    """Generate embedding using OpenAI's API."""
    response = openai.Embedding.create(
        input=text,
        model="text-embedding-3-small"
    )
    return response['data'][0]['embedding']

def insert_message(message_data: dict) -> uuid.UUID:
    """Insert message into Postgres and return message_id."""
    with pg_conn.cursor() as cur:
        cur.execute("""
            INSERT INTO messages (sent_at, user_id, server_id, channel_id, content)
            VALUES (%s, %s, %s, %s, %s)
            RETURNING message_id
        """, (
            message_data['sent_at'],
            message_data['user_id'],
            message_data['server_id'],
            message_data['channel_id'],
            message_data['content']
        ))
        message_id = cur.fetchone()[0]
        pg_conn.commit()
        return message_id

def upsert_vector(message_id: uuid.UUID, embedding: list):
    """Upsert vector into Qdrant."""
    qdrant_client.upsert(
        collection_name="message_embeddings",
        points=[
            models.PointStruct(
                id=str(message_id),
                vector=embedding
            )
        ]
    )

def process_message(message_data: dict):
    """Process a single message: store in Postgres and Qdrant."""
    # Generate embedding
    embedding = generate_embedding(message_data['content'])
    
    # Insert into Postgres
    message_id = insert_message(message_data)
    
    # Upsert into Qdrant
    upsert_vector(message_id, embedding)
    
    return message_id

def main():
    # Sample message for testing
    sample_message = {
        'sent_at': datetime.utcnow(),
        'user_id': uuid.uuid4(),
        'server_id': uuid.uuid4(),
        'channel_id': uuid.uuid4(),
        'content': "This is a test message for the vector database pipeline."
    }
    
    # Process the sample message
    message_id = process_message(sample_message)
    print(f"Processed message with ID: {message_id}")

if __name__ == "__main__":
    main() 