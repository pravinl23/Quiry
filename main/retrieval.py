import os
import numpy as np
import faiss
import google.generativeai as gen
from dotenv import load_dotenv
from math import sqrt
from main.database import get_server_db, generate_embedding
from datetime import datetime
import json

# Load environment variables
load_dotenv()
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")

# Initialize Gemini client
gen.configure(api_key=GEMINI_API_KEY)

'''
# Test OpenAI connection with a sample request
completion = client.chat.completions.create(
    model="gpt-3.5-turbo",
    messages=[
        {"role": "user", "content": "write a haiku about ai"}
    ]
)

print(completion.choices[0].message.content)

'''
CHECK_COSINE_SIMILARITY = True

# Load embeddings from Supabase and initializes FAISS searching
def load_embeddings(server_id):
    print(f"Loading embeddings for server {server_id}")
    messages = get_server_db(server_id)
    print(f"Messages: {messages}")

    if not messages:
        return None, None, None

    # Build an array of embeddings with proper error handling
    try:
        embedding_lists = [doc["embedding"].tolist() if isinstance(doc["embedding"], np.ndarray) else doc["embedding"] for doc in messages]
        embeddings = np.array(embedding_lists, dtype=np.float32)
        
        # Check if embeddings have correct dimension
        if embeddings.shape[1] != 768:
            print(f"Warning: Expected 768 dimensions, got {embeddings.shape[1]}")
            
    except Exception as e:
        print(f"Error creating embeddings array: {e}")
        return None, None, None

    message_text_mapping = {str(doc["id"]): doc["text_message"] for doc in messages}

    # Build the FAISS index
    embedding_dimension = embeddings.shape[1]
    try:
        index = faiss.IndexFlatL2(embedding_dimension)
        index.add(embeddings)
    except Exception as e:
        print(f"Error building FAISS index: {e}")
        return None, None, None

    return index, messages, message_text_mapping

# Uses FAISS to find vectors that are the most similar to eachother.
def search_similar_messages(query, index, all_docs, text_map, top_k=5):
    if index is None:
        return []

    # Embed the query using OpenAI
    try:
        query_embedding = generate_embedding(query)
    except Exception as e:
        print(f"Error embedding query: {e}")
        return []
    
    query_embedding_np = np.array([query_embedding], dtype=np.float32)

    # FAISS the top_k most similar vectors
    try:
        distances, indices = index.search(query_embedding_np, top_k)
    except Exception as e:
        print(f"Error searching for similar messages: {e}")
        return []

    candidates = []
    for index in indices[0]:
        if index < len(all_docs):
            doc = all_docs[index]
            candidates.append(doc)

    if not candidates:
        return []

    if CHECK_COSINE_SIMILARITY:
        # Rerank using cosine similarity
        query_norm = sqrt(sum(x*x for x in query_embedding))

        def cosine_sim(query_vec, doc_vec):
            dot_val = sum(q*d for q, d in zip(query_vec, doc_vec))
            doc_norm = sqrt(sum(d*d for d in doc_vec))
            return dot_val / (query_norm * doc_norm + 1e-9)

        scored_candidates = []

        for c in candidates:
            doc_embedding = c["embedding"]
            score = cosine_sim(query_embedding, doc_embedding)
            try:
                scored_candidates.append((c, score))
            except Exception as e:
                print(f"Error adding candidate: {e}")
                return []

        # Sort by descending similarity
        scored_candidates.sort(key=lambda x: x[1], reverse=True)

        # Extract top_k text
        top_texts = []
        for cand_doc, _ in scored_candidates[:top_k]:
            try:
                msg_id = str(cand_doc["id"])
                top_texts.append(text_map[msg_id])
            except Exception as e:
                print(f"Error adding candidate: {e}")
                return []
        return top_texts
    else:
        # No cosine similarity, then just approximate order from FAISS
        top_texts = []
        for doc in candidates:
            msg_id = str(doc["id"])
            top_texts.append(text_map[msg_id])
        return top_texts

# Generates response using GPT-3.5-turbo
def generate_response(query, server_id, top_k=5):
    # Load existing embeddings into FAISS
    try:
        index, all_docs, text_map = load_embeddings(server_id)
    except Exception as e:
        print(f"Error loading embeddings 1: {e}")
        return "No relevant messages have been indexed for this server yet."
    
    if index is None:
        return "No relevant messages have been indexed for this server yet."

    # Get the top_k chunks
    relevant_chunks = search_similar_messages(query, index, all_docs, text_map, top_k=top_k)
    
    if not relevant_chunks:
        context = "No similar messages were found in the database."
    else:
        context = "\n".join(relevant_chunks)

    # Get today's date for time references
    today = datetime.now().strftime("%Y-%m-%d")

    prompt = f"""You are Quiry, a friendly and insightful AI companion who loves helping people explore their Discord conversations and answering questions with genuine enthusiasm.

    **Available Context from Server Conversations:** {context if context else "No server context available"}

    **Your Personality:**
    - Talk like a close friend who's genuinely excited to help
    - Be warm, conversational, and delightfully insightful
    - Use natural language with occasional enthusiasm (but don't overdo it!)
    - Share interesting observations and connections you notice
    - Be curious and engaging, like you're having a real conversation

    **Response Guidelines:**
    - If asking about server conversations AND context is available → Draw insights from the conversations, mention interesting patterns you notice, and reference specific messages naturally (like "I noticed someone mentioned...")
    - If asking general questions OR no context available → Share your knowledge in a friendly, conversational way
    - When referencing server conversations, weave them into your response naturally rather than formal citations
    - Add your own thoughtful commentary and insights
    - Keep responses conversational but informative

    Question: {query}
    
    Remember: Respond like you're chatting with a friend who just asked you something interesting!
    """
    model = gen.GenerativeModel(
    'gemini-1.5-flash', 
    system_instruction="You are Quiry, an AI assistant that answers questions about Discord server conversations."
    )

    response = model.generate_content(prompt)

    return response.text
