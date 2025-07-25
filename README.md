# Quiry: Discord Server Memory & Search Bot

A Discord bot that provides AI-powered conversation search and retrieval capabilities. Built with Python, discord.py, Supabase, FAISS, and the Google Generative AI (Gemini API), Quiry leverages high-dimensional vector embeddings to enable Retrieval-Augmented Generation (RAG) for dynamic, context-driven responses.

## Features

1. **Real-time Message Processing**
   - Automatically captures and processes messages from all channels
   - Groups messages into semantic chunks for efficient storage
   - Maintains conversation context and metadata

2. **Advanced Search Capabilities**
   - Semantic search across server history
   - Context-aware responses using RAG
   - Time-based filtering and relevance ranking

3. **Scalable Architecture**
   - Manages hundreds of thousands of messages across multiple servers using **Supabase** for reliable data storage
   - Efficient vector similarity search with FAISS
   - Real-time message processing and embedding generation

> **Create the `message_chunks` table in Supabase:**
>
> ```sql
> create table message_chunks (
>     id uuid primary key default gen_random_uuid(),
>     server_id text,
>     channel_id text,
>     text_message text,
>     embedding vector(768), -- If you have the vector extension enabled
>     timestamp timestamp,
>     category text,
>     message_count integer
> );
> ```
>
> - If you don’t have the `vector` extension enabled, use a different type for `embedding` (e.g., `float8[]`).
> - Run this SQL in the Supabase dashboard’s SQL Editor.

## How It Works

1. **Message Collection**
   - Quiry listens for new messages, converts them into vector embeddings (via the Gemini API), and stores them in Supabase
   - Messages are grouped into chunks of 10 for efficient processing
   - Each chunk maintains metadata including server, channel, and timestamp information

2. **Ask Questions**
   - Use the `/asking` command to retrieve contextually similar past conversations
   - Quiry's advanced RAG system provides intelligent responses based on your server's history

3. **Admin Commands**
   - **`/clearing X`** – Remove the most recent X messages from your server's database (useful for data reset or privacy)
   - **`/invite`** – Get the bot's invite link
---

## Adding Quiry to Your Server

**[Invite Quiry](https://discord.com/oauth2/authorize?client_id=1340139928994189322&permissions=8&integration_type=0&scope=bot)**

1. Click the link above.  
2. Select the Discord server you want to add Quiry to.  
3. Grant the necessary permissions (e.g., reading message history).  
4. Quiry will join your server and begin indexing messages automatically.

---

## Usage

1. **Monitor and Store Messages**  
   - Quiry listens for new messages, converts them into vector embeddings (via the Gemini API), and stores them in Supabase.
2. **Ask Questions**  
   - Use the `/ask` command to retrieve contextually similar past conversations. Quiry's advanced RAG system provides intelligent responses based on your server's history.
3. **Admin Commands** (optional)  
   - **`/clear X`** – Remove the most recent X messages from your server's database (useful for data reset or privacy).  
   - **`/fetch X`** – (Currently Disabled) Backfill up to X historical messages from your server's channels, giving Quiry a larger data set for retrieval.

---

## Technical Overview

- **Language & Libraries**  
  - Python, discord.py  
  - Supabase  
  - NumPy, FAISS  
  - Google Generative AI (Gemini API)  
- **Core Concepts**  
  - **Vector Embeddings** for chat messages  
  - **Semantic Similarity Search** using FAISS  
  - **Retrieval-Augmented Generation (RAG)** for context-aware answers  
- **Architecture**  
  1. **Message Logging**: Listens to messages in real time.  
  2. **Embedding Generation**: Gemini API transforms each message into a high-dimensional vector.  
  3. **Indexing & Storage**: Embeddings stored in Supabase and indexed with FAISS.  
  4. **Response Generation**: When asked, Quiry fetches the most similar embeddings, forms a context, and produces a relevant response.

---

## Disclaimer

- By adding Quiry to your server, you grant permission for message data to be stored and processed for semantic retrieval and context-aware responses.

---

## Contact & Support

If you have questions or need support, you can contact the bot's owners:

- **Discord**: `[pppravin]`, `[alexwang06]`
- **Email**: `[pravin.lohani23@gmail.com]`, `[wangalex0140@gmail.com]`

Quiry is continually evolving to offer better search and AI-driven insights for your community. Feedback and suggestions are always welcome!

## Technical Architecture

### Core Components

1. **Message Processing Pipeline**
   ```python
   Discord Message → Buffer → Chunk → Embedding → Supabase
   ```
   - Messages are collected in memory buffers (10 messages per chunk)
   - Each chunk is processed into a single document with metadata
   - Google's Gemini text-embedding-004 generates vector embeddings
   - Data is stored in Supabase with vector support

2. **Search & Retrieval System**
   ```python
   User Query → Embedding → FAISS Search → Cosine Similarity → Context → GPT-4
   ```
   - FAISS (Facebook AI Similarity Search) for efficient vector similarity
   - Two-stage ranking: FAISS for initial retrieval, cosine similarity for reranking
   - Context window of top 5 most relevant message chunks

### Database Schema (Supabase)

```sql
message_chunks {
    id: uuid
    server_id: text
    channel_id: text
    text_message: text
    embedding: vector(768)  // Gemini embedding dimension
    timestamp: timestamp
    category: text
    message_count: integer
}
```

### Key Technical Decisions

1. **Chunking Strategy**
   - 10 messages per chunk to balance context and granularity
   - Chunks preserve conversation flow while reducing storage overhead
   - Metadata includes earliest timestamp for accurate time references

2. **Vector Search Implementation**
   ```python
   # FAISS Index Creation
   embedding_dimension = embeddings.shape[1]
   index = faiss.IndexFlatL2(embedding_dimension)
   index.add(embeddings)
   ```
   - L2 (Euclidean) distance for initial retrieval
   - Cosine similarity for final ranking
   - Configurable top-k retrieval (default: 5)

3. **Embedding Generation**
   ```python
   def generate_embedding(text):
       response = gen.embed_content(
           model="models/text-embedding-004",
           content=text,
           task_type="retrieval_document"
       )
       return response["embedding"]
   ```
   - Uses Google's Gemini text-embedding-004 model
   - 768-dimensional vectors
   - Optimized for semantic similarity and retrieval tasks

4. **Response Generation**
   ```python
   def generate_response(query, server_id, top_k=5):
       # Load and search embeddings
       index, all_docs, text_map = load_embeddings(server_id)
       relevant_chunks = search_similar_messages(query, index, all_docs, text_map)
       
       # Generate response with GPT-4
       response = client.chat.completions.create(
           model="gpt-4",
           messages=[...]
       )
   ```
   - GPT-4 for high-quality responses
   - Structured prompt for consistent formatting
   - Dynamic time reference calculation

### Memory Management

1. **Buffer System**
   ```python
   conversation_buffers = {}
   CHUNK_SIZE = 10
   ```
   - In-memory buffering for real-time processing
   - Server-channel specific buffers
   - Automatic chunking when buffer size threshold reached

2. **Data Flow**
   ```
   Message → Buffer
   Buffer (size >= CHUNK_SIZE) → Process Chunk
   Process Chunk → Generate Embedding
   Embedding + Metadata → Supabase
   ```

### Search Algorithm

1. **Initial Retrieval (FAISS)**
   ```python
   distances, indices = index.search(query_embedding_np, top_k)
   ```
   - Fast approximate nearest neighbor search
   - L2 distance for initial candidate selection

2. **Reranking (Cosine Similarity)**
   ```python
   def cosine_sim(query_vec, doc_vec):
       dot_val = sum(q*d for q, d in zip(query_vec, doc_vec))
       doc_norm = sqrt(sum(d*d for d in doc_vec))
       return dot_val / (query_norm * doc_norm + 1e-9)
   ```
   - Cosine similarity for semantic relevance
   - Normalized dot product for better accuracy
   - Epsilon term (1e-9) to prevent division by zero

### Response Formatting

1. **Structured Output**
   - Author citations with timestamps
   - Relative time references
   - Bullet points for multiple facts
   - Clear handling of missing information

2. **Safety Measures**
   - Content flagging system
   - User ID obfuscation
   - Explicit handling of unsafe content

### Performance Considerations

1. **Optimizations**
   - Chunking reduces storage and processing overhead
   - FAISS enables fast similarity search
   - Cosine similarity reranking improves accuracy
   - Buffer system prevents excessive API calls

2. **Scalability**
   - Server-specific data partitioning
   - Efficient vector storage in Supabase
   - Configurable chunk sizes
   - Adjustable top-k retrieval

### Technical Stack

- **Backend**: Python 3.12
- **AI/ML**: 
  - Google Gemini API (text-embedding-004 for embeddings)
  - OpenAI API (GPT-4 for response generation)
  - FAISS for vector similarity
- **Database**: Supabase (PostgreSQL with vector support)
- **Bot Framework**: Discord.py
- **Vector Operations**: NumPy
- **Environment**: Python virtual environment

This architecture enables efficient storage, retrieval, and generation of responses based on Discord server conversations while maintaining scalability and performance.
