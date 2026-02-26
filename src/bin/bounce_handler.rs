//! SNS-triggered Lambda entrypoint for SES bounce and complaint handling.
//!
//! Architecture: SES -> SNS -> this Lambda
//!
//! Delegates notification processing to `hndigest::bounce`.

use aws_config::BehaviorVersion;
use aws_lambda_events::event::sns::SnsEventObj;
use hndigest::bounce::SesNotification;
use hndigest::storage::LambdaStorage;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use std::env;
use std::sync::Arc;

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

    lambda_runtime::run(service_fn(|event| handler(event, storage.clone()))).await?;
    Ok(())
}

async fn handler(
    event: LambdaEvent<SnsEventObj<SesNotification>>,
    storage: Arc<LambdaStorage>,
) -> Result<(), Error> {
    for record in &event.payload.records {
        hndigest::bounce::handle_notification(&record.sns.message, &storage)
            .await
            .map_err(|e| Error::from(e.to_string()))?;
    }
    Ok(())
}
