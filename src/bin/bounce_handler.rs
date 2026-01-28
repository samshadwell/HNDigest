//! SNS-triggered Lambda that handles SES bounce and complaint notifications.
//!
//! Architecture: SES -> SNS -> this Lambda
//!
//! On permanent bounces or complaints, removes the subscriber from DynamoDB.
//! Transient bounces are logged and ignored.

use aws_config::BehaviorVersion;
use aws_lambda_events::event::sns::SnsEventObj;
use email_address::EmailAddress;
use hndigest::storage_adapter::StorageAdapter;
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
    notification_type: String,
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

struct HandlerState {
    storage: Arc<StorageAdapter>,
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

    let storage = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(HandlerState { storage });

    lambda_runtime::run(service_fn(|event| handler(event, state.clone()))).await?;
    Ok(())
}

async fn handler(
    event: LambdaEvent<SnsEventObj<SesNotification>>,
    state: Arc<HandlerState>,
) -> Result<(), Error> {
    for record in &event.payload.records {
        let notification = &record.sns.message;

        match notification.notification_type.as_str() {
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
                        remove_subscriber(&state.storage, &recipient.email_address).await;
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
                let complaint = notification.complaint.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Complaint notification missing complaint field")
                })?;

                info!(
                    recipients = ?complaint.complained_recipients.iter().map(|r| &r.email_address).collect::<Vec<_>>(),
                    "Complaint received — removing subscribers"
                );
                for recipient in &complaint.complained_recipients {
                    remove_subscriber(&state.storage, &recipient.email_address).await;
                }
            }
            other => {
                info!(notification_type = %other, "Ignoring notification type");
            }
        }
    }

    Ok(())
}

async fn remove_subscriber(storage: &Arc<StorageAdapter>, email_str: &str) {
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
