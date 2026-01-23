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
}
