use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde_json;
use tracing::{info, error};
use std::time::Duration;
use crate::{config::Config, kafka_types::{KafkaMessage, DISCORD_MESSAGES_TOPIC}};

pub struct KafkaProducer {
    producer: FutureProducer,
}

impl KafkaProducer {
    pub fn new(cfg: &Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.kafka_brokers)
            .set("message.timeout.ms", "5000")
            .set("delivery.timeout.ms", "10000")
            .set("request.timeout.ms", "30000")
            .set("retries", "3")
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .create()?;

        Ok(Self { producer })
    }

    pub async fn send_discord_message(&self, message: KafkaMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let topic = DISCORD_MESSAGES_TOPIC;
        let key = message.get_partition_key();
        let payload = serde_json::to_vec(&message)?;

        let record = FutureRecord::to(topic)
            .key(&key)
            .payload(&payload);

        match self.producer.send(record, Duration::from_secs(0)).await {
            Ok(_) => {
                info!(topic = topic, key = %key, "Sent Discord message to Kafka");
                Ok(())
            }
            Err((kafka_error, _)) => {
                error!(error = %kafka_error, topic = topic, "Failed to send Discord message to Kafka");
                Err(kafka_error.into())
            }
        }
    }

    // Additional producer methods for future Kafka consumer implementation:
    // - send_message_chunk
    // - send_embedding_request  
    // - send_pinecone_upsert
    // - send_query_request
    // - send_message (generic)
}
