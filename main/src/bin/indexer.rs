// ElasticSearch Indexer Worker
// Consumes messages from Kafka and indexes them in ElasticSearch
// Run with: cargo run --bin indexer

use dotenv::dotenv;
use tracing_subscriber;
use tracing::info;
use Quiry::{config::Config, kafka_consumer::KafkaConsumer, kafka_types::DISCORD_MESSAGES_TOPIC, elasticsearch::ElasticsearchClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();
    
    info!("Starting ElasticSearch Indexer Worker...");
    
    // Initialize ElasticSearch client
    let _es_client = ElasticsearchClient::new(&cfg).await?;
    
    // Initialize Kafka consumer
    let mut consumer = KafkaConsumer::new(cfg)?;
    
    // Subscribe to Discord messages topic
    consumer.subscribe_to_topics(&[DISCORD_MESSAGES_TOPIC]).await?;
    
    info!("Indexer subscribed to topics. Starting to consume and index messages...");
    
    // Start consuming and indexing messages
    consumer.start_consuming().await?;
    
    Ok(())
}


