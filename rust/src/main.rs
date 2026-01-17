mod configuration;
mod types;
mod storage_adapter;
mod post_fetcher;
mod post_snapshotter;
mod strategies;
mod digest_builder;
mod digest_mailer;

use aws_config::BehaviorVersion;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;
use chrono::{Timelike, Utc};
use log::{info, error};
use std::sync::Arc;
use askama::Template;
use crate::storage_adapter::StorageAdapter;
use crate::post_snapshotter::PostSnapshotter;
use crate::digest_builder::DigestBuilder;
use crate::digest_mailer::DigestMailer;
use crate::strategies::{DigestStrategy, TopNPosts, OverPointThreshold};
use crate::configuration::{TOP_N_VALUES, POINT_THRESHOLD_VALUES};
use crate::types::Post;

const SNAPSHOT_DAILY_HOUR: u32 = 5;

#[derive(Template)]
#[template(path = "digest.html")]
struct DigestTemplate<'a> {
    posts: &'a [Post],
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    
    let func = service_fn(func);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn func(_event: LambdaEvent<Value>) -> Result<(), Error> {
    info!("Starting HNDigest handler...");
    
    let now = Utc::now();
    let date = now
        .with_hour(SNAPSHOT_DAILY_HOUR).unwrap()
        .with_minute(0).unwrap()
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();
        
    info!("Processing for date: {}", date);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = aws_sdk_ses::Client::new(&config);

    // Wrap clients in Arc for sharing across tasks
    // Actually, StorageAdapter and DigestMailer are cheap to clone if they just hold Client? 
    // Types in AWS SDK are Clone (store Arc internally).
    // Let's check StorageAdapter definition. It holds `Client`.
    // DigestMailer holds `Client`.
    // So we can clone the adapters themselves if we simple derive Clone or implement it, OR wrapped in Arc.
    // Creating them inside Arc is easier.
    
    let storage_adapter = Arc::new(StorageAdapter::new(dynamodb_client));
    let mailer = Arc::new(DigestMailer::new(ses_client));
    // let digest_builder = Arc::new(DigestBuilder::new(&storage_adapter)); // DigestBuilder holds reference to StorageAdapter

    // Problem: DigestBuilder holds reference `&'a StorageAdapter`.
    // We should change DigestBuilder to hold `Arc<StorageAdapter>` or just `StorageAdapter` (since Client is cheap clone).
    // Let's assume we modify DigestBuilder to own StorageAdapter (which owns Client).
    // For now, let's keep it simple and just clone the clients if needed or share Arc.
    
    // Actually, let's rewrite DigestBuilder to own the adapter to make life easier with 'static lifetimes in tokio::spawn.
    // Wait, let's do that fix first.
    
    // Re-evaluating:
    // Helper function to process strategy.
    
    let snapshotter = PostSnapshotter::new(&storage_adapter); // Needs to own or ref? currently ref.
    
    info!("Snapshotting posts...");
    let all_posts_map = snapshotter.snapshot(date).await.map_err(|e| Error::from(e.to_string()))?;
    let all_posts: Arc<Vec<Post>> = Arc::new(all_posts_map.values().cloned().collect());

    // Build strategies list
    let mut strategies: Vec<Box<dyn DigestStrategy + Send + Sync>> = Vec::new();
    for &n in TOP_N_VALUES {
        strategies.push(Box::new(TopNPosts { n }));
    }
    for &t in POINT_THRESHOLD_VALUES {
        strategies.push(Box::new(OverPointThreshold { threshold: t }));
    }

    let mut handles = Vec::new();

    for strategy in strategies {
        let storage_adapter = storage_adapter.clone();
        let mailer = mailer.clone();
        let all_posts = all_posts.clone();
        // digest_builder holds &StorageAdapter. Passing ref across thread boundary is hard without Arc.
        // We will construct DigestBuilder inside the task or change definition.
        
        handles.push(tokio::spawn(async move {
            let strategy_type = strategy.type_();
            info!("Processing strategy: {}", strategy_type);
            
            // Re-create builder or change its definition?
            // Let's create `DigestBuilder` here. It takes `&StorageAdapter`. 
            // `storage_adapter` is `Arc<StorageAdapter>`.
            let digest_builder = DigestBuilder::new(storage_adapter.clone());

            let posts = match digest_builder.build_digest(
                strategy.as_ref(), 
                date,
                &all_posts
            ).await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to build digest for {}: {}", strategy_type, e);
                    return Err(e);
                }
            };

            info!("Selected {} posts for digest {}", posts.len(), strategy_type);

            let subscribers = match storage_adapter.fetch_subscribers(&strategy_type).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to fetch subscribers for {}: {}", strategy_type, e);
                    return Err(e);
                }
            };
            
            if let Some(subs) = subscribers {
                if !subs.is_empty() {
                    let tmpl = DigestTemplate { posts: &posts };
                    let content = match tmpl.render() {
                        Ok(c) => c,
                        Err(e) => {
                             error!("Failed to render template for {}: {}", strategy_type, e);
                             return Err(anyhow::Error::from(e));
                        }
                    };
                    
                    let subject = format!("Hacker News Digest for {}", date.format("%b %-d, %Y"));
                    
                    if let Err(e) = mailer.send_mail(&subject, &content, &subs).await {
                         error!("Failed to send mail for {}: {}", strategy_type, e);
                         return Err(e);
                    }
                } else {
                    info!("No subscribers for strategy {}", strategy_type);
                }
            } else {
                info!("No subscribers (None) for strategy {}", strategy_type);
            }
            Ok::<(), anyhow::Error>(())
        }));
    }

    // Proper way to join
    let results = futures::future::join_all(handles).await;
    for res in results {
        match res {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => error!("Strategy execution failed: {}", e),
            Err(e) => error!("Task failed: {}", e),
        }
    }

    info!("Handler completed successfully.");
    Ok(())
}
