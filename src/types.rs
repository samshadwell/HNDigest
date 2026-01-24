use crate::strategies::DigestStrategy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    #[serde(rename = "objectID")]
    pub object_id: String,
    pub title: String,
    pub url: Option<String>,
    pub points: i32,
    pub created_at: String, // Algolia returns ISO string usually, we can keep as string for simplicity or parse
}

/// A subscriber record stored in DynamoDB.
/// PK="SUBSCRIBER", SK="{email}"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    pub email: String,
    pub strategy: DigestStrategy,
    pub subscribed_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub unsubscribe_token: String,
}

impl Subscriber {
    /// Create a new subscriber with a generated unsubscribe token.
    pub fn new(email: String, strategy: DigestStrategy) -> Self {
        Self {
            email,
            strategy,
            subscribed_at: Utc::now(),
            verified_at: None,
            unsubscribe_token: uuid::Uuid::new_v4().to_string(),
        }
    }
}
