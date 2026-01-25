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

/// A pending subscription awaiting email verification.
/// PK="PENDING_SUBSCRIPTION", SK="{token}"
/// Has a TTL of 24 hours for automatic cleanup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSubscription {
    pub token: String,
    pub email: String,
    pub strategy: DigestStrategy,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl PendingSubscription {
    /// Create a new pending subscription with a 24-hour expiry.
    pub fn new(email: String, strategy: DigestStrategy) -> Self {
        let now = Utc::now();
        Self {
            token: uuid::Uuid::new_v4().to_string(),
            email,
            strategy,
            created_at: now,
            expires_at: now + chrono::Duration::hours(24),
        }
    }

    /// Check if this pending subscription has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}
