use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};
use crate::{config::Config, schema::QueryResult};

type DynErr = Box<dyn std::error::Error + Send + Sync>;

pub async fn get_embedding(cfg: &Config, text: &str) -> Result<Vec<f32>, DynErr> {
    let client = Client::new();

    let res = client
        .post("https://api.cohere.ai/v1/embed")
        .bearer_auth(&cfg.cohere_key)
        .json(&json!({
            "model": "embed-english-v3.0",
            "input_type": "search_document",
            "texts": [text]
        }))
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(format!("Cohere error: {}", res.text().await?).into());
    }

    let body: serde_json::Value = res.json().await?;
    if let Some(array) = body["embeddings"][0].as_array() {
        let emb: Vec<f32> = array.iter().filter_map(|v| v.as_f64()).map(|v| v as f32).collect();
        info!(len = emb.len(), sample = ?&emb[..5.min(emb.len())], "Got embedding");
        Ok(emb)
    } else {
        warn!("No embeddings in Cohere response: {body:?}");
        Err("No embeddings found".into())
    }
}

pub async fn generate_response(cfg: &Config, query: &str, context_messages: &[QueryResult]) -> Result<String, DynErr> {
    let client = Client::new();

    let context = context_messages
        .iter()
        .map(|msg| format!("- {}", msg.text))
        .collect::<Vec<_>>()
        .join("\n");

    let res = client
        .post("https://api.cohere.ai/v1/chat")
        .bearer_auth(&cfg.cohere_key)
        .json(&json!({
            "model": "command-r-08-2024",
            "message": query,
            "preamble": format!(
                "You are a helpful assistant that answers questions based on Discord message history. \
                Here are some relevant messages from the conversation:\n\n{}\n\n\
                Please provide a helpful answer based on the context above. If the context doesn't contain \
                enough information to answer the question, say so.",
                context
            ),
            "max_tokens": 300,
            "temperature": 0.7
        }))
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(format!("Cohere generate error: {}", res.text().await?).into());
    }

    let body: serde_json::Value = res.json().await?;
    if let Some(text) = body["text"].as_str() {
        let response = text.trim().to_string();
        info!(len = response.len(), "Generated response");
        Ok(response)
    } else {
        warn!("No text in Cohere response: {body:?}");
        Err("No generated text found".into())
    }
}
