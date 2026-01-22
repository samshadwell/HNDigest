mod configuration;
mod digest_builder;
mod digest_mailer;
mod post_fetcher;
mod post_snapshotter;
mod storage_adapter;
mod strategies;
mod types;

use crate::configuration::{POINT_THRESHOLD_VALUES, TOP_N_VALUES};
use crate::digest_builder::DigestBuilder;
use crate::digest_mailer::DigestMailer;
use crate::post_snapshotter::PostSnapshotter;
use crate::storage_adapter::StorageAdapter;
use crate::strategies::DigestStrategy;
use crate::types::Post;
use askama::Template;
use aws_config::BehaviorVersion;
use chrono::{NaiveTime, Utc};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde_json::Value;
use std::env;
use std::sync::Arc;
use tracing::{Instrument, error, info, info_span};

const SNAPSHOT_DAILY_HOUR: u32 = 5;

#[derive(Template)]
#[template(path = "digest.html")]
struct DigestTemplate<'a> {
    posts: &'a [Post],
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    lambda_runtime::run(service_fn(handler)).await?;
    Ok(())
}

async fn handler(_event: LambdaEvent<Value>) -> Result<(), Error> {
    info!("Starting HNDigest handler...");

    let date = Utc::now()
        .date_naive()
        .and_time(NaiveTime::from_hms_opt(SNAPSHOT_DAILY_HOUR, 0, 0).unwrap())
        .and_utc();

    info!("Processing for date: {}", date);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = aws_sdk_ses::Client::new(&config);
    let storage_adapter =
        Arc::new(StorageAdapter::new(dynamodb_client).map_err(|e| Error::from(e.to_string()))?);
    let mailer = Arc::new(DigestMailer::new(ses_client).map_err(|e| Error::from(e.to_string()))?);
    let snapshotter = PostSnapshotter::new(&storage_adapter);

    info!("Snapshotting posts...");
    let all_posts_map = snapshotter
        .snapshot(date)
        .await
        .map_err(|e| Error::from(e.to_string()))?;
    let all_posts: Arc<Vec<Post>> = Arc::new(all_posts_map.into_values().collect());

    let strategies: Vec<DigestStrategy> = TOP_N_VALUES
        .iter()
        .map(|&n| DigestStrategy::TopN(n))
        .chain(
            POINT_THRESHOLD_VALUES
                .iter()
                .map(|&t| DigestStrategy::OverPointThreshold(t)),
        )
        .collect();

    let handles: Vec<_> = strategies
        .into_iter()
        .map(|strategy| {
            let storage_adapter = Arc::clone(&storage_adapter);
            let mailer = Arc::clone(&mailer);
            let all_posts = Arc::clone(&all_posts);
            let span = info_span!("strategy", name = %strategy);

            tokio::spawn(
                async move {
                    process_strategy(strategy, date, storage_adapter, &mailer, &all_posts).await
                }
                .instrument(span),
            )
        })
        .collect();

    let results = futures::future::join_all(handles).await;
    for res in results {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => error!(error = %e, "Strategy execution failed"),
            Err(e) => error!(error = %e, "Task panicked"),
        }
    }

    info!("Handler completed successfully.");
    Ok(())
}

async fn process_strategy(
    strategy: DigestStrategy,
    date: chrono::DateTime<Utc>,
    storage_adapter: Arc<StorageAdapter>,
    mailer: &DigestMailer,
    all_posts: &[Post],
) -> anyhow::Result<()> {
    info!("Processing strategy");
    let digest_builder = DigestBuilder::new(Arc::clone(&storage_adapter));

    let posts = digest_builder
        .build_digest(strategy, date, all_posts)
        .await?;

    info!(posts = posts.len(), "Selected posts for digest");

    if posts.is_empty() {
        info!("No posts for strategy, skipping");
        return Ok(());
    }

    let strategy_name = strategy.to_string();
    let subscribers = storage_adapter.fetch_subscribers(&strategy_name).await?;

    let Some(subs) = subscribers.filter(|s| !s.is_empty()) else {
        info!("No subscribers for strategy");
        return Ok(());
    };

    let tmpl = DigestTemplate { posts: &posts };
    let content = tmpl.render()?;
    let base_subject = format!("Hacker News Digest for {}", date.format("%b %-d, %Y"));
    let subject = match env::var("SUBJECT_PREFIX") {
        Ok(prefix) if !prefix.is_empty() => format!("{} {}", prefix, base_subject),
        _ => base_subject,
    };

    mailer.send_mail(&subject, &content, &subs).await?;

    Ok(())
}
