use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};
use crate::config::Config;

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
