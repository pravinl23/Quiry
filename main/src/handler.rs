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
};

pub struct Handler {
    pub cfg: Config,
    pub chunk_manager: Mutex<ChunkManager>,
    pub kafka_producer: Option<KafkaProducer>,
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
                    if let Some(question_option) = command.data.options.first() {
                        if let Some(question) = question_option.value.as_str() {
                            info!("Processing /ask question: {}", question);

                            let initial_resp = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new().content("🔍 Searching for relevant messages..."),
                            );
                            if let Err(err) = command.create_response(&ctx.http, initial_resp).await {
                                error!("Cannot send initial response: {err:?}");
                                return;
                            }

                            let guild_id = command.guild_id.map(|id| id.to_string());
                            match self.handle_ask_command(question, guild_id).await {
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
        })
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
