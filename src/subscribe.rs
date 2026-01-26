//! Business logic for subscription management.
//!
//! Handles creating pending subscriptions and verifying them.

use crate::storage_adapter::StorageAdapter;
use crate::strategies::DigestStrategy;
use crate::types::{PendingSubscription, Subscriber, Token};
use anyhow::Result;
use email_address::EmailAddress;
use std::sync::Arc;

/// Result of attempting to create a subscription.
#[derive(Debug)]
pub enum SubscribeResult {
    /// Verification email should be sent.
    PendingCreated(PendingSubscription),
    /// Email is already subscribed and verified.
    AlreadySubscribed,
}

/// Create a pending subscription for an email address.
///
/// Returns `SubscribeResult::PendingCreated` with the pending subscription
/// if the email is not already subscribed, or `SubscribeResult::AlreadySubscribed`
/// if the email is already verified.
pub async fn create_pending_subscription(
    storage: &Arc<StorageAdapter>,
    email: &EmailAddress,
    strategy: DigestStrategy,
) -> Result<SubscribeResult> {
    if storage.subscriber_exists(email).await? {
        return Ok(SubscribeResult::AlreadySubscribed);
    }

    let pending = PendingSubscription::new(email.clone(), strategy);
    storage.upsert_pending_subscription(&pending).await?;

    Ok(SubscribeResult::PendingCreated(pending))
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

    // Check token first (before verified_at) to prevent email enumeration
    if &pending.token != token {
        return Ok(None);
    }

    // This check mostly exists to avoid generating a new unsubscribe token and
    // invalidating any already-existing unsubscribe links
    if pending.verified_at.is_some() {
        return Ok(Some(Subscriber::new(pending.email, pending.strategy)));
    }

    let subscriber = Subscriber::new(pending.email.clone(), pending.strategy);
    storage.upsert_subscriber(&subscriber).await?;

    let mut verified_pending = pending;
    verified_pending.verified_at = Some(chrono::Utc::now());
    storage
        .upsert_pending_subscription(&verified_pending)
        .await?;

    Ok(Some(subscriber))
}
