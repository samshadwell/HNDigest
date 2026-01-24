use crate::storage_adapter::StorageAdapter;
use crate::types::Subscriber;
use anyhow::{Result, bail};
use std::sync::Arc;

/// Look up a subscriber by their unsubscribe token.
///
/// Returns `Ok(Some(subscriber))` if found, `Ok(None)` if not found,
/// or an error if the token is empty or a database error occurs.
pub async fn lookup_subscriber(
    storage: &Arc<StorageAdapter>,
    token: &str,
) -> Result<Option<Subscriber>> {
    if token.is_empty() {
        bail!("Missing unsubscribe token");
    }

    storage.get_subscriber_by_token(token).await
}

/// Remove a subscriber by their unsubscribe token.
///
/// Returns `Ok(true)` if the subscriber was found and removed,
/// `Ok(false)` if no subscriber was found with that token,
/// or an error if the token is empty or a database error occurs.
pub async fn remove_subscriber(storage: &Arc<StorageAdapter>, token: &str) -> Result<bool> {
    if token.is_empty() {
        bail!("Missing unsubscribe token");
    }

    // Look up the subscriber by token
    let subscriber = match storage.get_subscriber_by_token(token).await? {
        Some(s) => s,
        None => return Ok(false),
    };

    // Remove the subscriber
    storage.remove_subscriber(&subscriber.email).await?;

    Ok(true)
}
