use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
    builder::{CreateCommand, CreateCommandOption},
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, CreateInteractionResponseFollowup, Interaction, CommandOptionType},
};
use tracing::{info, error, warn};
use tokio::sync::Mutex;
use crate::{
    config::Config,
    schema::MessageEvent,
    cohere::{get_embedding, generate_response, generate_response_from_chunks},
    pinecone::{upsert_to_pinecone, query_pinecone, query_chunks_pinecone},
    chunking::ChunkManager,
    kafka_producer::KafkaProducer,
    kafka_types::KafkaMessage,
    elasticsearch::ElasticsearchClient,
};

pub struct Handler {
    pub cfg: Config,
    pub chunk_manager: Mutex<ChunkManager>,
    pub kafka_producer: Option<KafkaProducer>,
    pub es_client: Option<ElasticsearchClient>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let hello_cmd = CreateCommand::new("hello").description("Say hello to the bot");
        if let Err(err) = ctx.http.create_global_command(&hello_cmd).await {
            error!("Failed to register global /hello: {err:?}");
        } else {
            info!("Global slash command /hello registered.");
        }

        let ask_cmd = CreateCommand::new("ask")
            .description("Ask a question based on message history")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "question", "Your question")
                    .required(true)
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "channel", "Filter by channel name (optional)")
                    .required(false)
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::User, "author", "Filter by specific user (optional)")
                    .required(false)
            );
        if let Err(err) = ctx.http.create_global_command(&ask_cmd).await {
            error!("Failed to register global /ask: {err:?}");
        } else {
            info!("Global slash command /ask registered.");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            match command.data.name.as_str() {
                "hello" => {
                    let resp = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content("Hello, world!"),
                    );
                    if let Err(err) = command.create_response(&ctx.http, resp).await {
                        error!("Cannot respond to /hello: {err:?}");
                    }
                }
                "ask" => {
                    // Parse command options
                    let mut question = None;
                    let mut channel_filter = None;
                    let mut author_filter = None;
                    
                    for option in &command.data.options {
                        match option.name.as_str() {
                            "question" => {
                                if let Some(value) = option.value.as_str() {
                                    question = Some(value);
                                }
                            }
                            "channel" => {
                                if let Some(value) = option.value.as_str() {
                                    channel_filter = Some(value);
                                }
                            }
                            "author" => {
                                if let Some(value) = option.value.as_user_id() {
                                    author_filter = Some(value.to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(question) = question {
                        info!("Processing /ask question: {} (channel: {:?}, author: {:?})", 
                              question, channel_filter, author_filter);

                        let initial_resp = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("🔍 Searching for relevant messages..."),
                        );
                        if let Err(err) = command.create_response(&ctx.http, initial_resp).await {
                            error!("Cannot send initial response: {err:?}");
                            return;
                        }

                        let guild_id = command.guild_id.map(|id| id.to_string());
                        match self.handle_ask_command_with_filters(question, guild_id, channel_filter, author_filter).await {
                            Ok(response) => {
                                let followup = CreateInteractionResponseFollowup::new().content(response);
                                if let Err(err) = command.create_followup(&ctx.http, followup).await {
                                    error!("Cannot send followup response: {err:?}");
                                }
                            }
                            Err(err) => {
                                error!("Failed to process /ask: {err}");
                                let error_resp = CreateInteractionResponseFollowup::new()
                                    .content("Sorry, I encountered an error while processing your question.");
                                if let Err(err) = command.create_followup(&ctx.http, error_resp).await {
                                    error!("Cannot send error response: {err:?}");
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot { return; }

        let event = MessageEvent {
            id: msg.id.to_string(),
            guild_id: msg.guild_id.map(|id| id.to_string()),
            channel_id: msg.channel_id.to_string(),
            author_id: msg.author.id.to_string(),
            timestamp: msg.timestamp.to_rfc3339().unwrap_or_else(|| "".to_string()),
            text: msg.content.clone(),
        };

        // Try to send to Kafka if available, otherwise process directly
        if let Some(ref producer) = self.kafka_producer {
            let kafka_message = KafkaMessage::new_discord_message(event.clone());
            if let Err(err) = producer.send_discord_message(kafka_message).await {
                error!("Failed to send message to Kafka: {err}");
                // Fallback to direct processing
                self.process_message_directly(event).await;
            }
        } else {
            // Process directly without Kafka
            info!("Processing message directly (Kafka not available): {}", event.text);
            self.process_message_directly(event).await;
        }
    }
}

impl Handler {
    pub fn new(cfg: Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Try to create Kafka producer, but don't fail if Kafka is not available
        let kafka_producer = match KafkaProducer::new(&cfg) {
            Ok(producer) => {
                info!("Kafka producer initialized successfully");
                Some(producer)
            }
            Err(err) => {
                warn!("Failed to initialize Kafka producer: {}. Running in fallback mode.", err);
                None
            }
        };
        
        Ok(Self {
            cfg,
            chunk_manager: Mutex::new(ChunkManager::new()),
            kafka_producer,
            es_client: None, // Will be initialized asynchronously
        })
    }

    pub async fn initialize_es_client(&self) -> Option<ElasticsearchClient> {
        match ElasticsearchClient::new(&self.cfg).await {
            Ok(client) => {
                info!("ElasticSearch client initialized successfully");
                Some(client)
            }
            Err(err) => {
                warn!("Failed to initialize ElasticSearch client: {}. Running without ES.", err);
                None
            }
        }
    }

    async fn hybrid_search(
        &self,
        query: &str,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        author_id: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Get Pinecone results (semantic search)
        let pinecone_results = if let Some(guild_id) = guild_id {
            let embedding = get_embedding(&self.cfg, query).await?;
            query_chunks_pinecone(
                &self.cfg,
                embedding,
                5,
                Some(guild_id.to_string()),
            ).await?
        } else {
            vec![]
        };

        // Get ElasticSearch results (keyword search)
        let es_results = if let Some(ref es_client) = self.es_client {
            es_client.search_messages(query, guild_id, channel_id, author_id, 5).await?
        } else {
            vec![]
        };

        // Combine and merge results
        let combined_results = self.merge_search_results(pinecone_results, es_results, 0.65).await?;
        
        if combined_results.is_empty() {
            return Ok("I couldn't find any relevant information about that topic.".to_string());
        }

        // Generate response from combined results
        let context_chunks: Vec<crate::schema::ChunkQueryResult> = combined_results.iter()
            .map(|result| crate::schema::ChunkQueryResult {
                chunk_id: result.text.clone(),
                text: result.text.clone(),
                summary: None,
                authors: vec![result.author_id.clone()],
                message_count: 1,
                first_timestamp: result.timestamp.clone(),
                last_timestamp: result.timestamp.clone(),
                score: result.score,
            })
            .collect();

        generate_response_from_chunks(&self.cfg, query, &context_chunks).await
    }

    async fn merge_search_results(
        &self,
        pinecone_results: Vec<crate::schema::ChunkQueryResult>,
        es_results: Vec<crate::elasticsearch::ESQueryResult>,
        alpha: f64,
    ) -> Result<Vec<crate::elasticsearch::ESQueryResult>, Box<dyn std::error::Error + Send + Sync>> {
        use std::collections::HashMap;
        
        let mut combined_scores: HashMap<String, (f64, crate::elasticsearch::ESQueryResult)> = HashMap::new();
        
        // Add Pinecone results (normalize scores to 0-1)
        for result in pinecone_results {
            let normalized_score = (result.score + 1.0) / 2.0; // Convert from [-1,1] to [0,1]
            let final_score = alpha * normalized_score;
            
            let es_result = crate::elasticsearch::ESQueryResult {
                text: result.text.clone(),
                author_id: result.authors.first().unwrap_or(&"unknown".to_string()).clone(),
                channel_id: "unknown".to_string(), // ChunkQueryResult doesn't have channel_id
                timestamp: result.first_timestamp.clone(),
                guild_id: None, // ChunkQueryResult doesn't have guild_id
                score: final_score,
            };
            
            combined_scores.insert(result.chunk_id, (final_score, es_result));
        }
        
        // Add ElasticSearch results
        for result in es_results {
            let normalized_score = result.score / 10.0; // Rough normalization
            let final_score = (1.0 - alpha) * normalized_score;
            
            if let Some((existing_score, _)) = combined_scores.get(&result.text) {
                // If we have both Pinecone and ES results for the same content, take the max
                if final_score > *existing_score {
                    combined_scores.insert(result.text.clone(), (final_score, result));
                }
            } else {
                combined_scores.insert(result.text.clone(), (final_score, result));
            }
        }
        
        // Sort by combined score and return top results
        let mut results: Vec<_> = combined_scores.into_values().collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results.into_iter().map(|(_, result)| result).collect())
    }

    async fn process_message_directly(&self, event: MessageEvent) {
        // Process message through chunking system
        let mut chunk_manager = self.chunk_manager.lock().await;
        if let Err(err) = chunk_manager.process_message(&self.cfg, event.clone()).await {
            error!("Failed to process message through chunking: {err}");
        }

        // Keep individual message processing as fallback/compatibility
        match get_embedding(&self.cfg, &event.text).await {
            Ok(embedding) => {
                if let Err(err) = upsert_to_pinecone(&self.cfg, &event, embedding).await {
                    error!("Failed to upsert individual message: {err}");
                }
            }
            Err(err) => error!("Individual message embedding failed: {err}"),
        }
    }

    async fn handle_ask_command_with_filters(
        &self, 
        question: &str, 
        guild_id: Option<String>,
        channel_filter: Option<&str>,
        author_filter: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize ES client if not already done
        if self.es_client.is_none() {
            // Note: This is a simplified approach. In production, you'd want to handle this more carefully
            // to avoid race conditions and ensure proper initialization
            info!("ElasticSearch client not initialized, using Pinecone-only search");
        }

        // Use hybrid search if ES is available, otherwise fallback to Pinecone-only
        if let Some(ref _es_client) = self.es_client {
            self.hybrid_search(
                question,
                guild_id.as_deref(),
                channel_filter,
                author_filter.as_deref(),
            ).await
        } else {
            // Fallback to original Pinecone-only search
            self.handle_ask_command(question, guild_id).await
        }
    }

    async fn handle_ask_command(&self, question: &str, guild_id: Option<String>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("Getting embedding for question: {}", question);
        let question_embedding = get_embedding(&self.cfg, question).await?;

        // First try to query chunks
        info!("Querying Pinecone for similar chunks in guild: {:?}", guild_id);
        let similar_chunks = query_chunks_pinecone(&self.cfg, question_embedding.clone(), 3, guild_id.clone()).await?;

        if !similar_chunks.is_empty() {
            info!("Found {} similar chunks, generating response", similar_chunks.len());
            let response = generate_response_from_chunks(&self.cfg, question, &similar_chunks).await?;
            return Ok(response);
        }

        // Fallback to individual messages
        info!("No chunks found, querying individual messages in guild: {:?}", guild_id);
        let similar_messages = query_pinecone(&self.cfg, question_embedding, 5, guild_id).await?;

        if similar_messages.is_empty() {
            return Ok("I couldn't find any relevant messages in the history to answer your question.".to_string());
        }

        info!("Found {} similar messages, generating response", similar_messages.len());
        let response = generate_response(&self.cfg, question, &similar_messages).await?;

        Ok(response)
    }
}
