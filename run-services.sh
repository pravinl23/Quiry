#!/bin/bash

# Script to run both Discord bot and Kafka consumer services
# This ensures messages are properly processed through Kafka

echo "Starting Quiry Services..."

# Change to the main directory where Cargo.toml is located
cd main

# Start Kafka consumer in background
echo "Starting Kafka Consumer Service..."
cargo run --bin consumer &
CONSUMER_PID=$!

# Wait a moment for consumer to start
sleep 3

# Start ElasticSearch indexer in background
echo "Starting ElasticSearch Indexer Service..."
cargo run --bin indexer &
INDEXER_PID=$!

# Wait a moment for indexer to start
sleep 3

# Start Discord bot
echo "Starting Discord Bot"
cargo run --bin Quiry &
BOT_PID=$!

echo "All services started!"
echo "Consumer PID: $CONSUMER_PID"
echo "Indexer PID: $INDEXER_PID"
echo "Bot PID: $BOT_PID"
echo ""
echo "Press Ctrl+C to stop both services"

# Wait for user interrupt
trap "echo 'Stopping services...'; kill $CONSUMER_PID $INDEXER_PID $BOT_PID; exit" INT
wait
