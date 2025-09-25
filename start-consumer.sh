#!/bin/bash

# Simple script to start just the Kafka consumer
# Run this in one terminal, then run the bot in another

echo "Starting Kafka Consumer Service..."
cd main
cargo run --bin consumer
