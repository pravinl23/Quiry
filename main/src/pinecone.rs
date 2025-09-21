use reqwest::Client;
use serde_json::json;
use tracing::{info, error};
use crate::{config::Config, schema::{MessageEvent, QueryResult}};

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
