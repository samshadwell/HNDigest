use crate::strategies::DigestStrategy;
use crate::types::{PendingSubscription, Post, Subscriber, Token};
use anyhow::{Context, Result};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use chrono::{DateTime, Duration, Utc};
use email_address::EmailAddress;
use std::collections::HashMap;
use std::str::FromStr;

const SNAPSHOT_PARTITION_KEY: &str = "POSTS_SNAPSHOT";
const DIGEST_PARTITION_KEY_PREFIX: &str = "DIGEST";
const SUBSCRIBER_PARTITION_KEY: &str = "SUBSCRIBER";
const PENDING_SUBSCRIPTION_PARTITION_KEY: &str = "PENDING_SUBSCRIPTION";
const MODEL_TTL_DAYS: i64 = 30;
const UNSUBSCRIBE_TOKEN_INDEX: &str = "unsubscribe_token_index";

// ============================================================================
// Storage trait
// ============================================================================

#[allow(async_fn_in_trait)]
pub trait Storage: Send + Sync {
    async fn snapshot_posts(
        &self,
        posts: &HashMap<String, Post>,
        date: DateTime<Utc>,
    ) -> Result<()>;

    async fn save_digest(&self, type_: &str, date: DateTime<Utc>, posts: &[Post]) -> Result<()>;

    async fn fetch_digest(&self, type_: &str, date: DateTime<Utc>) -> Result<Option<Vec<Post>>>;

    async fn get_subscriber_by_unsubscribe_token(
        &self,
        token: &Token,
    ) -> Result<Option<Subscriber>>;

    async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>>;

    async fn upsert_subscriber(&self, subscriber: &Subscriber) -> Result<()>;

    async fn remove_subscriber(&self, email: &EmailAddress) -> Result<()>;

    async fn upsert_pending_subscription(&self, pending: &PendingSubscription) -> Result<()>;

    async fn get_pending_subscription(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<PendingSubscription>>;

    async fn get_subscriber_by_email(&self, email: &EmailAddress) -> Result<Option<Subscriber>>;
}

// ============================================================================
// LambdaStorage — DynamoDB-backed implementation
// ============================================================================

pub struct LambdaStorage {
    client: Client,
    table_name: String,
}

impl LambdaStorage {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

impl Storage for LambdaStorage {
    async fn snapshot_posts(
        &self,
        posts: &HashMap<String, Post>,
        date: DateTime<Utc>,
    ) -> Result<()> {
        let datestamp = datestamp(date);
        let posts_av = to_dynamo_map(posts)?;

        let item = HashMap::from([
            (
                "PK".to_string(),
                AttributeValue::S(SNAPSHOT_PARTITION_KEY.to_string()),
            ),
            ("SK".to_string(), AttributeValue::S(datestamp)),
            ("posts".to_string(), posts_av),
            (
                "expires_at".to_string(),
                AttributeValue::N(
                    ((date + Duration::days(MODEL_TTL_DAYS)).timestamp()).to_string(),
                ),
            ),
        ]);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to snapshot posts")?;

        Ok(())
    }

    async fn save_digest(&self, type_: &str, date: DateTime<Utc>, posts: &[Post]) -> Result<()> {
        let datestamp = datestamp(date);
        let posts_av = to_dynamo_list(posts)?;

        let item = HashMap::from([
            (
                "PK".to_string(),
                AttributeValue::S(digest_partition_key(type_)),
            ),
            ("SK".to_string(), AttributeValue::S(datestamp)),
            ("posts".to_string(), posts_av),
            (
                "expires_at".to_string(),
                AttributeValue::N(
                    ((date + Duration::days(MODEL_TTL_DAYS)).timestamp()).to_string(),
                ),
            ),
        ]);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to save digest")?;

        Ok(())
    }

    async fn fetch_digest(&self, type_: &str, date: DateTime<Utc>) -> Result<Option<Vec<Post>>> {
        let datestamp = datestamp(date);

        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("PK", AttributeValue::S(digest_partition_key(type_)))
            .key("SK", AttributeValue::S(datestamp))
            .send()
            .await
            .context("Failed to fetch digest")?;

        output
            .item
            .and_then(|item| item.get("posts").cloned())
            .map(|posts_av| from_dynamo_list(&posts_av))
            .transpose()
    }

    async fn get_subscriber_by_unsubscribe_token(
        &self,
        token: &Token,
    ) -> Result<Option<Subscriber>> {
        let output = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name(UNSUBSCRIBE_TOKEN_INDEX)
            .key_condition_expression("unsubscribe_token = :token")
            .expression_attribute_values(":token", AttributeValue::S(token.to_string()))
            .send()
            .await
            .context("Failed to query subscriber by token")?;

        let items = output.items.unwrap_or_default();

        match items.len() {
            0 => Ok(None),
            1 => items
                .into_iter()
                .next()
                .map(subscriber_from_item)
                .transpose(),
            n => anyhow::bail!(
                "Data integrity error: found {} subscribers with token '{}'. Tokens must be unique.",
                n,
                token
            ),
        }
    }

    async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>> {
        let mut subscribers = Vec::new();
        let mut exclusive_start_key = None;

        loop {
            let mut request = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression("PK = :pk")
                .expression_attribute_values(
                    ":pk",
                    AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
                );

            if let Some(start_key) = exclusive_start_key {
                request = request.set_exclusive_start_key(Some(start_key));
            }

            let output = request
                .send()
                .await
                .context("Failed to query subscribers")?;

            if let Some(items) = output.items {
                for item in items {
                    subscribers.push(subscriber_from_item(item)?);
                }
            }

            exclusive_start_key = output.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }

        Ok(subscribers)
    }

    async fn upsert_subscriber(&self, subscriber: &Subscriber) -> Result<()> {
        let email_str = subscriber.email.to_string().to_lowercase();
        let item = HashMap::from([
            (
                "PK".to_string(),
                AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
            ),
            ("SK".to_string(), AttributeValue::S(email_str.clone())),
            ("email".to_string(), AttributeValue::S(email_str)),
            (
                "strategy".to_string(),
                AttributeValue::S(subscriber.strategy.to_string()),
            ),
            (
                "subscribed_at".to_string(),
                AttributeValue::S(subscriber.subscribed_at.to_rfc3339()),
            ),
            (
                "unsubscribe_token".to_string(),
                AttributeValue::S(subscriber.unsubscribe_token.to_string()),
            ),
        ]);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to set subscriber")?;

        Ok(())
    }

    async fn remove_subscriber(&self, email: &EmailAddress) -> Result<()> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key(
                "PK",
                AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
            )
            .key("SK", AttributeValue::S(email.to_string().to_lowercase()))
            .send()
            .await
            .context("Failed to remove subscriber")?;

        Ok(())
    }

    async fn upsert_pending_subscription(&self, pending: &PendingSubscription) -> Result<()> {
        let email_str = pending.email.to_string().to_lowercase();
        // Note: expires_at is stored as epoch seconds (N) for DynamoDB TTL,
        // while created_at is stored as RFC3339 string for human readability.
        let item = HashMap::from([
            (
                "PK".to_string(),
                AttributeValue::S(PENDING_SUBSCRIPTION_PARTITION_KEY.to_string()),
            ),
            ("SK".to_string(), AttributeValue::S(email_str.clone())),
            (
                "token".to_string(),
                AttributeValue::S(pending.token.to_string()),
            ),
            ("email".to_string(), AttributeValue::S(email_str)),
            (
                "strategy".to_string(),
                AttributeValue::S(pending.strategy.to_string()),
            ),
            (
                "created_at".to_string(),
                AttributeValue::S(pending.created_at.to_rfc3339()),
            ),
            (
                "expires_at".to_string(),
                AttributeValue::N(pending.expires_at.timestamp().to_string()),
            ),
        ]);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to create pending subscription")?;

        Ok(())
    }

    async fn get_pending_subscription(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<PendingSubscription>> {
        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(
                "PK",
                AttributeValue::S(PENDING_SUBSCRIPTION_PARTITION_KEY.to_string()),
            )
            .key("SK", AttributeValue::S(email.to_string().to_lowercase()))
            .send()
            .await
            .context("Failed to get pending subscription")?;

        output.item.map(pending_subscription_from_item).transpose()
    }

    async fn get_subscriber_by_email(&self, email: &EmailAddress) -> Result<Option<Subscriber>> {
        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(
                "PK",
                AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
            )
            .key("SK", AttributeValue::S(email.to_string().to_lowercase()))
            .send()
            .await
            .context("Failed to get subscriber")?;

        output.item.map(subscriber_from_item).transpose()
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn datestamp(date: DateTime<Utc>) -> String {
    date.format("%F").to_string()
}

fn digest_partition_key(type_: &str) -> String {
    format!("{}#{}", DIGEST_PARTITION_KEY_PREFIX, type_)
}

fn to_dynamo_map(posts: &HashMap<String, Post>) -> Result<AttributeValue> {
    let json = serde_json::to_value(posts)?;
    json_to_av(&json)
}

fn to_dynamo_list(posts: &[Post]) -> Result<AttributeValue> {
    let json = serde_json::to_value(posts)?;
    json_to_av(&json)
}

fn from_dynamo_list(av: &AttributeValue) -> Result<Vec<Post>> {
    let json = av_to_json(av)?;
    Ok(serde_json::from_value(json)?)
}

/// Convert a JSON value to a DynamoDB AttributeValue.
fn json_to_av(json: &serde_json::Value) -> Result<AttributeValue> {
    Ok(match json {
        serde_json::Value::Null => AttributeValue::Null(true),
        serde_json::Value::Bool(b) => AttributeValue::Bool(*b),
        serde_json::Value::Number(n) => AttributeValue::N(n.to_string()),
        serde_json::Value::String(s) => AttributeValue::S(s.clone()),
        serde_json::Value::Array(arr) => {
            AttributeValue::L(arr.iter().map(json_to_av).collect::<Result<_>>()?)
        }
        serde_json::Value::Object(map) => AttributeValue::M(
            map.iter()
                .map(|(k, v)| json_to_av(v).map(|av| (k.clone(), av)))
                .collect::<Result<_>>()?,
        ),
    })
}

/// Convert a DynamoDB AttributeValue back to JSON.
fn av_to_json(av: &AttributeValue) -> Result<serde_json::Value> {
    Ok(match av {
        AttributeValue::Null(_) => serde_json::Value::Null,
        AttributeValue::Bool(b) => serde_json::Value::Bool(*b),
        AttributeValue::N(n) => n
            .parse::<i64>()
            .map(Into::into)
            .or_else(|_| n.parse::<f64>().map(Into::into))
            .unwrap_or_else(|_| serde_json::Value::String(n.clone())),
        AttributeValue::S(s) => serde_json::Value::String(s.clone()),
        AttributeValue::L(list) => {
            serde_json::Value::Array(list.iter().map(av_to_json).collect::<Result<_>>()?)
        }
        AttributeValue::M(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| av_to_json(v).map(|json| (k.clone(), json)))
                .collect::<Result<_>>()?,
        ),
        _ => serde_json::Value::Null, // Ignore binary/set types (non-exhaustive enum)
    })
}

/// Convert a DynamoDB item to a Subscriber struct.
pub(crate) fn subscriber_from_item(item: HashMap<String, AttributeValue>) -> Result<Subscriber> {
    let email_str = item
        .get("email")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing email field"))?;
    let email = EmailAddress::from_str(email_str).context("Invalid email address in database")?;

    let strategy_str = item
        .get("strategy")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing strategy field"))?;
    let strategy = DigestStrategy::from_str(strategy_str).context("Invalid strategy value")?;

    let subscribed_at = item
        .get("subscribed_at")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing subscribed_at field"))?
        .parse::<DateTime<Utc>>()
        .context("Invalid subscribed_at timestamp")?;

    let unsubscribe_token = item
        .get("unsubscribe_token")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing unsubscribe_token field"))?
        .parse::<Token>()
        .map_err(|e| anyhow::anyhow!("Invalid unsubscribe_token: {}", e))?;

    Ok(Subscriber {
        email,
        strategy,
        subscribed_at,
        unsubscribe_token,
    })
}

/// Convert a DynamoDB item to a PendingSubscription struct.
pub(crate) fn pending_subscription_from_item(
    item: HashMap<String, AttributeValue>,
) -> Result<PendingSubscription> {
    let token = item
        .get("token")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing token field"))?
        .parse::<Token>()
        .map_err(|e| anyhow::anyhow!("Invalid token: {}", e))?;

    let email_str = item
        .get("email")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing email field"))?;
    let email = EmailAddress::from_str(email_str).context("Invalid email address in database")?;

    let strategy_str = item
        .get("strategy")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing strategy field"))?;
    let strategy = DigestStrategy::from_str(strategy_str).context("Invalid strategy value")?;

    let created_at = item
        .get("created_at")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing created_at field"))?
        .parse::<DateTime<Utc>>()
        .context("Invalid created_at timestamp")?;

    let expires_at_ts = item
        .get("expires_at")
        .and_then(|v| v.as_n().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing expires_at field"))?
        .parse::<i64>()
        .context("Invalid expires_at timestamp")?;
    let expires_at = DateTime::from_timestamp(expires_at_ts, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid expires_at timestamp value"))?;

    Ok(PendingSubscription {
        token,
        email,
        strategy,
        created_at,
        expires_at,
    })
}

// ============================================================================
// Test utilities — shared FakeStorage for in-crate tests
// ============================================================================

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct FakeStorage {
        pub subscribers: Mutex<HashMap<String, Subscriber>>,
        pub pending: Mutex<HashMap<String, PendingSubscription>>,
        pub digests: Mutex<HashMap<(String, String), Vec<Post>>>,
    }

    impl FakeStorage {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn with_subscriber(self, s: Subscriber) -> Self {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s);
            self
        }

        pub(crate) fn with_pending(self, p: PendingSubscription) -> Self {
            self.pending
                .lock()
                .unwrap()
                .insert(p.email.to_string().to_lowercase(), p);
            self
        }

        pub(crate) fn with_digest(
            self,
            type_: &str,
            date: DateTime<Utc>,
            posts: Vec<Post>,
        ) -> Self {
            self.digests
                .lock()
                .unwrap()
                .insert((type_.to_string(), datestamp(date)), posts);
            self
        }
    }

    impl Storage for FakeStorage {
        async fn snapshot_posts(
            &self,
            _posts: &HashMap<String, Post>,
            _date: DateTime<Utc>,
        ) -> Result<()> {
            Ok(())
        }

        async fn save_digest(
            &self,
            type_: &str,
            date: DateTime<Utc>,
            posts: &[Post],
        ) -> Result<()> {
            self.digests
                .lock()
                .unwrap()
                .insert((type_.to_string(), datestamp(date)), posts.to_vec());
            Ok(())
        }

        async fn fetch_digest(
            &self,
            type_: &str,
            date: DateTime<Utc>,
        ) -> Result<Option<Vec<Post>>> {
            Ok(self
                .digests
                .lock()
                .unwrap()
                .get(&(type_.to_string(), datestamp(date)))
                .cloned())
        }

        async fn get_subscriber_by_unsubscribe_token(
            &self,
            token: &Token,
        ) -> Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .values()
                .find(|s| s.unsubscribe_token == *token)
                .cloned())
        }

        async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>> {
            Ok(self.subscribers.lock().unwrap().values().cloned().collect())
        }

        async fn upsert_subscriber(&self, subscriber: &Subscriber) -> Result<()> {
            self.subscribers.lock().unwrap().insert(
                subscriber.email.to_string().to_lowercase(),
                subscriber.clone(),
            );
            Ok(())
        }

        async fn remove_subscriber(&self, email: &EmailAddress) -> Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .remove(&email.to_string().to_lowercase());
            Ok(())
        }

        async fn upsert_pending_subscription(&self, pending: &PendingSubscription) -> Result<()> {
            self.pending
                .lock()
                .unwrap()
                .insert(pending.email.to_string().to_lowercase(), pending.clone());
            Ok(())
        }

        async fn get_pending_subscription(
            &self,
            email: &EmailAddress,
        ) -> Result<Option<PendingSubscription>> {
            Ok(self
                .pending
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }

        async fn get_subscriber_by_email(
            &self,
            email: &EmailAddress,
        ) -> Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }
    }
}

// ============================================================================
// Tests — DynamoDB serialization helpers (no network required)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::DigestStrategy;
    use chrono::TimeZone;

    fn make_subscriber_item(
        email: &str,
        strategy: &str,
        subscribed_at: &str,
        token: &str,
    ) -> HashMap<String, AttributeValue> {
        HashMap::from([
            ("email".to_string(), AttributeValue::S(email.to_string())),
            (
                "strategy".to_string(),
                AttributeValue::S(strategy.to_string()),
            ),
            (
                "subscribed_at".to_string(),
                AttributeValue::S(subscribed_at.to_string()),
            ),
            (
                "unsubscribe_token".to_string(),
                AttributeValue::S(token.to_string()),
            ),
        ])
    }

    #[test]
    fn subscriber_from_item_valid() {
        let item = make_subscriber_item(
            "test@example.com",
            "TOP_N#10",
            "2024-01-01T00:00:00+00:00",
            "some-token",
        );
        let sub = subscriber_from_item(item).unwrap();
        assert_eq!(sub.email.to_string(), "test@example.com");
        assert_eq!(sub.strategy, DigestStrategy::TopN(10));
        assert_eq!(sub.unsubscribe_token.to_string(), "some-token");
    }

    #[test]
    fn subscriber_from_item_missing_email() {
        let item = HashMap::from([
            (
                "strategy".to_string(),
                AttributeValue::S("TOP_N#10".to_string()),
            ),
            (
                "subscribed_at".to_string(),
                AttributeValue::S("2024-01-01T00:00:00+00:00".to_string()),
            ),
            (
                "unsubscribe_token".to_string(),
                AttributeValue::S("token".to_string()),
            ),
        ]);
        assert!(subscriber_from_item(item).is_err());
    }

    #[test]
    fn subscriber_from_item_invalid_strategy() {
        let item = make_subscriber_item(
            "test@example.com",
            "INVALID_STRATEGY",
            "2024-01-01T00:00:00+00:00",
            "token",
        );
        assert!(subscriber_from_item(item).is_err());
    }

    #[test]
    fn pending_subscription_from_item_valid() {
        let expires_ts = Utc
            .with_ymd_and_hms(2024, 1, 2, 0, 0, 0)
            .unwrap()
            .timestamp();
        let item = HashMap::from([
            (
                "token".to_string(),
                AttributeValue::S("abc-token".to_string()),
            ),
            (
                "email".to_string(),
                AttributeValue::S("pending@example.com".to_string()),
            ),
            (
                "strategy".to_string(),
                AttributeValue::S("POINT_THRESHOLD#100".to_string()),
            ),
            (
                "created_at".to_string(),
                AttributeValue::S("2024-01-01T00:00:00+00:00".to_string()),
            ),
            (
                "expires_at".to_string(),
                AttributeValue::N(expires_ts.to_string()),
            ),
        ]);
        let pending = pending_subscription_from_item(item).unwrap();
        assert_eq!(pending.email.to_string(), "pending@example.com");
        assert_eq!(pending.strategy, DigestStrategy::OverPointThreshold(100));
        assert_eq!(pending.token.to_string(), "abc-token");
    }

    #[test]
    fn pending_subscription_from_item_missing_token() {
        let item = HashMap::from([
            (
                "email".to_string(),
                AttributeValue::S("pending@example.com".to_string()),
            ),
            (
                "strategy".to_string(),
                AttributeValue::S("TOP_N#10".to_string()),
            ),
            (
                "created_at".to_string(),
                AttributeValue::S("2024-01-01T00:00:00+00:00".to_string()),
            ),
            (
                "expires_at".to_string(),
                AttributeValue::N("9999999999".to_string()),
            ),
        ]);
        assert!(pending_subscription_from_item(item).is_err());
    }
}
