#!/bin/bash

# Script to create Kafka topics for Quiry
# Run this after starting the Docker Compose services

KAFKA_CONTAINER="quiry-kafka"

echo "Creating Kafka topics for Quiry..."

# Create topics with appropriate configurations
docker exec $KAFKA_CONTAINER kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic discord-messages \
  --partitions 3 \
  --replication-factor 1 \
  --config cleanup.policy=delete \
  --config retention.ms=604800000

docker exec $KAFKA_CONTAINER kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic message-chunks \
  --partitions 3 \
  --replication-factor 1 \
  --config cleanup.policy=delete \
  --config retention.ms=604800000

docker exec $KAFKA_CONTAINER kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic embedding-requests \
  --partitions 3 \
  --replication-factor 1 \
  --config cleanup.policy=delete \
  --config retention.ms=604800000

docker exec $KAFKA_CONTAINER kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic pinecone-upserts \
  --partitions 3 \
  --replication-factor 1 \
  --config cleanup.policy=delete \
  --config retention.ms=604800000

docker exec $KAFKA_CONTAINER kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic query-requests \
  --partitions 3 \
  --replication-factor 1 \
  --config cleanup.policy=delete \
  --config retention.ms=604800000

echo "Topics created successfully!"
echo ""
echo "List of topics:"
docker exec $KAFKA_CONTAINER kafka-topics --list --bootstrap-server localhost:9092

echo ""
echo "Kafka UI available at: http://localhost:8080"
echo "Schema Registry available at: http://localhost:8081"
