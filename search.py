import os
import uuid
from typing import List, Dict, Any
import psycopg2
from psycopg2.extras import RealDictCursor
from qdrant_client import QdrantClient
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

def search_messages(
    query: str,
    limit: int = 5,
    user_id: uuid.UUID = None,
    server_id: uuid.UUID = None,
    channel_id: uuid.UUID = None
) -> List[Dict[str, Any]]:
    """
    Perform hybrid search combining vector similarity with relational filters.
    
    Args:
        query: The search query text
        limit: Maximum number of results to return
        user_id: Optional filter by user
        server_id: Optional filter by server
        channel_id: Optional filter by channel
    
    Returns:
        List of message dictionaries with their similarity scores
    """
    # Generate query embedding
    query_embedding = generate_embedding(query)
    
    # Build filter conditions
    filter_conditions = []
    if user_id:
        filter_conditions.append(f"user_id = '{user_id}'")
    if server_id:
        filter_conditions.append(f"server_id = '{server_id}'")
    if channel_id:
        filter_conditions.append(f"channel_id = '{channel_id}'")
    
    # Search in Qdrant
    search_result = qdrant_client.search(
        collection_name="message_embeddings",
        query_vector=query_embedding,
        limit=limit,
        query_filter=models.Filter(
            must=[
                models.FieldCondition(
                    key=field,
                    match=models.MatchValue(value=value)
                ) for field, value in [
                    ("user_id", str(user_id)) if user_id else None,
                    ("server_id", str(server_id)) if server_id else None,
                    ("channel_id", str(channel_id)) if channel_id else None
                ] if value is not None
            ]
        ) if any([user_id, server_id, channel_id]) else None
    )
    
    # Get message IDs from search results
    message_ids = [result.id for result in search_result]
    
    # Fetch full message data from Postgres
    with pg_conn.cursor(cursor_factory=RealDictCursor) as cur:
        cur.execute("""
            SELECT m.*, 
                   (SELECT score FROM unnest(%s::uuid[], %s::float[]) AS t(id, score)
                    WHERE t.id = m.message_id) as similarity_score
            FROM messages m
            WHERE m.message_id = ANY(%s)
            ORDER BY similarity_score DESC
        """, (message_ids, [result.score for result in search_result], message_ids))
        
        results = cur.fetchall()
        return [dict(row) for row in results]

def main():
    # Example search
    query = "test message"
    results = search_messages(query, limit=5)
    
    print(f"\nSearch results for query: '{query}'")
    print("-" * 50)
    for result in results:
        print(f"Score: {result['similarity_score']:.4f}")
        print(f"Content: {result['content']}")
        print(f"User: {result['user_id']}")
        print(f"Sent at: {result['sent_at']}")
        print("-" * 50)

if __name__ == "__main__":
    main() 