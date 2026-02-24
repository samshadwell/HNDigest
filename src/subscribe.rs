//! Business logic for subscription management.
//!
//! Handles creating pending subscriptions and verifying them.

use crate::storage_adapter::StorageAdapter;
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
pub async fn create_pending_subscription(
    storage: &Arc<StorageAdapter>,
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
pub async fn update_subscription_strategy(
    storage: &Arc<StorageAdapter>,
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
pub async fn verify_subscription(
    storage: &Arc<StorageAdapter>,
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
