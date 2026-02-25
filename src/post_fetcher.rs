use crate::types::Post;
use anyhow::Result;
use reqwest::Client;
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
        let url = format!(
            "{}{}?hitsPerPage={}&tags=story&numericFilters=created_at_i>={}",
            HOST, PATH, top_k, since
        );
        self.fetch_posts_from_url(&url).await
    }

    async fn fetch_by_points(&self, points: i32, since: i64) -> Result<HashMap<String, Post>> {
        let url = format!(
            "{}{}?hitsPerPage=10000&tags=story&numericFilters=created_at_i>={},points>={}",
            HOST, PATH, since, points
        );
        self.fetch_posts_from_url(&url).await
    }

    async fn fetch_posts_from_url(&self, url: &str) -> Result<HashMap<String, Post>> {
        let resp: AlgoliaResponse = self.client.get(url).send().await?.json().await?;

        Ok(resp
            .hits
            .into_iter()
            .map(|post| (post.object_id.clone(), post))
            .collect())
    }
}
