use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageEvent {
    pub id: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub author_id: String,
    pub timestamp: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub text: String,
    pub author_id: String,
    pub timestamp: String,
    pub score: f64,
}
