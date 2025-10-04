// Kafka Consumer Service with Metrics
// Run with: cargo run --bin consumer

use dotenv::dotenv;
use tracing_subscriber;
use tracing::info;
use std::sync::Arc;
use warp::Filter;
use Quiry::{
    config::Config, 
    kafka_consumer::KafkaConsumer, 
    kafka_types::DISCORD_MESSAGES_TOPIC,
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
    
    let port = std::env::var("PORT").unwrap_or_else(|_| "8084".to_string()).parse::<u16>().unwrap_or(8084);
    info!("Starting Kafka Consumer Service on port {}...", port);
    
    let mut consumer = KafkaConsumer::new(cfg.clone())?;
    
    // Subscribe to Discord messages topic
    consumer.subscribe_to_topics(&[DISCORD_MESSAGES_TOPIC]).await?;
    
    info!("Consumer subscribed to topics. Starting to consume messages...");
    
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
        .map(|| "Quiry Consumer Service - /metrics, /health");

    let routes = metrics_route.or(health_route).or(root_route);
    
    // Start HTTP server in background
    let server = tokio::spawn(async move {
        warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    });
    
    // Start consuming messages (this will run forever)
    let consume_result = consumer.start_consuming().await;
    
    // If consuming fails, we'll still have the HTTP server running
    if let Err(e) = consume_result {
        info!("Consumer stopped with error: {}, but metrics server still running", e);
    }
    
    // Keep the server running
    server.await?;
    
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
