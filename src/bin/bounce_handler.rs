//! SNS-triggered Lambda that handles SES bounce and complaint notifications.
//!
//! Architecture: SES -> SNS -> this Lambda
//!
//! On permanent bounces or complaints, removes the subscriber from DynamoDB.
//! Transient bounces are logged and ignored.

use aws_config::BehaviorVersion;
use aws_lambda_events::event::sns::SnsEventObj;
use email_address::EmailAddress;
use hndigest::storage::{LambdaStorage, Storage};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde::{Deserialize, Serialize};
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

// SES notification types (no official Rust types for bounce/complaint payloads)
// See: https://docs.aws.amazon.com/ses/latest/dg/notification-contents.html
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SesNotification {
    event_type: String,
    bounce: Option<BounceNotification>,
    complaint: Option<ComplaintNotification>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BounceNotification {
    bounce_type: String,
    bounced_recipients: Vec<BouncedRecipient>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BouncedRecipient {
    email_address: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComplaintNotification {
    complained_recipients: Vec<ComplainedRecipient>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComplainedRecipient {
    email_address: String,
}

struct HandlerState<S> {
    storage: Arc<S>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let dynamodb_table = env::var("DYNAMODB_TABLE")
        .map_err(|_| Error::from("DYNAMODB_TABLE environment variable must be set"))?;

    let storage = Arc::new(LambdaStorage::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(HandlerState { storage });

    lambda_runtime::run(service_fn(|event| handler(event, state.clone()))).await?;
    Ok(())
}

async fn handler(
    event: LambdaEvent<SnsEventObj<SesNotification>>,
    state: Arc<HandlerState<LambdaStorage>>,
) -> Result<(), Error> {
    for record in &event.payload.records {
        handle_notification(&record.sns.message, &state.storage).await?;
    }
    Ok(())
}

/// Process a single SES notification. Extracted for testability.
async fn handle_notification<S: Storage>(
    notification: &SesNotification,
    storage: &Arc<S>,
) -> Result<(), Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use email_address::EmailAddress;
    use hndigest::storage::Storage;
    use hndigest::strategies::DigestStrategy;
    use hndigest::types::{PendingSubscription, Post, Subscriber, Token};
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Mutex;

    // Minimal FakeStorage for bounce handler tests
    #[derive(Default)]
    struct FakeStorage {
        subscribers: Mutex<HashMap<String, Subscriber>>,
    }

    impl FakeStorage {
        fn new() -> Self {
            Self::default()
        }

        fn with_subscriber(self, s: Subscriber) -> Self {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s);
            self
        }

        fn subscriber_count(&self) -> usize {
            self.subscribers.lock().unwrap().len()
        }

        fn has_subscriber(&self, email: &str) -> bool {
            self.subscribers
                .lock()
                .unwrap()
                .contains_key(&email.to_lowercase())
        }
    }

    impl Storage for FakeStorage {
        async fn snapshot_posts(
            &self,
            _: &HashMap<String, Post>,
            _: chrono::DateTime<chrono::Utc>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn save_digest(
            &self,
            _: &str,
            _: chrono::DateTime<chrono::Utc>,
            _: &[Post],
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn fetch_digest(
            &self,
            _: &str,
            _: chrono::DateTime<chrono::Utc>,
        ) -> anyhow::Result<Option<Vec<Post>>> {
            Ok(None)
        }
        async fn get_subscriber_by_unsubscribe_token(
            &self,
            _: &Token,
        ) -> anyhow::Result<Option<Subscriber>> {
            Ok(None)
        }
        async fn get_all_subscribers(&self) -> anyhow::Result<Vec<Subscriber>> {
            Ok(self.subscribers.lock().unwrap().values().cloned().collect())
        }
        async fn upsert_subscriber(&self, s: &Subscriber) -> anyhow::Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s.clone());
            Ok(())
        }
        async fn remove_subscriber(&self, email: &EmailAddress) -> anyhow::Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .remove(&email.to_string().to_lowercase());
            Ok(())
        }
        async fn upsert_pending_subscription(&self, _: &PendingSubscription) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_pending_subscription(
            &self,
            _: &EmailAddress,
        ) -> anyhow::Result<Option<PendingSubscription>> {
            Ok(None)
        }
        async fn get_subscriber_by_email(
            &self,
            email: &EmailAddress,
        ) -> anyhow::Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }
    }

    fn email(s: &str) -> EmailAddress {
        EmailAddress::from_str(s).unwrap()
    }

    fn make_subscriber(email_str: &str) -> Subscriber {
        Subscriber::new(email(email_str), DigestStrategy::TopN(10))
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
            Arc::new(FakeStorage::new().with_subscriber(make_subscriber("bounce@example.com")));

        handle_notification(&permanent_bounce(&["bounce@example.com"]), &storage)
            .await
            .unwrap();

        assert!(!storage.has_subscriber("bounce@example.com"));
    }

    #[tokio::test]
    async fn permanent_bounce_multiple_recipients_all_removed() {
        let storage = Arc::new(
            FakeStorage::new()
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
            Arc::new(FakeStorage::new().with_subscriber(make_subscriber("trans@example.com")));

        handle_notification(&transient_bounce(&["trans@example.com"]), &storage)
            .await
            .unwrap();

        assert!(storage.has_subscriber("trans@example.com"));
    }

    #[tokio::test]
    async fn complaint_removes_subscriber() {
        let storage =
            Arc::new(FakeStorage::new().with_subscriber(make_subscriber("spam@example.com")));

        handle_notification(&complaint(&["spam@example.com"]), &storage)
            .await
            .unwrap();

        assert!(!storage.has_subscriber("spam@example.com"));
    }

    #[tokio::test]
    async fn invalid_email_in_notification_is_skipped() {
        let storage =
            Arc::new(FakeStorage::new().with_subscriber(make_subscriber("good@example.com")));
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

        // Other subscriber is untouched
        assert!(storage.has_subscriber("good@example.com"));
    }

    #[tokio::test]
    async fn unknown_notification_type_is_ignored() {
        let storage =
            Arc::new(FakeStorage::new().with_subscriber(make_subscriber("x@example.com")));
        let notification = SesNotification {
            event_type: "Delivery".to_string(),
            bounce: None,
            complaint: None,
        };

        handle_notification(&notification, &storage).await.unwrap();

        assert!(storage.has_subscriber("x@example.com"));
    }
}
