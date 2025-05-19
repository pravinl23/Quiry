from qdrant_client import QdrantClient
from qdrant_client.http import models

def setup_qdrant_collection():
    # Connect to Qdrant
    client = QdrantClient("localhost", port=6333)
    
    # Delete collection if it exists
    try:
        client.delete_collection(collection_name="message_embeddings")
    except Exception:
        pass
    
    # Create collection
    client.create_collection(
        collection_name="message_embeddings",
        vectors_config=models.VectorParams(
            size=1536,
            distance=models.Distance.COSINE
        )
    )
    print("Qdrant collection 'message_embeddings' created successfully!")

if __name__ == "__main__":
    setup_qdrant_collection() 