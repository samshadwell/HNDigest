use anyhow::Context;
use aws_config::BehaviorVersion;
use chrono::{DateTime, NaiveTime, Utc};
use futures::stream::{self, StreamExt};
use hndigest::digest_builder::DigestBuilder;
use hndigest::mailer::{Mailer, render_digest_html, render_digest_text};
use hndigest::post_fetcher::AlgoliaPostFetcher;
use hndigest::post_snapshotter::PostSnapshotter;
use hndigest::storage::{LambdaStorage, Storage};
use hndigest::strategies::DigestStrategy;
use hndigest::types::Post;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tracing::{error, info};

const MAX_CONCURRENT_EMAILS: usize = 10;

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
    info!("Starting scheduled email handler...");

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
    let base_url = env::var("BASE_URL")
        .map_err(|_| Error::from("BASE_URL environment variable must be set"))?;
    let ses_configuration_set = env::var("SES_CONFIGURATION_SET")
        .map_err(|_| Error::from("SES_CONFIGURATION_SET environment variable must be set"))?;

    let date = Utc::now()
        .date_naive()
        .and_time(NaiveTime::from_hms_opt(run_hour_utc, 0, 0).unwrap())
        .and_utc();

    info!("Processing for date: {}", date);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let ses_client = aws_sdk_sesv2::Client::new(&config);
    let storage = Arc::new(LambdaStorage::new(dynamodb_client, dynamodb_table));
    let mailer = Arc::new(hndigest::mailer::SesMailer::new(
        ses_client,
        email_from,
        email_reply_to,
        ses_configuration_set,
    ));
    let snapshotter = PostSnapshotter::new(Arc::clone(&storage), AlgoliaPostFetcher::new());

    info!("Snapshotting posts...");
    let all_posts_map = snapshotter
        .snapshot(date)
        .await
        .map_err(|e| Error::from(e.to_string()))?;
    let all_posts: Vec<Post> = all_posts_map.into_values().collect();
    info!(posts = all_posts.len(), "Fetched posts");

    let strategies = DigestStrategy::all();
    info!(
        strategies = strategies.len(),
        "Building digests for all strategies"
    );

    let digests_by_strategy = build_all_digests(&strategies, date, &storage, &all_posts).await?;
    info!(
        strategies_with_posts = digests_by_strategy.len(),
        "Built digests"
    );

    let subscribers = storage
        .get_all_subscribers()
        .await
        .map_err(|e| Error::from(e.to_string()))?;
    info!(subscribers = subscribers.len(), "Found subscribers");

    if subscribers.is_empty() {
        info!("No subscribers to send to");
        return Ok(());
    }

    let subject = {
        let base = format!("Hacker Digest for {}", date.format("%b %-d, %Y"));
        match subject_prefix {
            Some(p) => format!("{} {}", p, base),
            None => base,
        }
    };

    let send_results: Vec<_> = stream::iter(subscribers)
        .map(|subscriber| {
            let digests = &digests_by_strategy;
            let mailer = &mailer;
            let subject = &subject;
            let base_url = &base_url;

            async move {
                let posts = match digests.get(&subscriber.strategy) {
                    Some(posts) if !posts.is_empty() => posts,
                    _ => {
                        info!(
                            email = %subscriber.email,
                            strategy = %subscriber.strategy,
                            "No posts for subscriber's strategy, skipping"
                        );
                        return Ok(());
                    }
                };

                let unsubscribe_url = format!(
                    "{}/api/unsubscribe?token={}",
                    base_url, subscriber.unsubscribe_token
                );

                let html = render_digest_html(posts, &unsubscribe_url)
                    .context("Failed to render HTML template")?;
                let text = render_digest_text(posts, &unsubscribe_url)
                    .context("Failed to render text template")?;

                mailer
                    .send_digest(subject, &html, &text, &subscriber.email, &unsubscribe_url)
                    .await
            }
        })
        .buffer_unordered(MAX_CONCURRENT_EMAILS)
        .collect()
        .await;

    let success_count = send_results.iter().filter(|r| r.is_ok()).count();
    let failure_count = send_results.iter().filter(|r| r.is_err()).count();
    info!(
        success = success_count,
        failures = failure_count,
        "Finished sending emails"
    );

    for result in &send_results {
        if let Err(e) = result {
            error!(error = %e, "Email send failed");
        }
    }

    if failure_count > 0 {
        Err(Error::from("Some emails failed to send."))
    } else {
        info!("Handler completed successfully.");
        Ok(())
    }
}

async fn build_all_digests<S: Storage>(
    strategies: &[DigestStrategy],
    date: DateTime<Utc>,
    storage: &Arc<S>,
    all_posts: &[Post],
) -> Result<HashMap<DigestStrategy, Vec<Post>>, Error> {
    let handles: Vec<_> = strategies
        .iter()
        .map(|&strategy| {
            let storage = Arc::clone(storage);
            let posts = all_posts.to_vec();
            async move {
                let digest_posts = DigestBuilder::new(storage)
                    .build_digest(strategy, date, &posts)
                    .await?;
                Ok::<_, anyhow::Error>((strategy, digest_posts))
            }
        })
        .collect();

    let mut digests = HashMap::new();
    for result in futures::future::join_all(handles).await {
        match result {
            Ok((strategy, posts)) if !posts.is_empty() => {
                digests.insert(strategy, posts);
            }
            Ok(_) => {}
            Err(e) => error!(error = %e, "Failed to build digest for strategy"),
        }
    }

    Ok(digests)
}
