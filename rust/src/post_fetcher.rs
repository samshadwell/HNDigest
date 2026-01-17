use crate::types::Post;
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;
use serde::Deserialize;

const HOST: &str = "https://hn.algolia.com";
const PATH: &str = "/api/v1/search";

#[derive(Deserialize)]
struct AlgoliaResponse {
    hits: Vec<Post>,
}

pub struct PostFetcher {
    client: Client,
}

impl PostFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn fetch(&self, top_k: usize, points: i32, since: i64) -> Result<HashMap<String, Post>> {
        let (top_k_posts, by_points_posts) = tokio::try_join!(
            self.fetch_top_k(top_k, since),
            self.fetch_by_points(points, since)
        )?;

        let mut combined = top_k_posts;
        combined.extend(by_points_posts);
        
        Ok(combined)
    }

    async fn fetch_top_k(&self, top_k: usize, since: i64) -> Result<HashMap<String, Post>> {
        let url = format!(
            "{}{}?hitsPerPage={}&tags=story&numericFilters=created_at_i>={}",
            HOST, PATH, top_k, since
        );
        self.fetch_posts_from_path(&url).await
    }

    async fn fetch_by_points(&self, points: i32, since: i64) -> Result<HashMap<String, Post>> {
        let url = format!(
            "{}{}?hitsPerPage=10000&tags=story&numericFilters=created_at_i>={},points>={}",
            HOST, PATH, since, points
        );
        self.fetch_posts_from_path(&url).await
    }

    async fn fetch_posts_from_path(&self, url: &str) -> Result<HashMap<String, Post>> {
        let resp = self.client.get(url).send().await?.json::<AlgoliaResponse>().await?;
        
        let mut posts = HashMap::new();
        for post in resp.hits {
            posts.insert(post.object_id.clone(), post);
        }
        
        Ok(posts)
    }
}
