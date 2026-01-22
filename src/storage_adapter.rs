use crate::types::Post;
use anyhow::{Context, Result};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

const SNAPSHOT_PARTITION_KEY: &str = "POSTS_SNAPSHOT";
const DIGEST_PARTITION_KEY_PREFIX: &str = "DIGEST";
const SUBSCRIBERS_PARTITION_KEY: &str = "SUBSCRIBERS";
const MODEL_TTL_DAYS: i64 = 30;

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

    pub async fn fetch_subscribers(&self, type_: &str) -> Result<Option<Vec<String>>> {
        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(
                "PK",
                AttributeValue::S(SUBSCRIBERS_PARTITION_KEY.to_string()),
            )
            .key("SK", AttributeValue::S(type_.to_string()))
            .send()
            .await
            .context("Failed to fetch subscribers")?;

        output
            .item
            .and_then(|item| item.get("emails").cloned())
            .map(|emails_av| as_string_list(&emails_av))
            .transpose()
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

fn as_string_list(av: &AttributeValue) -> Result<Vec<String>> {
    match av {
        AttributeValue::L(list) => Ok(list
            .iter()
            .filter_map(|item| match item {
                AttributeValue::S(s) => Some(s.clone()),
                _ => None,
            })
            .collect()),
        AttributeValue::Ss(set) => Ok(set.clone()),
        _ => anyhow::bail!("Expected list or string set attribute"),
    }
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
