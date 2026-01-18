use crate::configuration::{POINT_THRESHOLD_VALUES, TOP_N_VALUES};
use crate::post_fetcher::PostFetcher;
use crate::storage_adapter::StorageAdapter;
use crate::types::Post;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

const LOOKBACK_SECONDS: i64 = 2 * 24 * 60 * 60; // 2 days

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
        let max_top_n = TOP_N_VALUES.iter().max().copied().unwrap_or(50);
        let min_points = POINT_THRESHOLD_VALUES.iter().min().copied().unwrap_or(100);

        let since = date.timestamp() - LOOKBACK_SECONDS;

        // 2x top n in case all the top n were sent yesterday.
        let posts = self.fetcher.fetch(2 * max_top_n, min_points, since).await?;

        self.storage.snapshot_posts(&posts, date).await?;

        Ok(posts)
    }
}
