use crate::storage_adapter::StorageAdapter;
use crate::strategies::DigestStrategy;
use crate::types::Post;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;

use std::sync::Arc;

pub struct DigestBuilder {
    storage: Arc<StorageAdapter>,
}

impl DigestBuilder {
    pub fn new(storage: Arc<StorageAdapter>) -> Self {
        Self { storage }
    }

    pub async fn build_digest(
        &self,
        strategy: &dyn DigestStrategy,
        date: DateTime<Utc>,
        posts: &[Post],
    ) -> Result<Vec<Post>> {
        let strategy_type = strategy.type_();
        let yesterday = date - Duration::days(1);
        let yesterday_digest = self.storage.fetch_digest(&strategy_type, yesterday).await?;

        let mut unsent_posts = self.remove_sent_posts(posts, yesterday_digest.as_deref());

        unsent_posts.sort_by(|a, b| b.points.cmp(&a.points)); // Descending points

        let selected_posts = strategy.select(&unsent_posts);

        self.storage
            .save_digest(&strategy.type_(), date, &selected_posts)
            .await?;

        Ok(selected_posts)
    }

    fn remove_sent_posts(
        &self,
        all_posts: &[Post],
        yesterday_digest: Option<&[Post]>,
    ) -> Vec<Post> {
        if let Some(digest_posts) = yesterday_digest {
            let sent_ids: HashSet<&str> =
                digest_posts.iter().map(|p| p.object_id.as_str()).collect();
            all_posts
                .iter()
                .filter(|p| !sent_ids.contains(p.object_id.as_str()))
                .cloned()
                .collect()
        } else {
            all_posts.to_vec()
        }
    }
}
