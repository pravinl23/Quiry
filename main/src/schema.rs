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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageChunk {
    pub chunk_id: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub first_msg_id: String,
    pub last_msg_id: String,
    pub first_timestamp: String,
    pub last_timestamp: String,
    pub message_count: usize,
    pub authors: Vec<String>,
    pub full_text: String,
    pub summary: Option<String>,
    pub has_summary: bool,
}

#[derive(Debug, Clone)]
pub struct ChunkQueryResult {
    pub chunk_id: String,
    pub text: String,
    pub summary: Option<String>,
    pub authors: Vec<String>,
    pub message_count: usize,
    pub first_timestamp: String,
    pub last_timestamp: String,
    pub score: f64,
}
