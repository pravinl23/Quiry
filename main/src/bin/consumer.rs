// Kafka Consumer Service
// Run with: cargo run --bin consumer

use dotenv::dotenv;
use tracing_subscriber;
use tracing::info;
use Quiry::{config::Config, kafka_consumer::KafkaConsumer, kafka_types::DISCORD_MESSAGES_TOPIC};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();
    
    info!("Starting Kafka Consumer Service...");
    
    let mut consumer = KafkaConsumer::new(cfg)?;
    
    // Subscribe to Discord messages topic
    consumer.subscribe_to_topics(&[DISCORD_MESSAGES_TOPIC]).await?;
    
    info!("Consumer subscribed to topics. Starting to consume messages...");
    
    // Start consuming messages
    consumer.start_consuming().await?;
    
    Ok(())
}
