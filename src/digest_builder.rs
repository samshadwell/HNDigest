use crate::storage::Storage;
use crate::strategies::DigestStrategy;
use crate::types::Post;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;
use std::sync::Arc;

pub struct DigestBuilder<S> {
    storage: Arc<S>,
}

impl<S: Storage> DigestBuilder<S> {
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    pub async fn build_digest(
        &self,
        strategy: DigestStrategy,
        date: DateTime<Utc>,
        posts: &[Post],
    ) -> Result<Vec<Post>> {
        let strategy_name = strategy.to_string();
        let yesterday = date - Duration::days(1);
        let yesterday_digest = self.storage.fetch_digest(&strategy_name, yesterday).await?;

        let mut unsent_posts = filter_sent_posts(posts, yesterday_digest.as_deref());
        unsent_posts.sort_by(|a, b| b.points.cmp(&a.points));

        let selected_posts = strategy.select(&unsent_posts);

        self.storage
            .save_digest(&strategy_name, date, &selected_posts)
            .await?;

        Ok(selected_posts)
    }
}

pub(crate) fn filter_sent_posts(
    all_posts: &[Post],
    yesterday_digest: Option<&[Post]>,
) -> Vec<Post> {
    let Some(digest_posts) = yesterday_digest else {
        return all_posts.to_vec();
    };

    let sent_ids: HashSet<&str> = digest_posts.iter().map(|p| p.object_id.as_str()).collect();

    all_posts
        .iter()
        .filter(|p| !sent_ids.contains(p.object_id.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_utils::FakeStorage;
    use crate::strategies::DigestStrategy;
    use crate::types::Post;
    use chrono::Utc;

    fn make_post(id: &str, points: i32) -> Post {
        Post {
            object_id: id.to_string(),
            title: format!("Post {}", id),
            url: None,
            points,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // filter_sent_posts — pure function, no storage
    // -----------------------------------------------------------------------

    #[test]
    fn filter_sent_posts_no_yesterday_digest_returns_all() {
        let posts = vec![make_post("a", 100), make_post("b", 200)];
        let result = filter_sent_posts(&posts, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_sent_posts_removes_already_sent() {
        let posts = vec![
            make_post("a", 500),
            make_post("b", 200),
            make_post("c", 100),
        ];
        let yesterday = vec![make_post("a", 500)];
        let result = filter_sent_posts(&posts, Some(&yesterday));
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.object_id != "a"));
    }

    #[test]
    fn filter_sent_posts_empty_yesterday_digest_returns_all() {
        let posts = vec![make_post("a", 100), make_post("b", 200)];
        let result = filter_sent_posts(&posts, Some(&[]));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_sent_posts_all_sent_returns_empty() {
        let posts = vec![make_post("a", 100)];
        let yesterday = vec![make_post("a", 100)];
        let result = filter_sent_posts(&posts, Some(&yesterday));
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // build_digest — uses FakeStorage
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn build_digest_top_n_excludes_yesterday_and_sorts_by_points() {
        let date = Utc::now();
        let yesterday = date - Duration::days(1);
        let strategy = DigestStrategy::TopN(2);

        // Post "a" was already sent yesterday
        let storage = Arc::new(FakeStorage::new().with_digest(
            &strategy.to_string(),
            yesterday,
            vec![make_post("a", 500)],
        ));
        let builder = DigestBuilder::new(Arc::clone(&storage));

        let posts = vec![
            make_post("a", 500),
            make_post("b", 200),
            make_post("c", 100),
        ];
        let result = builder.build_digest(strategy, date, &posts).await.unwrap();

        // "a" filtered, top 2 of remaining are "b" then "c"
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].object_id, "b");
        assert_eq!(result[1].object_id, "c");

        // Digest should be saved
        let saved = storage
            .fetch_digest(&strategy.to_string(), date)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.len(), 2);
    }

    #[tokio::test]
    async fn build_digest_over_point_threshold_filters_correctly() {
        let date = Utc::now();
        let strategy = DigestStrategy::OverPointThreshold(200);
        let storage = Arc::new(FakeStorage::new());
        let builder = DigestBuilder::new(Arc::clone(&storage));

        let posts = vec![
            make_post("a", 500),
            make_post("b", 200),
            make_post("c", 100),
        ];
        let result = builder.build_digest(strategy, date, &posts).await.unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.points >= 200));
        assert!(result.iter().all(|p| p.object_id != "c"));
    }

    #[tokio::test]
    async fn build_digest_no_yesterday_digest_uses_all_posts() {
        let date = Utc::now();
        let strategy = DigestStrategy::TopN(10);
        let storage = Arc::new(FakeStorage::new()); // no pre-seeded digest
        let builder = DigestBuilder::new(Arc::clone(&storage));

        let posts = vec![make_post("a", 500), make_post("b", 200)];
        let result = builder.build_digest(strategy, date, &posts).await.unwrap();

        assert_eq!(result.len(), 2);
    }
}
