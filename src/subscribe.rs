//! Business logic for subscription management.
//!
//! Handles creating pending subscriptions and verifying them.

use crate::storage_adapter::StorageAdapter;
use crate::strategies::DigestStrategy;
use crate::types::{PendingSubscription, Subscriber};
use anyhow::{Result, bail};
use chrono::Utc;
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
    email: &str,
    strategy: DigestStrategy,
) -> Result<SubscribeResult> {
    let email = email.trim().to_lowercase();

    if !is_valid_email(&email) {
        bail!("Invalid email address");
    }

    // Check if already subscribed
    if storage.subscriber_exists(&email).await? {
        return Ok(SubscribeResult::AlreadySubscribed);
    }

    // Create pending subscription
    let pending = PendingSubscription::new(email, strategy);
    storage.create_pending_subscription(&pending).await?;

    Ok(SubscribeResult::PendingCreated(pending))
}

/// Verify a pending subscription by token.
///
/// Returns `Ok(Some(subscriber))` if the subscription was verified successfully,
/// `Ok(None)` if the token was not found or expired,
/// or an error if a database error occurs.
pub async fn verify_subscription(
    storage: &Arc<StorageAdapter>,
    token: &str,
) -> Result<Option<Subscriber>> {
    if token.is_empty() {
        bail!("Missing verification token");
    }

    // Look up the pending subscription
    let pending = match storage.get_pending_subscription(token).await? {
        Some(p) => p,
        None => return Ok(None),
    };

    // Check if expired
    if pending.is_expired() {
        // Clean up the expired record
        storage.delete_pending_subscription(token).await?;
        return Ok(None);
    }

    // Check if already subscribed (race condition protection)
    if storage.subscriber_exists(&pending.email).await? {
        // Clean up the pending subscription
        storage.delete_pending_subscription(token).await?;
        // Return a subscriber object so the user sees success
        // (they're already subscribed, which is the goal)
        return Ok(Some(Subscriber {
            email: pending.email,
            strategy: pending.strategy,
            subscribed_at: Utc::now(),
            verified_at: Some(Utc::now()),
            unsubscribe_token: String::new(), // Not needed for this response
        }));
    }

    // Create verified subscriber
    let mut subscriber = Subscriber::new(pending.email, pending.strategy);
    subscriber.verified_at = Some(Utc::now());
    storage.set_subscriber(&subscriber).await?;

    // Delete the pending subscription
    storage.delete_pending_subscription(token).await?;

    Ok(Some(subscriber))
}

/// Basic email validation.
fn is_valid_email(email: &str) -> bool {
    // Must contain exactly one @
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Local part must not be empty
    if local.is_empty() {
        return false;
    }

    // Domain must contain at least one dot and not be empty
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    // Domain must not start or end with a dot
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_emails() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@example.co.uk"));
        assert!(is_valid_email("user+tag@example.com"));
        assert!(is_valid_email("a@b.co"));
    }

    #[test]
    fn test_invalid_emails() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("noatsign"));
        assert!(!is_valid_email("@nodomain.com"));
        assert!(!is_valid_email("nolocal@"));
        assert!(!is_valid_email("no@dotindomain"));
        assert!(!is_valid_email("two@@ats.com"));
        assert!(!is_valid_email("dot@.start.com"));
        assert!(!is_valid_email("dot@end.com."));
    }
}
