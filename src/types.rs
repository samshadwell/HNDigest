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
