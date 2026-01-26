use crate::strategies::DigestStrategy;
use chrono::{DateTime, Utc};
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A non-empty token string (used for verification and unsubscribe tokens).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Token(String);

impl Token {
    /// Generate a new random token.
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl FromStr for Token {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Err("token cannot be empty")
        } else {
            Ok(Self(s.to_string()))
        }
    }
}

impl TryFrom<String> for Token {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            Err("token cannot be empty")
        } else {
            Ok(Self(s))
        }
    }
}

impl From<Token> for String {
    fn from(token: Token) -> String {
        token.0
    }
}

impl AsRef<str> for Token {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    #[serde(rename = "objectID")]
    pub object_id: String,
    pub title: String,
    pub url: Option<String>,
    pub points: i32,
    pub created_at: String, // Algolia returns ISO string usually, we can keep as string for simplicity or parse
}

/// A verified subscriber record stored in DynamoDB.
/// PK="SUBSCRIBER", SK="{email}"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    pub email: EmailAddress,
    pub strategy: DigestStrategy,
    pub subscribed_at: DateTime<Utc>,
    pub unsubscribe_token: Token,
}

impl Subscriber {
    /// Create a new verified subscriber with a generated unsubscribe token.
    pub fn new(email: EmailAddress, strategy: DigestStrategy) -> Self {
        Self {
            email,
            strategy,
            subscribed_at: Utc::now(),
            unsubscribe_token: Token::generate(),
        }
    }
}

/// A pending subscription awaiting email verification.
/// PK="PENDING_SUBSCRIPTION", SK="{email}"
/// Has a TTL of 24 hours for automatic cleanup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSubscription {
    pub email: EmailAddress,
    pub token: Token,
    pub strategy: DigestStrategy,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Set when the user clicks the verification link. Allows idempotent verification
    /// (clicking the link multiple times shows success instead of error).
    pub verified_at: Option<DateTime<Utc>>,
}

impl PendingSubscription {
    /// Create a new pending subscription with a 24-hour expiry.
    pub fn new(email: EmailAddress, strategy: DigestStrategy) -> Self {
        let now = Utc::now();
        Self {
            email,
            token: Token::generate(),
            strategy,
            created_at: now,
            expires_at: now + chrono::Duration::hours(24),
            verified_at: None,
        }
    }
}
