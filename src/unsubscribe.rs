use crate::storage_adapter::StorageAdapter;
use crate::types::{Subscriber, Token};
use anyhow::Result;
use std::sync::Arc;

/// Look up a subscriber by their unsubscribe token.
///
/// Returns `Ok(Some(subscriber))` if found, `Ok(None)` if not found.
pub async fn lookup_subscriber(
    storage: &Arc<StorageAdapter>,
    token: &Token,
) -> Result<Option<Subscriber>> {
    storage.get_subscriber_by_unsubscribe_token(token).await
}

/// Remove a subscriber by their unsubscribe token.
///
/// Returns `Ok(true)` if the subscriber was found and removed,
/// `Ok(false)` if no subscriber was found with that token.
pub async fn remove_subscriber(storage: &Arc<StorageAdapter>, token: &Token) -> Result<bool> {
    let subscriber = match storage.get_subscriber_by_unsubscribe_token(token).await? {
        Some(s) => s,
        None => return Ok(false),
    };

    storage.remove_subscriber(&subscriber.email).await?;

    Ok(true)
}
