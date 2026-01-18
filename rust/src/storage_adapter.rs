use aws_sdk_dynamodb::{
    types::AttributeValue,
    Client,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use anyhow::{Result, Context};
use crate::types::Post;

const TABLE: &str = "HNDigest";
const SNAPSHOT_PARTITION_KEY: &str = "POSTS_SNAPSHOT";
const DIGEST_PARTITION_KEY_PREFIX: &str = "DIGEST";
const SUBSCRIBERS_PARTITION_KEY: &str = "SUBSCRIBERS";
const MODEL_TTL: i64 = 30 * 24 * 60 * 60; // 30 days in seconds

pub struct StorageAdapter {
    client: Client,
}

impl StorageAdapter {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn snapshot_posts(&self, posts: &HashMap<String, Post>, date: DateTime<Utc>) -> Result<()> {
        let datestamp = datestamp(date);
        let posts_av = to_dynamo_map(posts)?;

        let item = HashMap::from([
            ("PK".to_string(), AttributeValue::S(SNAPSHOT_PARTITION_KEY.to_string())),
            ("SK".to_string(), AttributeValue::S(datestamp)),
            ("posts".to_string(), posts_av),
            ("expires_at".to_string(), AttributeValue::N((date.timestamp() + MODEL_TTL).to_string())),
        ]);

        self.client.put_item()
            .table_name(TABLE)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to snapshot posts")?;

        Ok(())
    }

    pub async fn fetch_post_snapshot(&self, date: DateTime<Utc>) -> Result<Option<HashMap<String, Post>>> {
        let datestamp = datestamp(date);
        
        let output = self.client.get_item()
            .table_name(TABLE)
            .key("PK", AttributeValue::S(SNAPSHOT_PARTITION_KEY.to_string()))
            .key("SK", AttributeValue::S(datestamp))
            .send()
            .await
            .context("Failed to fetch post snapshot")?;

        if let Some(item) = output.item {
            if let Some(posts_av) = item.get("posts") {
                let posts: HashMap<String, Post> = from_dynamo_map(posts_av)?;
                return Ok(Some(posts));
            }
        }

        Ok(None)
    }

    pub async fn save_digest(&self, type_: &str, date: DateTime<Utc>, posts: &[Post]) -> Result<()> {
        let datestamp = datestamp(date);
        
        let posts_av = to_dynamo_list(posts)?;

        let item = HashMap::from([
            ("PK".to_string(), AttributeValue::S(digest_partition_key(type_))),
            ("SK".to_string(), AttributeValue::S(datestamp)),
            ("posts".to_string(), posts_av),
            ("expires_at".to_string(), AttributeValue::N((date.timestamp() + MODEL_TTL).to_string())),
        ]);

        self.client.put_item()
            .table_name(TABLE)
            .set_item(Some(item))
            .send()
            .await
            .context("Failed to save digest")?;

        Ok(())
    }

    pub async fn fetch_digest(&self, type_: &str, date: DateTime<Utc>) -> Result<Option<Vec<Post>>> {
        let datestamp = datestamp(date);

        let output = self.client.get_item()
            .table_name(TABLE)
            .key("PK", AttributeValue::S(digest_partition_key(type_)))
            .key("SK", AttributeValue::S(datestamp))
            .send()
            .await
            .context("Failed to fetch digest")?;

        if let Some(item) = output.item {
            if let Some(posts_av) = item.get("posts") {
                let posts: Vec<Post> = from_dynamo_list(posts_av)?;
                return Ok(Some(posts));
            }
        }

        Ok(None)
    }

    pub async fn fetch_subscribers(&self, type_: &str) -> Result<Option<Vec<String>>> {
        let output = self.client.get_item()
            .table_name(TABLE)
            .key("PK", AttributeValue::S(SUBSCRIBERS_PARTITION_KEY.to_string()))
            .key("SK", AttributeValue::S(type_.to_string()))
            .send()
            .await
            .context("Failed to fetch subscribers")?;

        if let Some(item) = output.item {
            if let Some(emails_av) = item.get("emails") {
                 if let Ok(emails) = as_string_list(emails_av) {
                     return Ok(Some(emails));
                 }
                 // Try SS (String Set) if it was stored as set, or L (List).
            }
        }

        Ok(None)
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

fn from_dynamo_map(av: &AttributeValue) -> Result<HashMap<String, Post>> {
    let json = av_to_json(av)?;
    Ok(serde_json::from_value(json)?)
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
        AttributeValue::L(list) => {
            let mut res = Vec::new();
            for item in list {
                if let AttributeValue::S(s) = item {
                    res.push(s.clone());
                }
            }
            Ok(res)
        },
        AttributeValue::Ss(set) => Ok(set.clone()),
        _ => Ok(vec![]),
    }
}

// Simplified Serde <-> DynamoDB AV conversion
// For recursion, we need to handle Object (Map), Array (List), String, Number, etc.
fn json_to_av(json: &serde_json::Value) -> Result<AttributeValue> {
    match json {
        serde_json::Value::Null => Ok(AttributeValue::Null(true)),
        serde_json::Value::Bool(b) => Ok(AttributeValue::Bool(*b)),
        serde_json::Value::Number(n) => Ok(AttributeValue::N(n.to_string())),
        serde_json::Value::String(s) => Ok(AttributeValue::S(s.clone())),
        serde_json::Value::Array(arr) => {
            let mut list = Vec::new();
            for item in arr {
                list.push(json_to_av(item)?);
            }
            Ok(AttributeValue::L(list))
        },
        serde_json::Value::Object(map) => {
            let mut m = HashMap::new();
            for (k, v) in map {
                m.insert(k.clone(), json_to_av(v)?);
            }
            Ok(AttributeValue::M(m))
        },
    }
}

fn av_to_json(av: &AttributeValue) -> Result<serde_json::Value> {
    match av {
        AttributeValue::Null(_) => Ok(serde_json::Value::Null),
        AttributeValue::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        AttributeValue::N(n) => {
            if let Ok(i) = n.parse::<i64>() {
                 Ok(serde_json::json!(i))
            } else if let Ok(f) = n.parse::<f64>() {
                 Ok(serde_json::json!(f))
            } else {
                 Ok(serde_json::Value::String(n.clone())) // Fallback
            }
        },
        AttributeValue::S(s) => Ok(serde_json::Value::String(s.clone())),
        AttributeValue::L(list) => {
            let mut arr = Vec::new();
            for item in list {
                arr.push(av_to_json(item)?);
            }
            Ok(serde_json::Value::Array(arr))
        },
        AttributeValue::M(map) => {
            let mut obj = serde_json::Map::<String, serde_json::Value>::new();
            for (k, v) in map {
                obj.insert(k.clone(), av_to_json(v)?);
            }
            Ok(serde_json::Value::Object(obj))
        },
        _ => Ok(serde_json::Value::Null), // Ignore binary/set types for now if not expected
    }
}
