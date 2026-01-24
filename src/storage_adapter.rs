use crate::strategies::DigestStrategy;
use crate::types::{Post, Subscriber};
use anyhow::{Context, Result};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::str::FromStr;

const SNAPSHOT_PARTITION_KEY: &str = "POSTS_SNAPSHOT";
const DIGEST_PARTITION_KEY_PREFIX: &str = "DIGEST";
const SUBSCRIBER_PARTITION_KEY: &str = "SUBSCRIBER";
const MODEL_TTL_DAYS: i64 = 30;
const UNSUBSCRIBE_TOKEN_INDEX: &str = "unsubscribe_token_index";

pub struct StorageAdapter {
    client: Client,
    table_name: String,
}

impl StorageAdapter {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub async fn snapshot_posts(
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

    pub async fn save_digest(
        &self,
        type_: &str,
        date: DateTime<Utc>,
        posts: &[Post],
    ) -> Result<()> {
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

    pub async fn fetch_digest(
        &self,
        type_: &str,
        date: DateTime<Utc>,
    ) -> Result<Option<Vec<Post>>> {
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

    /// Get a subscriber by their unsubscribe token.
    /// Returns None if no subscriber exists with this token.
    /// Fails if multiple subscribers have the same token (should never happen).
    pub async fn get_subscriber_by_token(&self, token: &str) -> Result<Option<Subscriber>> {
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

    /// Get all subscribers (scans the table for SUBSCRIBER records).
    pub async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>> {
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

    /// Create or update a subscriber record.
    pub async fn set_subscriber(&self, subscriber: &Subscriber) -> Result<()> {
        let mut item = HashMap::from([
            (
                "PK".to_string(),
                AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
            ),
            (
                "SK".to_string(),
                AttributeValue::S(subscriber.email.to_lowercase()),
            ),
            (
                "email".to_string(),
                AttributeValue::S(subscriber.email.to_lowercase()),
            ),
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

        if let Some(verified_at) = subscriber.verified_at {
            item.insert(
                "verified_at".to_string(),
                AttributeValue::S(verified_at.to_rfc3339()),
            );
        }

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to set subscriber")?;

        Ok(())
    }

    /// Remove a subscriber by email address.
    pub async fn remove_subscriber(&self, email: &str) -> Result<()> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key(
                "PK",
                AttributeValue::S(SUBSCRIBER_PARTITION_KEY.to_string()),
            )
            .key("SK", AttributeValue::S(email.to_lowercase()))
            .send()
            .await
            .context("Failed to remove subscriber")?;

        Ok(())
    }
}

fn datestamp(date: DateTime<Utc>) -> String {
    date.format("%F").to_string()
}

fn digest_partition_key(type_: &str) -> String {
    format!("{}#{}", DIGEST_PARTITION_KEY_PREFIX, type_)
}

// Helpers for DynamoDB serde
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
        _ => serde_json::Value::Null, // Ignore binary/set types
    })
}

/// Convert a DynamoDB item to a Subscriber struct.
fn subscriber_from_item(item: HashMap<String, AttributeValue>) -> Result<Subscriber> {
    let email = item
        .get("email")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing email field"))?
        .clone();

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

    let verified_at = item
        .get("verified_at")
        .and_then(|v| v.as_s().ok())
        .map(|s| s.parse::<DateTime<Utc>>())
        .transpose()
        .context("Invalid verified_at timestamp")?;

    let unsubscribe_token = item
        .get("unsubscribe_token")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing unsubscribe_token field"))?
        .clone();

    Ok(Subscriber {
        email,
        strategy,
        subscribed_at,
        verified_at,
        unsubscribe_token,
    })
}
