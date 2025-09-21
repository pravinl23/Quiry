mod config;
mod schema;
mod cohere;
mod pinecone;
mod handler;

use dotenv::dotenv;
use serenity::prelude::*;
use tracing_subscriber;
use crate::{config::Config, handler::Handler};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler { cfg: cfg };

    let mut client = Client::builder(&handler.cfg.discord_token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {err:?}");
    }
}
