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

    // Read all configuration from environment variables
    let run_hour_utc: u32 = env::var("RUN_HOUR_UTC")
        .map_err(|_| Error::from("RUN_HOUR_UTC environment variable must be set"))?
        .parse()
        .map_err(|_| Error::from("RUN_HOUR_UTC must be a valid number"))?;
    let dynamodb_table = env::var("DYNAMODB_TABLE")
        .map_err(|_| Error::from("DYNAMODB_TABLE environment variable must be set"))?;
    let email_from = env::var("EMAIL_FROM")
        .map_err(|_| Error::from("EMAIL_FROM environment variable must be set"))?;
    let email_reply_to = env::var("EMAIL_REPLY_TO")
        .map_err(|_| Error::from("EMAIL_REPLY_TO environment variable must be set"))?;
    let subject_prefix = env::var("SUBJECT_PREFIX").ok().filter(|s| !s.is_empty());

    let date = Utc::now()
        .date_naive()
        .and_time(NaiveTime::from_hms_opt(run_hour_utc, 0, 0).unwrap())
        .and_utc();

    info!("Processing for date: {}", date);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = aws_sdk_ses::Client::new(&config);
    let storage_adapter = Arc::new(StorageAdapter::new(dynamodb_client, dynamodb_table));
    let mailer = Arc::new(DigestMailer::new(ses_client, email_from, email_reply_to));
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
            let subject_prefix = subject_prefix.clone();
            let span = info_span!("strategy", name = %strategy);

            tokio::spawn(
                async move {
                    process_strategy(
                        strategy,
                        date,
                        storage_adapter,
                        &mailer,
                        &all_posts,
                        subject_prefix.as_deref(),
                    )
                    .await
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
    subject_prefix: Option<&str>,
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
    let subject = match subject_prefix {
        Some(prefix) => format!("{} {}", prefix, base_subject),
        None => base_subject,
    };

    mailer.send_mail(&subject, &content, &subs).await?;

    Ok(())
}
