//! Business logic for subscription management.
//!
//! Handles creating pending subscriptions and verifying them.

use crate::storage::Storage;
use crate::strategies::DigestStrategy;
use crate::types::{PendingSubscription, Subscriber, Token};
use anyhow::Result;
use email_address::EmailAddress;
use std::sync::Arc;
use tracing::info;

/// Create a pending subscription for an email address.
///
/// If the email is already subscribed, will still create
/// pending record and return it.
pub async fn create_pending_subscription<S: Storage>(
    storage: &Arc<S>,
    email: &EmailAddress,
    strategy: DigestStrategy,
) -> Result<PendingSubscription> {
    let pending = PendingSubscription::new(email.clone(), strategy);
    storage.upsert_pending_subscription(&pending).await?;

    Ok(pending)
}

/// Update an existing subscriber's digest strategy in storage.
///
/// Returns the previous strategy so the caller can describe the change (e.g. in a
/// notification email). The subscriber's `subscribed_at` and `unsubscribe_token` are
/// preserved unchanged.
pub async fn update_subscription_strategy<S: Storage>(
    storage: &Arc<S>,
    existing: Subscriber,
    new_strategy: DigestStrategy,
) -> Result<DigestStrategy> {
    let old_strategy = existing.strategy;
    let updated = Subscriber {
        strategy: new_strategy,
        ..existing
    };
    storage.upsert_subscriber(&updated).await?;
    info!(
        email = %updated.email,
        old_strategy = %old_strategy,
        new_strategy = %new_strategy,
        "Subscriber strategy updated"
    );
    Ok(old_strategy)
}

/// Verify a pending subscription by email and token.
///
/// Returns `Ok(Some(subscriber))` if the subscription was verified successfully,
/// `Ok(None)` if no pending subscription exists or the token doesn't match.
/// Returns an error if a database error occurs.
///
/// This is idempotent: if the token is valid and already verified, returns success.
pub async fn verify_subscription<S: Storage>(
    storage: &Arc<S>,
    email: &EmailAddress,
    token: &Token,
) -> Result<Option<Subscriber>> {
    let pending = match storage.get_pending_subscription(email).await? {
        Some(p) => p,
        None => return Ok(None),
    };

    if &pending.token != token {
        return Ok(None);
    }

    let subscriber = Subscriber::new(pending.email.clone(), pending.strategy);
    storage.upsert_subscriber(&subscriber).await?;

    Ok(Some(subscriber))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_utils::InMemoryStorage;
    use crate::strategies::DigestStrategy;
    use crate::types::{PendingSubscription, Subscriber, Token};
    use email_address::EmailAddress;
    use std::str::FromStr;

    fn email(s: &str) -> EmailAddress {
        EmailAddress::from_str(s).unwrap()
    }

    #[tokio::test]
    async fn create_pending_stores_record() {
        let storage = Arc::new(InMemoryStorage::new());
        let strategy = DigestStrategy::TopN(10);
        let email = email("new@example.com");

        let pending = create_pending_subscription(&storage, &email, strategy)
            .await
            .unwrap();

        assert_eq!(pending.email, email);
        assert_eq!(pending.strategy, strategy);

        // Verify the record was actually stored
        let stored = storage
            .get_pending_subscription(&email)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.token, pending.token);
    }

    #[tokio::test]
    async fn verify_subscription_valid_token_creates_subscriber() {
        let pending =
            PendingSubscription::new(email("verify@example.com"), DigestStrategy::TopN(20));
        let token = pending.token.clone();
        let storage = Arc::new(InMemoryStorage::new().with_pending(pending));

        let result = verify_subscription(&storage, &email("verify@example.com"), &token)
            .await
            .unwrap();

        assert!(result.is_some());
        let sub = result.unwrap();
        assert_eq!(sub.email, email("verify@example.com"));
        assert_eq!(sub.strategy, DigestStrategy::TopN(20));

        // Subscriber should now be in storage
        let stored = storage
            .get_subscriber_by_email(&email("verify@example.com"))
            .await
            .unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn verify_subscription_wrong_token_returns_none() {
        let pending =
            PendingSubscription::new(email("verify@example.com"), DigestStrategy::TopN(10));
        let storage = Arc::new(InMemoryStorage::new().with_pending(pending));
        let wrong_token: Token = "wrong-token".parse().unwrap();

        let result = verify_subscription(&storage, &email("verify@example.com"), &wrong_token)
            .await
            .unwrap();

        assert!(result.is_none());
        // No subscriber should have been created
        let stored = storage
            .get_subscriber_by_email(&email("verify@example.com"))
            .await
            .unwrap();
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn verify_subscription_no_pending_returns_none() {
        let storage = Arc::new(InMemoryStorage::new());
        let token: Token = "some-token".parse().unwrap();

        let result = verify_subscription(&storage, &email("nobody@example.com"), &token)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_strategy_persists_new_and_returns_old() {
        let sub = Subscriber::new(email("sub@example.com"), DigestStrategy::TopN(10));
        let storage = Arc::new(InMemoryStorage::new().with_subscriber(sub));

        let old = update_subscription_strategy(
            &storage,
            storage
                .get_subscriber_by_email(&email("sub@example.com"))
                .await
                .unwrap()
                .unwrap(),
            DigestStrategy::TopN(50),
        )
        .await
        .unwrap();

        assert_eq!(old, DigestStrategy::TopN(10));

        let updated = storage
            .get_subscriber_by_email(&email("sub@example.com"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.strategy, DigestStrategy::TopN(50));
    }
}
