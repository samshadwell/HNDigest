//! SES bounce and complaint notification handling.
//!
//! Business logic for processing SES notification payloads received via SNS.
//! The Lambda entrypoint in `src/bin/bounce_handler.rs` handles unwrapping the
//! SNS event and delegates here.

use crate::storage::Storage;
use anyhow::Result;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

// SES notification types — no official Rust SDK types for bounce/complaint payloads.
// See: https://docs.aws.amazon.com/ses/latest/dg/notification-contents.html

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SesNotification {
    pub event_type: String,
    pub bounce: Option<BounceNotification>,
    pub complaint: Option<ComplaintNotification>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BounceNotification {
    pub bounce_type: String,
    pub bounced_recipients: Vec<BouncedRecipient>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BouncedRecipient {
    pub email_address: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintNotification {
    pub complained_recipients: Vec<ComplainedRecipient>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplainedRecipient {
    pub email_address: String,
}

/// Process a single SES notification, removing subscribers on permanent bounces
/// or complaints.
pub async fn handle_notification<S: Storage>(
    notification: &SesNotification,
    storage: &Arc<S>,
) -> Result<()> {
    match notification.event_type.as_str() {
        "Bounce" => {
            let bounce = notification
                .bounce
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Bounce notification missing bounce field"))?;

            if bounce.bounce_type == "Permanent" {
                info!(
                    bounce_type = %bounce.bounce_type,
                    recipients = ?bounce.bounced_recipients.iter().map(|r| &r.email_address).collect::<Vec<_>>(),
                    "Permanent bounce — removing subscribers"
                );
                for recipient in &bounce.bounced_recipients {
                    remove_subscriber(storage, &recipient.email_address).await;
                }
            } else {
                info!(
                    bounce_type = %bounce.bounce_type,
                    recipients = ?bounce.bounced_recipients.iter().map(|r| &r.email_address).collect::<Vec<_>>(),
                    "Transient bounce — ignoring"
                );
            }
        }
        "Complaint" => {
            let complaint = notification
                .complaint
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Complaint notification missing complaint field"))?;

            info!(
                recipients = ?complaint.complained_recipients.iter().map(|r| &r.email_address).collect::<Vec<_>>(),
                "Complaint received — removing subscribers"
            );
            for recipient in &complaint.complained_recipients {
                remove_subscriber(storage, &recipient.email_address).await;
            }
        }
        other => {
            info!(notification_type = %other, "Ignoring notification type");
        }
    }
    Ok(())
}

async fn remove_subscriber<S: Storage>(storage: &Arc<S>, email_str: &str) {
    let email = match EmailAddress::from_str(email_str) {
        Ok(e) => e,
        Err(e) => {
            warn!(email = %email_str, error = %e, "Invalid email address in notification — skipping");
            return;
        }
    };

    match storage.remove_subscriber(&email).await {
        Ok(()) => info!(email = %email, "Subscriber removed"),
        Err(e) => warn!(email = %email, error = %e, "Failed to remove subscriber"),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_utils::InMemoryStorage;
    use crate::strategies::DigestStrategy;
    use crate::types::Subscriber;
    use email_address::EmailAddress;
    use std::str::FromStr;

    fn make_subscriber(email_str: &str) -> Subscriber {
        Subscriber::new(
            EmailAddress::from_str(email_str).unwrap(),
            DigestStrategy::TopN(10),
        )
    }

    fn permanent_bounce(emails: &[&str]) -> SesNotification {
        SesNotification {
            event_type: "Bounce".to_string(),
            bounce: Some(BounceNotification {
                bounce_type: "Permanent".to_string(),
                bounced_recipients: emails
                    .iter()
                    .map(|e| BouncedRecipient {
                        email_address: e.to_string(),
                    })
                    .collect(),
            }),
            complaint: None,
        }
    }

    fn transient_bounce(emails: &[&str]) -> SesNotification {
        SesNotification {
            event_type: "Bounce".to_string(),
            bounce: Some(BounceNotification {
                bounce_type: "Transient".to_string(),
                bounced_recipients: emails
                    .iter()
                    .map(|e| BouncedRecipient {
                        email_address: e.to_string(),
                    })
                    .collect(),
            }),
            complaint: None,
        }
    }

    fn complaint(emails: &[&str]) -> SesNotification {
        SesNotification {
            event_type: "Complaint".to_string(),
            bounce: None,
            complaint: Some(ComplaintNotification {
                complained_recipients: emails
                    .iter()
                    .map(|e| ComplainedRecipient {
                        email_address: e.to_string(),
                    })
                    .collect(),
            }),
        }
    }

    #[tokio::test]
    async fn permanent_bounce_removes_subscriber() {
        let storage =
            Arc::new(InMemoryStorage::new().with_subscriber(make_subscriber("bounce@example.com")));
        handle_notification(&permanent_bounce(&["bounce@example.com"]), &storage)
            .await
            .unwrap();
        assert!(!storage.has_subscriber("bounce@example.com"));
    }

    #[tokio::test]
    async fn permanent_bounce_multiple_recipients_all_removed() {
        let storage = Arc::new(
            InMemoryStorage::new()
                .with_subscriber(make_subscriber("a@example.com"))
                .with_subscriber(make_subscriber("b@example.com")),
        );
        handle_notification(
            &permanent_bounce(&["a@example.com", "b@example.com"]),
            &storage,
        )
        .await
        .unwrap();
        assert_eq!(storage.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn transient_bounce_does_not_remove_subscriber() {
        let storage =
            Arc::new(InMemoryStorage::new().with_subscriber(make_subscriber("trans@example.com")));
        handle_notification(&transient_bounce(&["trans@example.com"]), &storage)
            .await
            .unwrap();
        assert!(storage.has_subscriber("trans@example.com"));
    }

    #[tokio::test]
    async fn complaint_removes_subscriber() {
        let storage =
            Arc::new(InMemoryStorage::new().with_subscriber(make_subscriber("spam@example.com")));
        handle_notification(&complaint(&["spam@example.com"]), &storage)
            .await
            .unwrap();
        assert!(!storage.has_subscriber("spam@example.com"));
    }

    #[tokio::test]
    async fn invalid_email_in_notification_is_skipped() {
        let storage =
            Arc::new(InMemoryStorage::new().with_subscriber(make_subscriber("good@example.com")));
        let notification = SesNotification {
            event_type: "Bounce".to_string(),
            bounce: Some(BounceNotification {
                bounce_type: "Permanent".to_string(),
                bounced_recipients: vec![BouncedRecipient {
                    email_address: "not-an-email".to_string(),
                }],
            }),
            complaint: None,
        };
        handle_notification(&notification, &storage).await.unwrap();
        assert!(storage.has_subscriber("good@example.com"));
    }

    #[tokio::test]
    async fn unknown_notification_type_is_ignored() {
        let storage =
            Arc::new(InMemoryStorage::new().with_subscriber(make_subscriber("x@example.com")));
        let notification = SesNotification {
            event_type: "Delivery".to_string(),
            bounce: None,
            complaint: None,
        };
        handle_notification(&notification, &storage).await.unwrap();
        assert!(storage.has_subscriber("x@example.com"));
    }
}
