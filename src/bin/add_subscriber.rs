use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use chrono::Utc;
use hndigest::storage_adapter::StorageAdapter;
use hndigest::strategies::DigestStrategy;
use hndigest::types::Subscriber;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin add-subscriber <email> [strategy]");
        eprintln!(
            "Example: DYNAMODB_TABLE=HNDigest-staging cargo run --bin add-subscriber test@example.com TOP_N#10"
        );
        std::process::exit(1);
    }

    let email = &args[1];
    let strategy_str = if args.len() > 2 { &args[2] } else { "TOP_N#10" };

    let strategy = DigestStrategy::from_str(strategy_str).context("Invalid strategy")?;

    let dynamodb_table =
        env::var("DYNAMODB_TABLE").context("DYNAMODB_TABLE environment variable must be set")?;

    println!("Initializing DynamoDB client...");
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let storage_adapter = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));

    println!("Creating verified subscriber for {}", email);
    let mut subscriber = Subscriber::new(email.to_string(), strategy);
    subscriber.verified_at = Some(Utc::now());

    storage_adapter.set_subscriber(&subscriber).await?;

    println!("Successfully added verified subscriber:");
    println!("  Email: {}", subscriber.email);
    println!("  Strategy: {}", subscriber.strategy);
    println!("  Unsubscribe Token: {}", subscriber.unsubscribe_token);

    Ok(())
}
