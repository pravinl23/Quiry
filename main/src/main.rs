use dotenv::dotenv;
use serenity::prelude::*;
use tracing_subscriber;
use Quiry::{config::Config, handler::Handler};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut handler = Handler::new(cfg).expect("Failed to create handler");
    
    // Initialize ElasticSearch client asynchronously
    if let Some(es_client) = handler.initialize_es_client().await {
        handler.es_client = Some(es_client);
    }

    let mut client = Client::builder(&handler.cfg.discord_token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(err) = client.start().await {
        eprintln!("Client error: {err:?}");
    }
}
