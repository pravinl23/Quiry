use prometheus::{Counter, Histogram, Gauge, Registry, TextEncoder, HistogramOpts, Opts};
use tracing::error;

lazy_static::lazy_static! {
    // Message processing metrics
    pub static ref MESSAGES_PROCESSED: Counter = Counter::with_opts(
        Opts::new("quiry_messages_processed_total", "Total number of messages processed")
    ).unwrap();
    
    pub static ref MESSAGES_FAILED: Counter = Counter::with_opts(
        Opts::new("quiry_messages_failed_total", "Total number of messages that failed processing")
    ).unwrap();
    
    // Latency metrics
    pub static ref MESSAGE_PROCESSING_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_message_processing_duration_seconds", "Time spent processing messages")
    ).unwrap();
    
    pub static ref EMBEDDING_GENERATION_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_embedding_generation_duration_seconds", "Time spent generating embeddings")
    ).unwrap();
    
    pub static ref PINECONE_UPSERT_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_pinecone_upsert_duration_seconds", "Time spent upserting to Pinecone")
    ).unwrap();
    
    pub static ref ELASTICSEARCH_INDEX_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_elasticsearch_index_duration_seconds", "Time spent indexing to ElasticSearch")
    ).unwrap();
    
    pub static ref DISCORD_API_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_discord_api_duration_seconds", "Time spent on Discord API calls")
    ).unwrap();
    
    // Kafka metrics
    pub static ref KAFKA_MESSAGES_SENT: Counter = Counter::with_opts(
        Opts::new("quiry_kafka_messages_sent_total", "Total number of messages sent to Kafka")
    ).unwrap();
    
    pub static ref KAFKA_MESSAGES_RECEIVED: Counter = Counter::with_opts(
        Opts::new("quiry_kafka_messages_received_total", "Total number of messages received from Kafka")
    ).unwrap();
    
    // Search metrics
    pub static ref SEARCH_REQUESTS: Counter = Counter::with_opts(
        Opts::new("quiry_search_requests_total", "Total number of search requests")
    ).unwrap();
    
    pub static ref SEARCH_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("quiry_search_duration_seconds", "Time spent on search operations")
    ).unwrap();
    
    // Health metrics
    pub static ref ACTIVE_CONNECTIONS: Gauge = Gauge::with_opts(
        Opts::new("quiry_active_connections", "Number of active connections")
    ).unwrap();
    
    pub static ref MEMORY_USAGE: Gauge = Gauge::with_opts(
        Opts::new("quiry_memory_usage_bytes", "Memory usage in bytes")
    ).unwrap();
}

pub struct MetricsRegistry {
    registry: Registry,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // Register all metrics
        registry.register(Box::new(MESSAGES_PROCESSED.clone())).unwrap();
        registry.register(Box::new(MESSAGES_FAILED.clone())).unwrap();
        registry.register(Box::new(MESSAGE_PROCESSING_DURATION.clone())).unwrap();
        registry.register(Box::new(EMBEDDING_GENERATION_DURATION.clone())).unwrap();
        registry.register(Box::new(PINECONE_UPSERT_DURATION.clone())).unwrap();
        registry.register(Box::new(ELASTICSEARCH_INDEX_DURATION.clone())).unwrap();
        registry.register(Box::new(DISCORD_API_DURATION.clone())).unwrap();
        registry.register(Box::new(KAFKA_MESSAGES_SENT.clone())).unwrap();
        registry.register(Box::new(KAFKA_MESSAGES_RECEIVED.clone())).unwrap();
        registry.register(Box::new(SEARCH_REQUESTS.clone())).unwrap();
        registry.register(Box::new(SEARCH_DURATION.clone())).unwrap();
        registry.register(Box::new(ACTIVE_CONNECTIONS.clone())).unwrap();
        registry.register(Box::new(MEMORY_USAGE.clone())).unwrap();
        
        Self { registry }
    }
    
    pub fn gather_metrics(&self) -> String {
        let metric_families = self.registry.gather();
        let encoder = TextEncoder::new();
        encoder.encode_to_string(&metric_families).unwrap_or_else(|e| {
            error!("Failed to encode metrics: {}", e);
            String::new()
        })
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Helper macros for easy metric recording
#[macro_export]
macro_rules! record_duration {
    ($histogram:expr, $code:block) => {{
        let _timer = $histogram.start_timer();
        $code
    }};
}

#[macro_export]
macro_rules! increment_counter {
    ($counter:expr) => {
        $counter.inc();
    };
}

#[macro_export]
macro_rules! increment_counter_by {
    ($counter:expr, $value:expr) => {
        $counter.inc_by($value);
    };
}
