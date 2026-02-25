use crate::storage::Storage;
use crate::types::Token;
use anyhow::Result;
use std::sync::Arc;

/// Remove a subscriber by their unsubscribe token.
///
/// Returns `Ok(true)` if the subscriber was found and removed,
/// `Ok(false)` if no subscriber was found with that token.
pub async fn remove_subscriber<S: Storage>(storage: &Arc<S>, token: &Token) -> Result<bool> {
    let subscriber = match storage.get_subscriber_by_unsubscribe_token(token).await? {
        Some(s) => s,
        None => return Ok(false),
    };

    storage.remove_subscriber(&subscriber.email).await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_utils::FakeStorage;
    use crate::strategies::DigestStrategy;
    use crate::types::Subscriber;
    use email_address::EmailAddress;
    use std::str::FromStr;

    fn email(s: &str) -> EmailAddress {
        EmailAddress::from_str(s).unwrap()
    }

    #[tokio::test]
    async fn remove_subscriber_found_removes_and_returns_true() {
        let sub = Subscriber::new(email("unsub@example.com"), DigestStrategy::TopN(10));
        let token = sub.unsubscribe_token.clone();
        let storage = Arc::new(FakeStorage::new().with_subscriber(sub));

        let result = remove_subscriber(&storage, &token).await.unwrap();

        assert!(result);
        assert!(
            storage
                .get_subscriber_by_email(&email("unsub@example.com"))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn remove_subscriber_not_found_returns_false() {
        let storage = Arc::new(FakeStorage::new());
        let token: Token = "unknown-token".parse().unwrap();

        let result = remove_subscriber(&storage, &token).await.unwrap();

        assert!(!result);
    }

    #[tokio::test]
    async fn remove_subscriber_does_not_affect_other_subscribers() {
        let sub1 = Subscriber::new(email("keep@example.com"), DigestStrategy::TopN(10));
        let sub2 = Subscriber::new(email("remove@example.com"), DigestStrategy::TopN(10));
        let token = sub2.unsubscribe_token.clone();
        let storage = Arc::new(
            FakeStorage::new()
                .with_subscriber(sub1)
                .with_subscriber(sub2),
        );

        remove_subscriber(&storage, &token).await.unwrap();

        assert!(
            storage
                .get_subscriber_by_email(&email("keep@example.com"))
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            storage
                .get_subscriber_by_email(&email("remove@example.com"))
                .await
                .unwrap()
                .is_none()
        );
    }
}
