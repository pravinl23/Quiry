// ElasticSearch Indexer Worker with Metrics
// Consumes messages from Kafka and indexes them in ElasticSearch
// Run with: cargo run --bin indexer

use dotenv::dotenv;
use tracing_subscriber;
use tracing::{info, error};
use std::sync::Arc;
use warp::Filter;
use tokio::sync::Mutex;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use Quiry::{
    config::Config, 
    kafka_types::{DISCORD_MESSAGES_TOPIC, KafkaMessage, KafkaPayload}, 
    elasticsearch::ElasticsearchClient,
    metrics::MetricsRegistry,
    health::HealthChecker,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();
    let metrics_registry = Arc::new(MetricsRegistry::new());
    let health_checker = Arc::new(HealthChecker::new());
    
    let port = std::env::var("PORT").unwrap_or_else(|_| "8085".to_string()).parse::<u16>().unwrap_or(8085);
    info!("Starting ElasticSearch Indexer Worker on port {}...", port);
    
    // Initialize ElasticSearch client
    let es_client = Arc::new(Mutex::new(ElasticsearchClient::new(&cfg).await?));
    
    // Initialize Kafka consumer
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "quiry-indexer")
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "30000")
        .set("enable.auto.commit", "true")
        .set("auto.commit.interval.ms", "5000")
        .set("auto.offset.reset", "earliest")
        .set("max.poll.interval.ms", "600000")
        .set("heartbeat.interval.ms", "10000")
        .create()?;
    
    consumer.subscribe(&[DISCORD_MESSAGES_TOPIC])?;
    info!("Indexer subscribed to topics. Starting to consume and index messages...");
    
    // Start metrics server
    let metrics_route = warp::path("metrics")
        .and(warp::get())
        .and(with_metrics(metrics_registry.clone()))
        .and_then(handle_metrics);

    let health_route = warp::path("health")
        .and(warp::get())
        .and(with_health_checker(health_checker.clone()))
        .and(with_config(cfg.clone()))
        .and_then(handle_health);

    let root_route = warp::path::end()
        .and(warp::get())
        .map(|| "Quiry Indexer Service - /metrics, /health");

    let routes = metrics_route.or(health_route).or(root_route);
    
    // Start HTTP server in background
    let server = tokio::spawn(async move {
        warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    });
    
    // Start consuming and indexing messages
    let indexer = tokio::spawn(async move {
        loop {
            match consumer.recv().await {
                Ok(message) => {
                    if let Some(payload) = message.payload() {
                        match serde_json::from_slice::<KafkaMessage>(payload) {
                            Ok(kafka_msg) => {
                                if let KafkaPayload::DiscordMessage(msg_event) = kafka_msg.payload {
                                    info!(message_id = %msg_event.id, "Indexing message to ElasticSearch");
                                    
                                    let es = es_client.lock().await;
                                    if let Err(e) = es.index_message(&msg_event).await {
                                        error!(message_id = %msg_event.id, "Failed to index message: {}", e);
                                    }
                                }
                            }
                            Err(e) => error!("Failed to deserialize Kafka message: {}", e),
                        }
                    }
                }
                Err(e) => error!("Error receiving message from Kafka: {}", e),
            }
        }
    });
    
    // Wait for both tasks
    tokio::select! {
        _ = server => info!("HTTP server stopped"),
        _ = indexer => info!("Indexer stopped"),
    }
    
    Ok(())
}

fn with_metrics(
    metrics: Arc<MetricsRegistry>,
) -> impl Filter<Extract = (Arc<MetricsRegistry>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || metrics.clone())
}

fn with_health_checker(
    health_checker: Arc<HealthChecker>,
) -> impl Filter<Extract = (Arc<HealthChecker>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || health_checker.clone())
}

fn with_config(
    config: Config,
) -> impl Filter<Extract = (Config,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || config.clone())
}

async fn handle_metrics(metrics: Arc<MetricsRegistry>) -> Result<impl warp::Reply, warp::Rejection> {
    let metrics_text = metrics.gather_metrics();
    Ok(warp::reply::with_header(
        metrics_text,
        "Content-Type",
        "text/plain; version=0.0.4; charset=utf-8",
    ))
}

async fn handle_health(
    health_checker: Arc<HealthChecker>,
    config: Config,
) -> Result<impl warp::Reply, warp::Rejection> {
    let health_status = health_checker
        .get_overall_health(&config.elasticsearch_url, &config.pinecone_host)
        .await;
    
    let json_response = serde_json::to_string_pretty(&health_status)
        .unwrap_or_else(|_| "{\"error\": \"Failed to serialize health status\"}".to_string());
    
    Ok(warp::reply::with_header(
        json_response,
        "Content-Type",
        "application/json",
    ))
}
