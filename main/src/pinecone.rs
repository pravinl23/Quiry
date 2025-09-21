use reqwest::Client;
use serde_json::json;
use tracing::{info, error};
use crate::{config::Config, schema::MessageEvent};

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
