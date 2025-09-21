use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn, error};
use uuid::Uuid;
use crate::schema::{MessageEvent, MessageChunk};
use crate::config::Config;
use crate::cohere::{get_embedding, generate_summary};
use crate::pinecone::upsert_chunk_to_pinecone;

const MAX_CHUNK_SIZE: usize = 12;
const MIN_CHUNK_SIZE: usize = 3;
const TIME_GAP_MINUTES: u64 = 15;
const SUMMARY_THRESHOLD_CHARS: usize = 2000;

#[derive(Debug)]
pub struct MessageBuffer {
    messages: Vec<MessageEvent>,
    last_message_time: SystemTime,
}

impl MessageBuffer {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            last_message_time: UNIX_EPOCH,
        }
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn len(&self) -> usize {
        self.messages.len()
    }

    fn should_flush(&self, new_message_time: SystemTime) -> bool {
        if self.messages.is_empty() {
            return false;
        }

        // Check if buffer is at max capacity
        if self.len() >= MAX_CHUNK_SIZE {
            return true;
        }

        // Check for time gap
        if let Ok(duration) = new_message_time.duration_since(self.last_message_time) {
            duration > Duration::from_secs(TIME_GAP_MINUTES * 60)
        } else {
            false
        }
    }

    fn add_message(&mut self, message: MessageEvent) {
        if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&message.timestamp) {
            self.last_message_time = timestamp.into();
        }
        self.messages.push(message);
    }

    fn create_chunk(&mut self) -> Option<MessageChunk> {
        if self.messages.len() < MIN_CHUNK_SIZE {
            return None;
        }

        let chunk_id = Uuid::new_v4().to_string();
        let first_msg = &self.messages[0];
        let last_msg = &self.messages[self.messages.len() - 1];

        // Collect unique authors
        let mut authors: Vec<String> = self.messages
            .iter()
            .map(|msg| msg.author_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        authors.sort();

        // Combine all message texts
        let full_text = self.messages
            .iter()
            .map(|msg| format!("{}: {}", msg.author_id, msg.text))
            .collect::<Vec<_>>()
            .join("\n");

        let chunk = MessageChunk {
            chunk_id,
            guild_id: first_msg.guild_id.clone(),
            channel_id: first_msg.channel_id.clone(),
            first_msg_id: first_msg.id.clone(),
            last_msg_id: last_msg.id.clone(),
            first_timestamp: first_msg.timestamp.clone(),
            last_timestamp: last_msg.timestamp.clone(),
            message_count: self.messages.len(),
            authors,
            full_text,
            summary: None,
            has_summary: false,
        };

        // Clear the buffer
        self.messages.clear();

        Some(chunk)
    }
}

pub struct ChunkManager {
    buffers: HashMap<String, MessageBuffer>,
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    fn get_buffer_key(guild_id: &Option<String>, channel_id: &str) -> String {
        match guild_id {
            Some(gid) => format!("{}:{}", gid, channel_id),
            None => format!("dm:{}", channel_id),
        }
    }

    pub async fn process_message(&mut self, cfg: &Config, message: MessageEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let buffer_key = Self::get_buffer_key(&message.guild_id, &message.channel_id);

        // Parse message timestamp
        let message_time = chrono::DateTime::parse_from_rfc3339(&message.timestamp)
            .map_err(|e| format!("Invalid timestamp: {}", e))?
            .into();

        let mut chunk_to_process = None;

        // Check if we should flush the current buffer before adding the new message
        {
            let buffer = self.buffers.entry(buffer_key.clone()).or_insert_with(MessageBuffer::new);
            if buffer.should_flush(message_time) {
                chunk_to_process = buffer.create_chunk();
            }
        }

        // Process the chunk if created (outside of the borrow)
        if let Some(chunk) = chunk_to_process {
            info!(chunk_id=?chunk.chunk_id, message_count=chunk.message_count, "Created chunk");
            self.process_chunk(cfg, chunk).await?;
        }

        // Add the new message to the buffer
        let buffer = self.buffers.get_mut(&buffer_key).unwrap();
        buffer.add_message(message);

        // Check if buffer is now at max capacity and should be flushed
        let mut chunk_to_process = None;
        if buffer.len() >= MAX_CHUNK_SIZE {
            chunk_to_process = buffer.create_chunk();
        }

        // Process the chunk if created
        if let Some(chunk) = chunk_to_process {
            info!(chunk_id=?chunk.chunk_id, message_count=chunk.message_count, "Created chunk (max size)");
            self.process_chunk(cfg, chunk).await?;
        }

        Ok(())
    }

    async fn process_chunk(&self, cfg: &Config, mut chunk: MessageChunk) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Generate summary if chunk is long enough
        if chunk.full_text.len() > SUMMARY_THRESHOLD_CHARS {
            match generate_summary(cfg, &chunk.full_text).await {
                Ok(summary) => {
                    chunk.summary = Some(summary);
                    chunk.has_summary = true;
                    info!(chunk_id=?chunk.chunk_id, "Generated summary for chunk");
                }
                Err(err) => {
                    warn!(chunk_id=?chunk.chunk_id, error=?err, "Failed to generate summary");
                }
            }
        }

        // Get embedding for the chunk (use summary if available, otherwise full text)
        let text_to_embed = if chunk.has_summary {
            chunk.summary.as_ref().unwrap()
        } else {
            &chunk.full_text
        };

        match get_embedding(cfg, text_to_embed).await {
            Ok(embedding) => {
                if let Err(err) = upsert_chunk_to_pinecone(cfg, &chunk, embedding).await {
                    error!(chunk_id=?chunk.chunk_id, error=?err, "Failed to upsert chunk to Pinecone");
                }
            }
            Err(err) => {
                error!(chunk_id=?chunk.chunk_id, error=?err, "Failed to get embedding for chunk");
            }
        }

        Ok(())
    }

    pub async fn flush_all_buffers(&mut self, cfg: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut chunks_to_process = Vec::new();

        // Collect all chunks from all buffers
        for (buffer_key, buffer) in self.buffers.iter_mut() {
            if !buffer.is_empty() {
                if let Some(chunk) = buffer.create_chunk() {
                    info!(buffer_key=?buffer_key, chunk_id=?chunk.chunk_id, "Flushed buffer to chunk");
                    chunks_to_process.push(chunk);
                }
            }
        }

        // Process all collected chunks
        for chunk in chunks_to_process {
            self.process_chunk(cfg, chunk).await?;
        }

        Ok(())
    }
}