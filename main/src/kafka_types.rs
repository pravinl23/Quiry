use serde::{Deserialize, Serialize};
use crate::schema::{MessageEvent, MessageChunk};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KafkaEventType {
    DiscordMessage,
    MessageChunk,
    EmbeddingRequest,
    PineconeUpsert,
    QueryRequest,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KafkaMessage {
    pub event_type: KafkaEventType,
    pub message_id: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub timestamp: String,
    pub payload: KafkaPayload,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KafkaPayload {
    DiscordMessage(MessageEvent),
    MessageChunk(MessageChunk),
    EmbeddingRequest {
        text: String,
        message_id: String,
        is_chunk: bool,
    },
    PineconeUpsert {
        message_id: String,
        embedding: Vec<f32>,
        is_chunk: bool,
    },
    QueryRequest {
        question: String,
        user_id: String,
        guild_id: Option<String>,
    },
}

impl KafkaMessage {
    pub fn new_discord_message(message: MessageEvent) -> Self {
        Self {
            event_type: KafkaEventType::DiscordMessage,
            message_id: message.id.clone(),
            guild_id: message.guild_id.clone(),
            channel_id: message.channel_id.clone(),
            timestamp: message.timestamp.clone(),
            payload: KafkaPayload::DiscordMessage(message),
        }
    }

    // Additional constructor methods for future Kafka consumer implementation:
    // - new_message_chunk
    // - new_embedding_request
    // - new_pinecone_upsert
    // - new_query_request

    pub fn get_partition_key(&self) -> String {
        match &self.guild_id {
            Some(guild_id) => guild_id.clone(),
            None => format!("dm:{}", self.channel_id),
        }
    }
}

// Topic names
pub const DISCORD_MESSAGES_TOPIC: &str = "discord-messages";
// Additional topics for future Kafka consumer implementation:
// pub const MESSAGE_CHUNKS_TOPIC: &str = "message-chunks";
// pub const EMBEDDING_REQUESTS_TOPIC: &str = "embedding-requests";
// pub const PINECONE_UPSERTS_TOPIC: &str = "pinecone-upserts";
// pub const QUERY_REQUESTS_TOPIC: &str = "query-requests";
