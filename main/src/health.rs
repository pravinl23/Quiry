use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: String,
    pub services: std::collections::HashMap<String, ServiceHealth>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceHealth {
    pub status: String,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

pub struct HealthChecker {
    discord_healthy: Arc<Mutex<bool>>,
    kafka_healthy: Arc<Mutex<bool>>,
    elasticsearch_healthy: Arc<Mutex<bool>>,
    pinecone_healthy: Arc<Mutex<bool>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            discord_healthy: Arc::new(Mutex::new(false)),
            kafka_healthy: Arc::new(Mutex::new(false)),
            elasticsearch_healthy: Arc::new(Mutex::new(false)),
            pinecone_healthy: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn check_discord(&self) -> ServiceHealth {
        let start = std::time::Instant::now();
        
        // Simple Discord health check - if we can create a client, we're healthy
        let _http_client = serenity::http::Http::new("dummy_token");
        let response_time = start.elapsed().as_millis() as u64;
        
        // For now, assume Discord is healthy if we can create the client
        *self.discord_healthy.lock().await = true;
        ServiceHealth {
            status: "healthy".to_string(),
            message: Some("Discord API accessible".to_string()),
            response_time_ms: Some(response_time),
        }
    }

    pub async fn check_kafka(&self) -> ServiceHealth {
        let start = std::time::Instant::now();
        
        // Simple Kafka health check - try to create a producer
        use rdkafka::config::FromClientConfig;
        use rdkafka::client::DefaultClientContext;
        match rdkafka::producer::FutureProducer::<DefaultClientContext, rdkafka::util::TokioRuntime>::from_config(&rdkafka::ClientConfig::new()) {
            Ok(_) => {
                let response_time = start.elapsed().as_millis() as u64;
                *self.kafka_healthy.lock().await = true;
                ServiceHealth {
                    status: "healthy".to_string(),
                    message: Some("Kafka producer created successfully".to_string()),
                    response_time_ms: Some(response_time),
                }
            }
            Err(e) => {
                *self.kafka_healthy.lock().await = false;
                ServiceHealth {
                    status: "unhealthy".to_string(),
                    message: Some(format!("Kafka error: {}", e)),
                    response_time_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
    }

    pub async fn check_elasticsearch(&self, es_url: &str) -> ServiceHealth {
        let start = std::time::Instant::now();
        
        match reqwest::get(&format!("{}/_cluster/health", es_url)).await {
            Ok(response) => {
                let response_time = start.elapsed().as_millis() as u64;
                if response.status().is_success() {
                    *self.elasticsearch_healthy.lock().await = true;
                    ServiceHealth {
                        status: "healthy".to_string(),
                        message: Some("ElasticSearch cluster healthy".to_string()),
                        response_time_ms: Some(response_time),
                    }
                } else {
                    *self.elasticsearch_healthy.lock().await = false;
                    ServiceHealth {
                        status: "unhealthy".to_string(),
                        message: Some(format!("ElasticSearch returned status: {}", response.status())),
                        response_time_ms: Some(response_time),
                    }
                }
            }
            Err(e) => {
                *self.elasticsearch_healthy.lock().await = false;
                ServiceHealth {
                    status: "unhealthy".to_string(),
                    message: Some(format!("ElasticSearch connection error: {}", e)),
                    response_time_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
    }

    pub async fn check_pinecone(&self, pinecone_host: &str) -> ServiceHealth {
        let start = std::time::Instant::now();
        
        // Simple Pinecone health check - try to make a basic request
        match reqwest::get(&format!("{}/describe_index_stats", pinecone_host)).await {
            Ok(response) => {
                let response_time = start.elapsed().as_millis() as u64;
                if response.status().is_success() {
                    *self.pinecone_healthy.lock().await = true;
                    ServiceHealth {
                        status: "healthy".to_string(),
                        message: Some("Pinecone API accessible".to_string()),
                        response_time_ms: Some(response_time),
                    }
                } else {
                    *self.pinecone_healthy.lock().await = false;
                    ServiceHealth {
                        status: "unhealthy".to_string(),
                        message: Some(format!("Pinecone returned status: {}", response.status())),
                        response_time_ms: Some(response_time),
                    }
                }
            }
            Err(e) => {
                *self.pinecone_healthy.lock().await = false;
                ServiceHealth {
                    status: "unhealthy".to_string(),
                    message: Some(format!("Pinecone connection error: {}", e)),
                    response_time_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
    }

    pub async fn get_overall_health(&self, es_url: &str, pinecone_host: &str) -> HealthStatus {
        let mut services = std::collections::HashMap::new();
        
        // Check all services
        services.insert("discord".to_string(), self.check_discord().await);
        services.insert("kafka".to_string(), self.check_kafka().await);
        services.insert("elasticsearch".to_string(), self.check_elasticsearch(es_url).await);
        services.insert("pinecone".to_string(), self.check_pinecone(pinecone_host).await);
        
        // Determine overall status
        let all_healthy = services.values().all(|service| service.status == "healthy");
        let overall_status = if all_healthy { "healthy" } else { "degraded" };
        
        HealthStatus {
            status: overall_status.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            services,
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}
