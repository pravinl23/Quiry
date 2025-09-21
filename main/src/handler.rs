use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, id::GuildId},
    prelude::*,
    builder::{CreateCommand, CreateCommandOption},
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, CreateInteractionResponseFollowup, Interaction, CommandOptionType},
};
use tracing::{info, error};
use crate::{
    config::Config,
    schema::MessageEvent,
    cohere::{get_embedding, generate_response},
    pinecone::{upsert_to_pinecone, query_pinecone}
};

pub struct Handler {
    pub cfg: Config,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId::new(1383876896420528179);

        let hello_cmd = CreateCommand::new("hello").description("Say hello to the bot");
        if let Err(err) = guild_id.create_command(&ctx.http, hello_cmd).await {
            error!("Failed to register /hello: {err:?}");
        } else {
            info!("Slash command /hello registered.");
        }

        let ask_cmd = CreateCommand::new("ask")
            .description("Ask a question based on message history")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "question", "Your question")
                    .required(true)
            );
        if let Err(err) = guild_id.create_command(&ctx.http, ask_cmd).await {
            error!("Failed to register /ask: {err:?}");
        } else {
            info!("Slash command /ask registered.");
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

                            match self.handle_ask_command(question).await {
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

        match get_embedding(&self.cfg, &msg.content).await {
            Ok(embedding) => {
                if let Err(err) = upsert_to_pinecone(&self.cfg, &event, embedding).await {
                    error!("Failed to upsert: {err}");
                }
            }
            Err(err) => error!("Embedding failed: {err}"),
        }
    }
}

impl Handler {
    async fn handle_ask_command(&self, question: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("Getting embedding for question: {}", question);
        let question_embedding = get_embedding(&self.cfg, question).await?;

        info!("Querying Pinecone for similar messages");
        let similar_messages = query_pinecone(&self.cfg, question_embedding, 5).await?;

        if similar_messages.is_empty() {
            return Ok("I couldn't find any relevant messages in the history to answer your question.".to_string());
        }

        info!("Found {} similar messages, generating response", similar_messages.len());
        let response = generate_response(&self.cfg, question, &similar_messages).await?;

        Ok(response)
    }
}
