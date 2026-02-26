use crate::types::{PendingSubscription, Post, Subscriber, Token};
use anyhow::Result;
use chrono::{DateTime, Utc};
use email_address::EmailAddress;
use std::collections::HashMap;

pub mod dynamo;
pub use dynamo::LambdaStorage;

// ============================================================================
// Storage trait
// ============================================================================

#[allow(async_fn_in_trait)]
pub trait Storage: Send + Sync {
    async fn snapshot_posts(
        &self,
        posts: &HashMap<String, Post>,
        date: DateTime<Utc>,
    ) -> Result<()>;

    async fn fetch_digest(&self, type_: &str, date: DateTime<Utc>) -> Result<Option<Vec<Post>>>;
    async fn save_digest(&self, type_: &str, date: DateTime<Utc>, posts: &[Post]) -> Result<()>;

    async fn get_subscriber_by_email(&self, email: &EmailAddress) -> Result<Option<Subscriber>>;
    async fn get_subscriber_by_unsubscribe_token(
        &self,
        token: &Token,
    ) -> Result<Option<Subscriber>>;
    async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>>;
    async fn upsert_subscriber(&self, subscriber: &Subscriber) -> Result<()>;
    async fn remove_subscriber(&self, email: &EmailAddress) -> Result<()>;

    async fn get_pending_subscription(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<PendingSubscription>>;
    async fn upsert_pending_subscription(&self, pending: &PendingSubscription) -> Result<()>;
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(super) fn datestamp(date: DateTime<Utc>) -> String {
    date.format("%F").to_string()
}

// ============================================================================
// Test utilities — FakeStorage for in-crate tests
// ============================================================================

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct InMemoryStorage {
        pub subscribers: Mutex<HashMap<String, Subscriber>>,
        pub pending: Mutex<HashMap<String, PendingSubscription>>,
        pub digests: Mutex<HashMap<(String, String), Vec<Post>>>,
    }

    impl InMemoryStorage {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn with_subscriber(self, s: Subscriber) -> Self {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s);
            self
        }

        pub(crate) fn with_pending(self, p: PendingSubscription) -> Self {
            self.pending
                .lock()
                .unwrap()
                .insert(p.email.to_string().to_lowercase(), p);
            self
        }

        pub(crate) fn with_digest(
            self,
            type_: &str,
            date: DateTime<Utc>,
            posts: Vec<Post>,
        ) -> Self {
            self.digests
                .lock()
                .unwrap()
                .insert((type_.to_string(), datestamp(date)), posts);
            self
        }

        pub(crate) fn get_subscriber(&self, email: &str) -> Option<Subscriber> {
            self.subscribers
                .lock()
                .unwrap()
                .get(&email.to_lowercase())
                .cloned()
        }

        pub(crate) fn get_pending(&self, email: &str) -> Option<PendingSubscription> {
            self.pending
                .lock()
                .unwrap()
                .get(&email.to_lowercase())
                .cloned()
        }

        pub(crate) fn has_subscriber(&self, email: &str) -> bool {
            self.subscribers
                .lock()
                .unwrap()
                .contains_key(&email.to_lowercase())
        }

        pub(crate) fn subscriber_count(&self) -> usize {
            self.subscribers.lock().unwrap().len()
        }
    }

    impl Storage for InMemoryStorage {
        async fn snapshot_posts(&self, _: &HashMap<String, Post>, _: DateTime<Utc>) -> Result<()> {
            Ok(())
        }

        async fn save_digest(
            &self,
            type_: &str,
            date: DateTime<Utc>,
            posts: &[Post],
        ) -> Result<()> {
            self.digests
                .lock()
                .unwrap()
                .insert((type_.to_string(), datestamp(date)), posts.to_vec());
            Ok(())
        }

        async fn fetch_digest(
            &self,
            type_: &str,
            date: DateTime<Utc>,
        ) -> Result<Option<Vec<Post>>> {
            Ok(self
                .digests
                .lock()
                .unwrap()
                .get(&(type_.to_string(), datestamp(date)))
                .cloned())
        }

        async fn get_subscriber_by_unsubscribe_token(
            &self,
            token: &Token,
        ) -> Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .values()
                .find(|s| s.unsubscribe_token == *token)
                .cloned())
        }

        async fn get_all_subscribers(&self) -> Result<Vec<Subscriber>> {
            Ok(self.subscribers.lock().unwrap().values().cloned().collect())
        }

        async fn upsert_subscriber(&self, s: &Subscriber) -> Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s.clone());
            Ok(())
        }

        async fn remove_subscriber(&self, email: &EmailAddress) -> Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .remove(&email.to_string().to_lowercase());
            Ok(())
        }

        async fn upsert_pending_subscription(&self, p: &PendingSubscription) -> Result<()> {
            self.pending
                .lock()
                .unwrap()
                .insert(p.email.to_string().to_lowercase(), p.clone());
            Ok(())
        }

        async fn get_pending_subscription(
            &self,
            email: &EmailAddress,
        ) -> Result<Option<PendingSubscription>> {
            Ok(self
                .pending
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }

        async fn get_subscriber_by_email(
            &self,
            email: &EmailAddress,
        ) -> Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }
    }
}
