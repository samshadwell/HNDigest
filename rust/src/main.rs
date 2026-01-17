mod configuration;
mod types;
mod storage_adapter;
mod post_fetcher;
mod post_snapshotter;
mod strategies;
mod digest_builder;
mod digest_renderer;
mod digest_mailer;

use aws_config::BehaviorVersion;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;
use chrono::{Timelike, Utc};
use log::info;
use crate::storage_adapter::StorageAdapter;
use crate::post_snapshotter::PostSnapshotter;
use crate::digest_builder::DigestBuilder;
use crate::digest_mailer::DigestMailer;
use crate::digest_renderer::DigestRenderer;
use crate::strategies::{DigestStrategy, TopNPosts, OverPointThreshold};
use crate::configuration::{TOP_N_VALUES, POINT_THRESHOLD_VALUES};

const SNAPSHOT_DAILY_HOUR: u32 = 5;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    
    let func = service_fn(func);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn func(_event: LambdaEvent<Value>) -> Result<(), Error> {
    info!("Starting HNDigest handler...");
    
    // Determine the date: Today at 5 AM UTC
    let now = Utc::now();
    // Use `with_hour` etc to set time to 5:00:00
    // If it fails (invalid time), panic is acceptable (5 AM is valid)
    let date = now
        .with_hour(SNAPSHOT_DAILY_HOUR).unwrap()
        .with_minute(0).unwrap()
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();
        
    info!("Processing for date: {}", date);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = aws_sdk_ses::Client::new(&config);

    let storage_adapter = StorageAdapter::new(dynamodb_client);
    let snapshotter = PostSnapshotter::new(&storage_adapter);
    
    info!("Snapshotting posts...");
    let all_posts_map = snapshotter.snapshot(date).await.map_err(|e| Error::from(e.to_string()))?;
    let all_posts: Vec<_> = all_posts_map.values().cloned().collect();

    let digest_builder = DigestBuilder::new(&storage_adapter);
    let mailer = DigestMailer::new(ses_client);

    // Build strategies list
    let mut strategies: Vec<Box<dyn DigestStrategy>> = Vec::new();
    for &n in TOP_N_VALUES {
        strategies.push(Box::new(TopNPosts { n }));
    }
    for &t in POINT_THRESHOLD_VALUES {
        strategies.push(Box::new(OverPointThreshold { threshold: t }));
    }

    for strategy in strategies {
        info!("Processing strategy: {}", strategy.type_());
        
        let posts = digest_builder.build_digest(
            strategy.as_ref(), // Pass as ref to trait object
            date,
            &all_posts
        ).await.map_err(|e| Error::from(e.to_string()))?;

        info!("Selected {} posts for digest", posts.len());

        // Check if subscribers exist before rendering?
        // Ruby: subscribers = storage_adapter.fetch_subscribers
        // next if nil or empty
        // mailer.send_mail
        
        let subscribers = storage_adapter.fetch_subscribers(&strategy.type_())
            .await.map_err(|e| Error::from(e.to_string()))?;
            
        if let Some(subs) = subscribers {
            if !subs.is_empty() {
                let renderer = DigestRenderer::new(&posts, date);
                mailer.send_mail(&renderer, &subs).await.map_err(|e| Error::from(e.to_string()))?;
            } else {
                info!("No subscribers for strategy {}", strategy.type_());
            }
        } else {
            info!("No subscribers (None) for strategy {}", strategy.type_());
        }
    }

    info!("Handler completed successfully.");
    Ok(())
}
