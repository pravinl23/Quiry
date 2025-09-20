use dotenv::dotenv;
use serenity::{
    async_trait,
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, Interaction},
    builder::CreateCommand,
    model::{channel::Message, gateway::Ready, id::GuildId},
    prelude::*,
};
use std::env;
use reqwest::Client as HttpClient;
use serenity::Client as DiscordClient;
use serde_json::json;

type DynErr = Box<dyn std::error::Error + Send + Sync>;

const GUILD_ID: u64 = 1383876896420528179;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            if command.data.name == "hello" {
                let resp = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content("Hello, world!"),
                );
                if let Err(err) = command.create_response(&ctx.http, resp).await {
                    eprintln!("Cannot respond to /hello: {err:?}");
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        let guild_id = GuildId::new(GUILD_ID);
        let cmd = CreateCommand::new("hello").description("Say hello to the bot");
        if let Err(err) = guild_id.create_command(&ctx.http, cmd).await {
            eprintln!("Failed to register /hello: {err:?}");
        } else {
            println!("Slash command /hello registered.");
        }
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot { return; }

        match get_embedding(&msg.content).await {
            Ok(embedding) => {
                println!("Message: {}", msg.content);
                println!("Embedding sample: {:?}", &embedding[..5.min(embedding.len())]);

                if let Err(err) =
                    upsert_to_pinecone(&msg.id.to_string(), embedding, &msg.content).await
                {
                    eprintln!("Failed to upsert to Pinecone: {err}");
                }
            }
            Err(err) => eprintln!("Failed to get embedding: {err}"),
        }
    }
}

async fn get_embedding(text: &str) -> Result<Vec<f32>, DynErr> {
    let api_key = env::var("COHERE_API_KEY")?;
    let client = HttpClient::new();

    let res = client
        .post("https://api.cohere.ai/v1/embed")
        .bearer_auth(api_key)
        .json(&json!({
            "model": "embed-english-v3.0",
            "input_type": "search_document",
            "texts": [text]
        }))
        .send()
        .await?;

    let body: serde_json::Value = res.json().await?;
    if let Some(array) = body["embeddings"].get(0).and_then(|v| v.as_array()) {
        Ok(array.iter().filter_map(|v| v.as_f64()).map(|v| v as f32).collect())
    } else {
        Err("No embeddings found in Cohere response".into())
    }
}

async fn upsert_to_pinecone(id: &str, embedding: Vec<f32>, text: &str) -> Result<(), DynErr> {
    let api_key   = env::var("PINECONE_API_KEY")?;
    let host      = env::var("PINECONE_HOST")?;
    let index     = env::var("PINECONE_INDEX")?;
    let namespace = env::var("PINECONE_NAMESPACE").unwrap_or_else(|_| "default".into());

    let url = format!("{host}/vectors/upsert");

    let client = HttpClient::new();
    let res = client
        .post(&url)
        .header("Api-Key", api_key)                      // <-- Pinecone expects Api-Key, not Bearer
        .json(&json!({
            "namespace": namespace,
            "vectors": [{
                "id": id,
                "values": embedding,
                "metadata": { "text": text }
            }]
        }))
        .send()
        .await?;

    let status = res.status();
    let text_body = res.text().await?;                   // may be empty or non-JSON
    if !status.is_success() {
        return Err(format!("Pinecone upsert failed: status={} body={}", status, text_body).into());
    }

    // Try to parse JSON if present; otherwise just log success.
    if !text_body.trim().is_empty() {
        match serde_json::from_str::<serde_json::Value>(&text_body) {
            Ok(v) => println!("Inserted message {id} into Pinecone (index: {index}) -> {v}"),
            Err(_) => println!("Inserted message {id} into Pinecone (index: {index}); raw: {text_body}"),
        }
    } else {
        println!("Inserted message {id} into Pinecone (index: {index}); empty response body");
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in env (create a .env if running locally)");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = DiscordClient::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating Discord client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {err:?}");
    }
}
