# Kafka Setup for Quiry

This document explains how to set up and run Quiry with Kafka integration.

## Prerequisites

- Docker and Docker Compose
- Rust toolchain
- Your existing environment variables (Discord, Cohere, Pinecone)

## Environment Variables

Add these new environment variables to your `.env` file:

```bash
# Kafka Configuration
KAFKA_BROKERS=localhost:9092
KAFKA_GROUP_ID=quiry-bot
```

## Quick Start

1. **Start Kafka services:**
   ```bash
   docker-compose up -d
   ```

2. **Create Kafka topics:**
   ```bash
   ./setup-kafka.sh
   ```

3. **Run the bot:**
   ```bash
   cd main
   cargo run
   ```

## Services

- **Kafka**: Message broker (localhost:9092)
- **Zookeeper**: Kafka coordination (localhost:2181)
- **Kafka UI**: Web interface for monitoring (http://localhost:8080)
- **Schema Registry**: Message schema management (http://localhost:8081)

## Topics

The following topics are created:

- `discord-messages` - Raw Discord messages
- `message-chunks` - Processed message chunks
- `embedding-requests` - Requests for embedding generation
- `pinecone-upserts` - Vector database operations
- `query-requests` - User query processing

## Architecture

```
Discord Messages → Kafka Producer → discord-messages topic → Kafka Consumer → Processing
```

## Monitoring

- Visit http://localhost:8080 to see Kafka UI
- Monitor message flow, consumer lag, and topic health
- View message contents and metadata

## Troubleshooting

1. **Kafka not starting**: Check if ports 9092, 2181, 8080, 8081 are available
2. **Consumer not receiving messages**: Verify topics exist and consumer group is correct
3. **Producer errors**: Check Kafka broker connectivity and topic configuration

## Development

- Topics are configured with 3 partitions for parallel processing
- Messages are retained for 7 days
- Auto-creation of topics is enabled for development
