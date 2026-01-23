//! Migration script to convert existing subscribers from the old model to the new model.
//!
//! Old model: PK="SUBSCRIBERS", SK="{strategy}" -> emails: ["alice@...", "bob@..."]
//! New model: PK="SUBSCRIBER", SK="{email}" -> strategy, subscribed_at, verified_at
//!
//! Usage:
//!   DYNAMODB_TABLE=hndigest-prod cargo run --bin migrate-subscribers
//!
//! Add --dry-run to see what would be migrated without making changes.

use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use chrono::Utc;
use hndigest::storage_adapter::StorageAdapter;
use hndigest::strategies::DigestStrategy;
use hndigest::types::Subscriber;
use std::collections::HashSet;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let table_name = env::var("DYNAMODB_TABLE")
        .map_err(|_| anyhow::anyhow!("DYNAMODB_TABLE environment variable must be set"))?;

    println!("Migration: Old subscriber model -> New subscriber model");
    println!("Table: {}", table_name);
    println!("Mode: {}", if dry_run { "DRY RUN" } else { "LIVE" });
    println!();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let storage = StorageAdapter::new(dynamodb_client, table_name);

    // Build list of all strategies
    let strategies = DigestStrategy::all();

    let migration_time = Utc::now();
    let mut migrated_count = 0;
    let mut seen_emails: HashSet<String> = HashSet::new();

    for strategy in strategies {
        let strategy_name = strategy.to_string();
        println!("Processing strategy: {}", strategy_name);

        let subscribers = storage
            .fetch_subscribers(&strategy_name)
            .await
            .context(format!("Failed to fetch subscribers for {}", strategy_name))?;

        let Some(emails) = subscribers else {
            println!("  No subscribers found for this strategy");
            continue;
        };

        println!("  Found {} email(s)", emails.len());

        for email in emails {
            let email_lower = email.to_lowercase();

            // Check if we've already seen this email in another strategy
            if seen_emails.contains(&email_lower) {
                println!(
                    "  SKIPPING {} - already migrated with different strategy",
                    email_lower
                );
                continue;
            }
            seen_emails.insert(email_lower.clone());

            let subscriber = Subscriber {
                email: email_lower.clone(),
                strategy,
                subscribed_at: migration_time,
                verified_at: Some(migration_time), // Grandfathered as verified
            };

            if dry_run {
                println!(
                    "  [DRY RUN] Would migrate: {} -> {}",
                    email_lower, strategy_name
                );
            } else {
                storage
                    .set_subscriber(&subscriber)
                    .await
                    .context(format!("Failed to migrate {}", email_lower))?;
                println!("  Migrated: {} -> {}", email_lower, strategy_name);
            }

            migrated_count += 1;
        }
    }

    println!();
    println!("Migration complete!");
    println!(
        "Total subscribers {}: {}",
        if dry_run { "to migrate" } else { "migrated" },
        migrated_count
    );

    if !dry_run {
        println!();
        println!("NOTE: Old subscriber records (PK=SUBSCRIBERS) were NOT deleted.");
        println!("Verify the migration was successful before manually deleting them.");
    }

    Ok(())
}
