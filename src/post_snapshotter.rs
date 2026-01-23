use crate::post_fetcher::PostFetcher;
use crate::storage_adapter::StorageAdapter;
use crate::strategies::DigestStrategy;
use crate::types::Post;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

const LOOKBACK_DAYS: i64 = 2;

pub struct PostSnapshotter<'a> {
    storage: &'a StorageAdapter,
    fetcher: PostFetcher,
}

impl<'a> PostSnapshotter<'a> {
    pub fn new(storage: &'a StorageAdapter) -> Self {
        Self {
            storage,
            fetcher: PostFetcher::new(),
        }
    }

    pub async fn snapshot(&self, date: DateTime<Utc>) -> Result<HashMap<String, Post>> {
        let max_top_n = DigestStrategy::max_top_n();
        let min_points = DigestStrategy::min_point_threshold();

        let since = (date - Duration::days(LOOKBACK_DAYS)).timestamp();

        // 2x top n in case all the top n were sent yesterday
        let posts = self.fetcher.fetch(2 * max_top_n, min_points, since).await?;

        self.storage.snapshot_posts(&posts, date).await?;

        Ok(posts)
    }
}
