use std::env;

pub struct Config {
    pub discord_token: String,
    pub cohere_key: String,
    pub pinecone_key: String,
    pub pinecone_host: String,
    pub pinecone_index: String,
    pub namespace: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            discord_token: env::var("DISCORD_TOKEN")
                .expect("Expected DISCORD_TOKEN in env"),
            cohere_key: env::var("COHERE_API_KEY")
                .expect("Expected COHERE_API_KEY in env"),
            pinecone_key: env::var("PINECONE_API_KEY")
                .expect("Expected PINECONE_API_KEY in env"),
            pinecone_host: env::var("PINECONE_HOST")
                .expect("Expected PINECONE_HOST in env"),
            pinecone_index: env::var("PINECONE_INDEX")
                .expect("Expected PINECONE_INDEX in env"),
            namespace: env::var("PINECONE_NAMESPACE")
                .unwrap_or_else(|_| "default".into()),
        }
    }
}
