use crate::types::Post;
use anyhow::Result;
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use std::collections::HashMap;

const HOST: &str = "https://hn.algolia.com";
const PATH: &str = "/api/v1/search";

// ============================================================================
// PostFetcher trait
// ============================================================================

#[allow(async_fn_in_trait)]
pub trait PostFetcher: Send + Sync {
    /// Fetch HN posts, returning a map keyed by object_id for deduplication.
    ///
    /// - `top_k`: fetch the top-K stories by score
    /// - `points`: also fetch all stories above this point threshold
    /// - `since`: Unix timestamp; only stories created after this time
    async fn fetch(&self, top_k: usize, points: i32, since: i64) -> Result<HashMap<String, Post>>;
}

// ============================================================================
// AlgoliaPostFetcher — HN Algolia API implementation
// ============================================================================

#[derive(Deserialize)]
struct AlgoliaResponse {
    hits: Vec<Post>,
}

pub struct AlgoliaPostFetcher {
    client: Client,
}

impl Default for AlgoliaPostFetcher {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl AlgoliaPostFetcher {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PostFetcher for AlgoliaPostFetcher {
    async fn fetch(&self, top_k: usize, points: i32, since: i64) -> Result<HashMap<String, Post>> {
        let (top_k_posts, by_points_posts) = tokio::try_join!(
            self.fetch_top_k(top_k, since),
            self.fetch_by_points(points, since)
        )?;

        let mut combined = top_k_posts;
        combined.extend(by_points_posts);

        Ok(combined)
    }
}

impl AlgoliaPostFetcher {
    async fn fetch_top_k(&self, top_k: usize, since: i64) -> Result<HashMap<String, Post>> {
        let req = self.client.get(format!("{}{}", HOST, PATH)).query(&[
            ("hitsPerPage", top_k.to_string()),
            ("tags", "story".to_string()),
            ("filters", format!("created_at_i >= {}", since)),
        ]);

        self.fetch_posts(req).await
    }

    async fn fetch_by_points(&self, points: i32, since: i64) -> Result<HashMap<String, Post>> {
        // Ordering is by points. Around June 2026 this API stopped filtering by points server-side,
        // so we need to do it client-side. Making a simplifying assumption that there won't be
        // more than 1 page of results (1000 posts) with at least 'points' points
        let req = self.client.get(format!("{}{}", HOST, PATH)).query(&[
            ("hitsPerPage", "1000".to_string()),
            ("tags", "story".to_string()),
            ("filters", format!("created_at_i >= {}", since)),
        ]);
        let filtered = self
            .fetch_posts(req)
            .await?
            .into_iter()
            .filter(|(_, p)| p.points >= points)
            .collect();
        Ok(filtered)
    }

    async fn fetch_posts(&self, req: RequestBuilder) -> Result<HashMap<String, Post>> {
        let resp: AlgoliaResponse = req.send().await?.json().await?;
        Ok(resp
            .hits
            .into_iter()
            .map(|post| (post.object_id.clone(), post))
            .collect())
    }
}
