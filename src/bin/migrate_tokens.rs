//! Migration script to add unsubscribe tokens to existing subscribers.
//!
//! This script reads all subscribers from DynamoDB and adds an unsubscribe_token
//! to any subscriber record that doesn't have one.
//!
//! Usage:
//!   DYNAMODB_TABLE=hndigest cargo run --bin migrate-tokens
//!
//! For staging:
//!   DYNAMODB_TABLE=hndigest-staging cargo run --bin migrate-tokens

use aws_config::BehaviorVersion;
use hndigest::storage_adapter::StorageAdapter;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .init();

    let dynamodb_table = env::var("DYNAMODB_TABLE").unwrap_or_else(|_| {
        eprintln!("DYNAMODB_TABLE environment variable not set.");
        eprintln!("Usage: DYNAMODB_TABLE=hndigest cargo run --bin migrate-tokens");
        std::process::exit(1);
    });

    println!("Migration: Adding unsubscribe tokens to subscribers");
    println!("Table: {}", dynamodb_table);
    println!();

    // Initialize AWS clients
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let storage = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));

    // Get all subscribers
    println!("Fetching all subscribers...");
    let subscribers = storage.get_all_subscribers().await?;
    println!("Found {} subscribers", subscribers.len());
    println!();

    // Process each subscriber
    let mut updated_count = 0;
    let mut skipped_count = 0;

    for subscriber in subscribers {
        // The storage_adapter.get_all_subscribers() already generates a token
        // for subscribers without one (see subscriber_from_item in storage_adapter.rs).
        // We just need to save the subscriber back to persist the generated token.
        //
        // In a production scenario, you might want to check if the token was
        // actually generated vs. already existing, but since UUIDs are unique
        // and the field is new, we can safely update all records.

        println!(
            "Processing: {} (strategy: {})",
            subscriber.email, subscriber.strategy
        );

        // Save the subscriber (this will persist the generated token)
        match storage.set_subscriber(&subscriber).await {
            Ok(_) => {
                println!("  -> Token set: {}", subscriber.unsubscribe_token);
                updated_count += 1;
            }
            Err(e) => {
                eprintln!("  -> ERROR: {}", e);
                skipped_count += 1;
            }
        }
    }

    println!();
    println!("Migration complete!");
    println!("  Updated: {}", updated_count);
    println!("  Errors:  {}", skipped_count);

    Ok(())
}
