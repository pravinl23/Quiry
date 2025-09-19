use dotenv::dotenv;
use serenity::{
    async_trait,
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, Interaction},
    builder::CreateCommand,
    model::{gateway::Ready, id::GuildId},
    prelude::*,
};
use std::env;

const GUILD_ID: u64 = 1383876896420528179;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else { return };

        if command.data.name == "hello" {
            let resp = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Hello, world!"),
            );

            if let Err(err) = command.create_response(&ctx.http, resp).await {
                eprintln!("Cannot respond to /hello: {err:?}");
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        // Register /hello in the test guild for instant availability
        let guild_id = GuildId::new(GUILD_ID);
        let cmd = CreateCommand::new("hello").description("Say hello to the bot");

        if let Err(err) = guild_id.create_command(&ctx.http, cmd).await {
            eprintln!("Failed to register /hello: {err:?}");
        } else {
            println!("Slash command /hello registered.");
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in env (create a .env if running locally)");

    // Intents not required for slash-only bots
    let intents = GatewayIntents::empty();

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {err:?}");
    }
}
