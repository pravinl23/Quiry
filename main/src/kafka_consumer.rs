use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use serde_json;
use tracing::{info, error, debug};
use crate::{
    config::Config,
    kafka_types::KafkaMessage,
    cohere::{get_embedding, generate_response, generate_response_from_chunks},
    pinecone::{upsert_to_pinecone, query_pinecone, query_chunks_pinecone},
    chunking::ChunkManager,
    metrics::{KAFKA_MESSAGES_RECEIVED, MESSAGES_PROCESSED, MESSAGES_FAILED},
};

pub struct KafkaConsumer {
    consumer: StreamConsumer,
    cfg: Config,
    chunk_manager: ChunkManager,
}

impl KafkaConsumer {
    pub fn new(cfg: Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", &cfg.kafka_group_id)
            .set("bootstrap.servers", &cfg.kafka_brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "30000")
            .set("enable.auto.commit", "true")
            .set("auto.commit.interval.ms", "5000")
            .set("auto.offset.reset", "earliest")
            .set("max.poll.interval.ms", "600000") // 10 minutes
            .set("heartbeat.interval.ms", "10000") // 10 seconds
            .create()?;

        Ok(Self {
            consumer,
            cfg,
            chunk_manager: ChunkManager::new(),
        })
    }

    pub async fn subscribe_to_topics(&self, topics: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.consumer.subscribe(topics)?;
        info!(topics = ?topics, "Subscribed to Kafka topics");
        Ok(())
    }


    pub async fn start_consuming(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting Kafka consumer...");

        loop {
            let (payload, topic, partition, offset) = {
                let message = match self.consumer.recv().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(error = %e, "Error receiving message from Kafka");
                        continue;
                    }
                };

                if let Some(payload) = message.payload() {
                    let topic = message.topic().to_string();
                    let partition = message.partition();
                    let offset = message.offset();
                    (payload.to_vec(), topic, partition, offset)
                } else {
                    continue;
                }
            };
            
            KAFKA_MESSAGES_RECEIVED.inc();
            match self.process_message(&payload).await {
                Ok(_) => {
                    MESSAGES_PROCESSED.inc();
                    debug!(topic = %topic, partition = partition, offset = offset, "Processed message successfully");
                }
                Err(err) => {
                    MESSAGES_FAILED.inc();
                    error!(error = %err, topic = %topic, "Failed to process message");
                }
            }
        }
    }

    async fn process_message(&mut self, payload: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let kafka_message: KafkaMessage = serde_json::from_slice(payload)?;
        
        match kafka_message.event_type {
            crate::kafka_types::KafkaEventType::DiscordMessage => {
                self.handle_discord_message(kafka_message).await
            }
            crate::kafka_types::KafkaEventType::QueryRequest => {
                self.handle_query_request(kafka_message).await
            }
            _ => {
                info!("Unhandled message type: {:?}", kafka_message.event_type);
                Ok(())
            }
        }
    }

    async fn handle_discord_message(&mut self, message: KafkaMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let crate::kafka_types::KafkaPayload::DiscordMessage(msg_event) = message.payload {
            info!(message_id = %msg_event.id, "Processing Discord message from Kafka");

            // Process through chunking system
            if let Err(err) = self.chunk_manager.process_message(&self.cfg, msg_event.clone()).await {
                error!(error = %err, "Failed to process message through chunking");
            }

            // Also process as individual message for fallback
            match get_embedding(&self.cfg, &msg_event.text).await {
                Ok(embedding) => {
                    if let Err(err) = upsert_to_pinecone(&self.cfg, &msg_event, embedding).await {
                        error!(error = %err, "Failed to upsert individual message");
                    }
                }
                Err(err) => {
                    error!(error = %err, "Failed to get embedding for individual message");
                }
            }
        }
        Ok(())
    }

    async fn handle_query_request(&mut self, message: KafkaMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let crate::kafka_types::KafkaPayload::QueryRequest { question, user_id, guild_id } = message.payload {
            info!(question = %question, user_id = %user_id, "Processing query request");

            // This would typically send the response back to Discord
            // For now, we'll just log that we processed it
            match get_embedding(&self.cfg, &question).await {
                Ok(embedding) => {
                    // Query Pinecone for similar content
                    let similar_chunks = query_chunks_pinecone(&self.cfg, embedding.clone(), 3, guild_id.clone()).await?;
                    
                    if !similar_chunks.is_empty() {
                        let response = generate_response_from_chunks(&self.cfg, &question, &similar_chunks).await?;
                        info!(response = %response, "Generated response from chunks");
                    } else {
                        let similar_messages = query_pinecone(&self.cfg, embedding, 5, guild_id).await?;
                        if !similar_messages.is_empty() {
                            let response = generate_response(&self.cfg, &question, &similar_messages).await?;
                            info!(response = %response, "Generated response from messages");
                        }
                    }
                }
                Err(err) => {
                    error!(error = %err, "Failed to get embedding for query");
                }
            }
        }
        Ok(())
    }
}
