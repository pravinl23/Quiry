// ElasticSearch Indexer Worker
// Consumes messages from Kafka and indexes them in ElasticSearch
// Run with: cargo run --bin indexer

use dotenv::dotenv;
use tracing_subscriber;
use tracing::info;
use Quiry::{config::Config, kafka_consumer::KafkaConsumer, kafka_types::DISCORD_MESSAGES_TOPIC, elasticsearch::ElasticsearchClient, schema::MessageEvent};
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();
    
    info!("Starting ElasticSearch Indexer Worker...");
    
    // Initialize ElasticSearch client
    let es_client = ElasticsearchClient::new(&cfg).await?;
    
    // Initialize Kafka consumer
    let mut consumer = KafkaConsumer::new(cfg)?;
    
    // Subscribe to Discord messages topic
    consumer.subscribe_to_topics(&[DISCORD_MESSAGES_TOPIC]).await?;
    
    info!("Indexer subscribed to topics. Starting to consume and index messages...");
    
    // Start consuming and indexing messages
    index_messages_loop(&mut consumer, &es_client).await?;
    
    Ok(())
}

async fn index_messages_loop(
    consumer: &mut KafkaConsumer,
    es_client: &ElasticsearchClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let message = match consumer.consumer.recv().await {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(error = %e, "Error receiving message from Kafka");
                continue;
            }
        };

        if let Some(payload) = message.payload() {
            let topic = message.topic().to_string();
            let partition = message.partition();
            let offset = message.offset();
            
            match process_and_index_message(payload, es_client).await {
                Ok(_) => {
                    info!(topic = %topic, partition = partition, offset = offset, "Indexed message to ElasticSearch");
                }
                Err(err) => {
                    tracing::error!(error = %err, topic = %topic, "Failed to index message to ElasticSearch");
                }
            }
        }
    }
}

async fn process_and_index_message(
    payload: &[u8],
    es_client: &ElasticsearchClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let kafka_message: Quiry::kafka_types::KafkaMessage = serde_json::from_slice(payload)?;
    
    match kafka_message.event_type {
        Quiry::kafka_types::KafkaEventType::DiscordMessage => {
            if let Quiry::kafka_types::KafkaPayload::DiscordMessage(msg_event) = kafka_message.payload {
                info!(message_id = %msg_event.id, "Indexing Discord message to ElasticSearch");
                
                // Index the message in ElasticSearch
                if let Err(err) = es_client.index_message(&msg_event).await {
                    tracing::error!(error = %err, message_id = %msg_event.id, "Failed to index message to ElasticSearch");
                }
            }
        }
        _ => {
            info!("Unhandled message type for indexing: {:?}", kafka_message.event_type);
        }
    }
    
    Ok(())
}

