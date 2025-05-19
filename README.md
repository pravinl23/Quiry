# Hybrid Vector + Relational Message Search

This project implements a hybrid search system for chat messages using PostgreSQL for relational data and Qdrant for vector similarity search. It combines the benefits of both traditional SQL queries and semantic search capabilities.

## Prerequisites

- Docker and Docker Compose
- Python 3.8+
- OpenAI API key

## Setup

1. Clone the repository and navigate to the project directory.

2. Create a `.env` file with your OpenAI API key:

```
OPENAI_API_KEY=your_api_key_here
```

3. Start the required services using Docker Compose:

```bash
docker-compose up -d
```

4. Install Python dependencies:

```bash
pip install -r requirements.txt
```

5. Initialize the database schema:

```bash
psql -h localhost -U postgres -d messages -f migrations/0001_init.sql
```

6. Set up the Qdrant collection:

```bash
python setup_qdrant.py
```

## Usage

### Ingesting Messages

To ingest a new message into the system:

```python
from ingest import process_message
from datetime import datetime
import uuid

message_data = {
    'sent_at': datetime.utcnow(),
    'user_id': uuid.uuid4(),
    'server_id': uuid.uuid4(),
    'channel_id': uuid.uuid4(),
    'content': "Your message content here"
}

message_id = process_message(message_data)
```

### Searching Messages

To search for messages:

```python
from search import search_messages
import uuid

# Basic semantic search
results = search_messages("your search query", limit=5)

# Search with filters
results = search_messages(
    "your search query",
    limit=5,
    user_id=uuid.UUID("user-uuid-here"),
    server_id=uuid.UUID("server-uuid-here"),
    channel_id=uuid.UUID("channel-uuid-here")
)
```

## Architecture

- **PostgreSQL**: Stores the actual message data and metadata
- **Qdrant**: Handles vector similarity search using OpenAI embeddings
- **Hybrid Search**: Combines vector similarity with relational filters for powerful search capabilities

## Development

To run the test ingestion and search:

```bash
python ingest.py  # Ingest a test message
python search.py  # Perform a test search
```

## Notes

- The system uses OpenAI's `text-embedding-3-small` model for generating embeddings
- Vector similarity search is performed using cosine distance
- The hybrid search combines vector similarity scores with relational filters
- All timestamps are stored in UTC
