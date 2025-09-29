use elasticsearch::{Elasticsearch, IndexParts, SearchParts, DeleteParts};
use elasticsearch::http::transport::{Transport, TransportBuilder};
use elasticsearch::indices::{IndicesCreateParts, IndicesExistsParts};
use serde_json::{json, Value};
use tracing::{info, error, warn};
use crate::{config::Config, schema::MessageEvent};

type DynErr = Box<dyn std::error::Error + Send + Sync>;

pub struct ElasticsearchClient {
    client: Elasticsearch,
    index_name: String,
}

#[derive(Debug, Clone)]
pub struct ESQueryResult {
    pub text: String,
    pub author_id: String,
    pub channel_id: String,
    pub timestamp: String,
    pub guild_id: Option<String>,
    pub score: f64,
}

impl ElasticsearchClient {
    pub async fn new(cfg: &Config) -> Result<Self, DynErr> {
        let transport = TransportBuilder::new()
            .url(&cfg.elasticsearch_url)?
            .build()?;
        
        let client = Elasticsearch::new(transport);
        
        let es_client = Self {
            client,
            index_name: cfg.elasticsearch_index.clone(),
        };
        
        // Create index with proper mappings
        es_client.create_index().await?;
        
        Ok(es_client)
    }

    async fn create_index(&self) -> Result<(), DynErr> {
        let index_exists = self.client
            .indices()
            .exists(IndicesExistsParts::Index(&[&self.index_name]))
            .send()
            .await?;

        if !index_exists.status_code().is_success() {
            info!("Creating ElasticSearch index: {}", self.index_name);
            
            let body = json!({
                "mappings": {
                    "properties": {
                        "message_id": {
                            "type": "keyword"
                        },
                        "guild_id": {
                            "type": "keyword"
                        },
                        "channel_id": {
                            "type": "keyword"
                        },
                        "author_id": {
                            "type": "keyword"
                        },
                        "text": {
                            "type": "text",
                            "analyzer": "standard",
                            "fields": {
                                "raw": {
                                    "type": "keyword"
                                }
                            }
                        },
                        "timestamp": {
                            "type": "date",
                            "format": "strict_date_optional_time||epoch_millis"
                        },
                        "created_at": {
                            "type": "date",
                            "format": "strict_date_optional_time||epoch_millis"
                        }
                    }
                },
                "settings": {
                    "number_of_shards": 1,
                    "number_of_replicas": 0,
                    "analysis": {
                        "analyzer": {
                            "standard": {
                                "type": "standard",
                                "stopwords": "_english_"
                            }
                        }
                    }
                }
            });

            self.client
                .indices()
                .create(IndicesCreateParts::Index(&self.index_name))
                .body(body)
                .send()
                .await?;

            info!("ElasticSearch index created successfully");
        } else {
            info!("ElasticSearch index already exists");
        }

        Ok(())
    }

    pub async fn index_message(&self, message: &MessageEvent) -> Result<(), DynErr> {
        let doc = json!({
            "message_id": message.id,
            "guild_id": message.guild_id,
            "channel_id": message.channel_id,
            "author_id": message.author_id,
            "text": message.text,
            "timestamp": message.timestamp,
            "created_at": chrono::Utc::now().to_rfc3339()
        });

        let response = self.client
            .index(IndexParts::IndexId(&self.index_name, &message.id))
            .body(doc)
            .send()
            .await?;

        if response.status_code().is_success() {
            info!(message_id = %message.id, "Indexed message to ElasticSearch");
        } else {
            error!(message_id = %message.id, "Failed to index message to ElasticSearch");
        }

        Ok(())
    }

    pub async fn delete_message(&self, message_id: &str) -> Result<(), DynErr> {
        let response = self.client
            .delete(DeleteParts::IndexId(&self.index_name, message_id))
            .send()
            .await?;

        if response.status_code().is_success() {
            info!(message_id = %message_id, "Deleted message from ElasticSearch");
        } else {
            warn!(message_id = %message_id, "Message not found in ElasticSearch for deletion");
        }

        Ok(())
    }

    pub async fn search_messages(
        &self,
        query: &str,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        author_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ESQueryResult>, DynErr> {
        let mut must_clauses = vec![
            json!({
                "multi_match": {
                    "query": query,
                    "fields": ["text^2", "text.raw"],
                    "type": "best_fields",
                    "fuzziness": "AUTO"
                }
            })
        ];

        // Add filters
        if let Some(guild_id) = guild_id {
            must_clauses.push(json!({
                "term": {
                    "guild_id": guild_id
                }
            }));
        }

        if let Some(channel_id) = channel_id {
            must_clauses.push(json!({
                "term": {
                    "channel_id": channel_id
                }
            }));
        }

        if let Some(author_id) = author_id {
            must_clauses.push(json!({
                "term": {
                    "author_id": author_id
                }
            }));
        }

        let search_body = json!({
            "query": {
                "bool": {
                    "must": must_clauses
                }
            },
            "size": limit,
            "sort": [
                { "_score": { "order": "desc" } },
                { "timestamp": { "order": "desc" } }
            ]
        });

        let response = self.client
            .search(SearchParts::Index(&[&self.index_name]))
            .body(search_body)
            .send()
            .await?;

        if !response.status_code().is_success() {
            return Err("ElasticSearch search failed".into());
        }

        let response_body: Value = response.json().await?;
        let hits = response_body["hits"]["hits"].as_array().unwrap_or(&vec![]);

        let mut results = Vec::new();
        for hit in hits {
            let source = &hit["_source"];
            let score = hit["_score"].as_f64().unwrap_or(0.0);

            if let (Some(text), Some(author_id), Some(channel_id), Some(timestamp)) = (
                source["text"].as_str(),
                source["author_id"].as_str(),
                source["channel_id"].as_str(),
                source["timestamp"].as_str(),
            ) {
                results.push(ESQueryResult {
                    text: text.to_string(),
                    author_id: author_id.to_string(),
                    channel_id: channel_id.to_string(),
                    timestamp: timestamp.to_string(),
                    guild_id: source["guild_id"].as_str().map(|s| s.to_string()),
                    score,
                });
            }
        }

        info!(count = results.len(), "Found {} messages in ElasticSearch", results.len());
        Ok(results)
    }

    pub async fn health_check(&self) -> Result<bool, DynErr> {
        let response = self.client
            .cluster()
            .health()
            .send()
            .await?;

        Ok(response.status_code().is_success())
    }
}

