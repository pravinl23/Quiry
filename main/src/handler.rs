use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, id::GuildId},
    prelude::*,
    builder::CreateCommand,
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, Interaction},
};
use tracing::{info, error};
use crate::{config::Config, schema::MessageEvent, cohere::get_embedding, pinecone::upsert_to_pinecone};

pub struct Handler {
    pub cfg: Config,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId::new(1383876896420528179);
        let cmd = CreateCommand::new("hello").description("Say hello to the bot");
        if let Err(err) = guild_id.create_command(&ctx.http, cmd).await {
            error!("Failed to register /hello: {err:?}");
        } else {
            info!("Slash command /hello registered.");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            if command.data.name == "hello" {
                let resp = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content("Hello, world!"),
                );
                if let Err(err) = command.create_response(&ctx.http, resp).await {
                    error!("Cannot respond to /hello: {err:?}");
                }
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
