use reqwest::Client;
use serde_json::json;
use tracing::{info, error};
use crate::{config::Config, schema::{MessageEvent, QueryResult, MessageChunk, ChunkQueryResult}};

type DynErr = Box<dyn std::error::Error + Send + Sync>;

pub async fn upsert_to_pinecone(cfg: &Config, msg: &MessageEvent, embedding: Vec<f32>) -> Result<(), DynErr> {
    let url = format!("{}/vectors/upsert", cfg.pinecone_host);
    let client = Client::new();

    let res = client
        .post(&url)
        .header("Api-Key", &cfg.pinecone_key)
        .json(&json!({
            "namespace": cfg.namespace,
            "vectors": [{
                "id": msg.id,
                "values": embedding,
                "metadata": {
                    "guild_id": msg.guild_id,
                    "channel_id": msg.channel_id,
                    "author_id": msg.author_id,
                    "timestamp": msg.timestamp,
                    "text": msg.text
                }
            }]
        }))
        .send()
        .await?;

    let status = res.status();
    let body = res.text().await?;

    if !status.is_success() {
        error!(status=?status, body=?body, "Pinecone upsert failed");
        return Err(format!("Pinecone error: {status}").into());
    }

    info!(msg_id=?msg.id, "Upserted to Pinecone");
    Ok(())
}

pub async fn query_pinecone(cfg: &Config, embedding: Vec<f32>, top_k: usize, guild_id: Option<String>) -> Result<Vec<QueryResult>, DynErr> {
    let url = format!("{}/query", cfg.pinecone_host);
    let client = Client::new();

    let mut query = json!({
        "namespace": cfg.namespace,
        "vector": embedding,
        "topK": top_k,
        "includeMetadata": true,
        "includeValues": false
    });

    // Add guild_id filter if provided
    if let Some(guild_id) = guild_id {
        query["filter"] = json!({
            "guild_id": {"$eq": guild_id}
        });
    } else {
        // For DMs, filter for messages without guild_id (null values)
        query["filter"] = json!({
            "guild_id": {"$exists": false}
        });
    }

    let res = client
        .post(&url)
        .header("Api-Key", &cfg.pinecone_key)
        .json(&query)
        .send()
        .await?;

    let status = res.status();
    let body: serde_json::Value = res.json().await?;

    if !status.is_success() {
        error!(status=?status, body=?body, "Pinecone query failed");
        return Err(format!("Pinecone query error: {status}").into());
    }

    let empty_vec = vec![];
    let matches = body["matches"].as_array().unwrap_or(&empty_vec);
    let mut results = Vec::new();

    for match_obj in matches {
        let score = match_obj["score"].as_f64().unwrap_or(0.0);
        let metadata = &match_obj["metadata"];

        if let (Some(text), Some(author_id), Some(timestamp)) = (
            metadata["text"].as_str(),
            metadata["author_id"].as_str(),
            metadata["timestamp"].as_str(),
        ) {
            results.push(QueryResult {
                text: text.to_string(),
                author_id: author_id.to_string(),
                timestamp: timestamp.to_string(),
                score,
            });
        }
    }

    info!(count = results.len(), "Found similar messages");
    Ok(results)
}

pub async fn upsert_chunk_to_pinecone(cfg: &Config, chunk: &MessageChunk, embedding: Vec<f32>) -> Result<(), DynErr> {
    let url = format!("{}/vectors/upsert", cfg.pinecone_host);
    let client = Client::new();

    let mut metadata = json!({
        "type": "chunk",
        "chunk_id": chunk.chunk_id,
        "guild_id": chunk.guild_id,
        "channel_id": chunk.channel_id,
        "first_msg_id": chunk.first_msg_id,
        "last_msg_id": chunk.last_msg_id,
        "first_timestamp": chunk.first_timestamp,
        "last_timestamp": chunk.last_timestamp,
        "message_count": chunk.message_count,
        "authors": chunk.authors,
        "full_text": chunk.full_text,
        "has_summary": chunk.has_summary
    });

    // Only include summary if it exists (avoid null values)
    if let Some(ref summary) = chunk.summary {
        metadata["summary"] = json!(summary);
    }

    let res = client
        .post(&url)
        .header("Api-Key", &cfg.pinecone_key)
        .json(&json!({
            "namespace": cfg.namespace,
            "vectors": [{
                "id": format!("chunk_{}", chunk.chunk_id),
                "values": embedding,
                "metadata": metadata
            }]
        }))
        .send()
        .await?;

    let status = res.status();
    let body = res.text().await?;

    if !status.is_success() {
        error!(status=?status, body=?body, "Pinecone chunk upsert failed");
        return Err(format!("Pinecone chunk error: {status}").into());
    }

    info!(chunk_id=?chunk.chunk_id, "Upserted chunk to Pinecone");
    Ok(())
}

pub async fn query_chunks_pinecone(cfg: &Config, embedding: Vec<f32>, top_k: usize, guild_id: Option<String>) -> Result<Vec<ChunkQueryResult>, DynErr> {
    let url = format!("{}/query", cfg.pinecone_host);
    let client = Client::new();

    let mut query = json!({
        "namespace": cfg.namespace,
        "vector": embedding,
        "topK": top_k,
        "includeMetadata": true,
        "includeValues": false,
        "filter": {
            "type": {"$eq": "chunk"}
        }
    });

    // Add guild_id filter if provided
    if let Some(guild_id) = guild_id {
        query["filter"]["guild_id"] = json!({"$eq": guild_id});
    } else {
        // For DMs, filter for chunks without guild_id (null values)
        query["filter"]["guild_id"] = json!({"$exists": false});
    }

    let res = client
        .post(&url)
        .header("Api-Key", &cfg.pinecone_key)
        .json(&query)
        .send()
        .await?;

    let status = res.status();
    let body: serde_json::Value = res.json().await?;

    if !status.is_success() {
        error!(status=?status, body=?body, "Pinecone chunk query failed");
        return Err(format!("Pinecone chunk query error: {status}").into());
    }

    let empty_vec = vec![];
    let matches = body["matches"].as_array().unwrap_or(&empty_vec);
    let mut results = Vec::new();

    for match_obj in matches {
        let score = match_obj["score"].as_f64().unwrap_or(0.0);
        let metadata = &match_obj["metadata"];

        if let (
            Some(chunk_id),
            Some(full_text),
            Some(first_timestamp),
            Some(last_timestamp),
            Some(message_count)
        ) = (
            metadata["chunk_id"].as_str(),
            metadata["full_text"].as_str(),
            metadata["first_timestamp"].as_str(),
            metadata["last_timestamp"].as_str(),
            metadata["message_count"].as_u64(),
        ) {
            let summary = metadata["summary"].as_str().map(|s| s.to_string());
            let authors = metadata["authors"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_else(Vec::new);

            // Use summary if available, otherwise use truncated full text
            let text = if let Some(ref summary) = summary {
                summary.clone()
            } else {
                if full_text.len() > 500 {
                    format!("{}...", &full_text[..500])
                } else {
                    full_text.to_string()
                }
            };

            results.push(ChunkQueryResult {
                chunk_id: chunk_id.to_string(),
                text,
                summary,
                authors,
                message_count: message_count as usize,
                first_timestamp: first_timestamp.to_string(),
                last_timestamp: last_timestamp.to_string(),
                score,
            });
        }
    }

    info!(count = results.len(), "Found similar chunks");
    Ok(results)
}
