# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Quiry is a Discord bot written in Rust that automatically processes Discord messages using AI embeddings and vector storage. It's the second iteration of a Discord bot, focusing on modern architecture and scalability.

## Core Architecture

The bot uses a modular architecture with the following components:

- **main.rs**: Application entry point that initializes the Discord client with Serenity framework
- **handler.rs**: Discord event handler that processes messages, slash commands, and bot lifecycle events
- **config.rs**: Environment-based configuration management for API keys and service endpoints
- **cohere.rs**: Integration with Cohere API for text embeddings using the embed-english-v3.0 model
- **pinecone.rs**: Vector database operations for storing and retrieving message embeddings
- **schema.rs**: Data structures for message events and API interactions

## Key Technologies

- **Serenity 0.12**: Discord API library with framework, standard_framework, client, gateway, and rustls_backend features
- **Tokio**: Async runtime with rt-multi-thread and macros features
- **Cohere API**: Text embedding service for semantic search capabilities
- **Pinecone**: Vector database for storing and querying message embeddings
- **Reqwest**: HTTP client for API calls to external services

## Development Commands

```bash
# Build the project
cd main && cargo build

# Run the bot
cd main && cargo run

# Check for compilation errors
cd main && cargo check

# Run tests (if any exist)
cd main && cargo test

# Format code
cd main && cargo fmt

# Run clippy for linting
cd main && cargo clippy
```

## Environment Configuration

The bot requires these environment variables in `.env`:
- `DISCORD_TOKEN`: Discord bot token
- `COHERE_API_KEY`: Cohere API key for embeddings
- `PINECONE_API_KEY`: Pinecone API key
- `PINECONE_HOST`: Pinecone index host URL
- `PINECONE_INDEX`: Pinecone index name
- `PINECONE_NAMESPACE`: Pinecone namespace (defaults to "default")

## Message Processing Flow

1. Bot receives Discord message via Serenity event handler
2. Message content is sent to Cohere API for embedding generation
3. Message metadata and embedding are stored in Pinecone vector database
4. Bot ignores messages from other bots to prevent loops

## Slash Commands

Currently implements:
- `/hello`: Simple hello command for testing bot functionality

## Working Directory

All Rust code is located in the `main/` directory. When making changes or running commands, ensure you're in the `main/` directory or use `cd main &&` prefix for commands.