//! HTTP API Lambda entrypoint.
//!
//! Wires lambda_http to the framework-agnostic handler in `hndigest::api`.

use aws_config::BehaviorVersion;
use aws_sdk_sesv2::Client as SesClient;
use aws_sdk_ssm::Client as SsmClient;
use hndigest::api::{ApiRequest, ApiResponse, AppState, handle};
use hndigest::captcha::TurnstileCaptcha;
use hndigest::mailer::SesMailer;
use hndigest::storage::LambdaStorage;
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};
use std::collections::HashMap;
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
    let ses_client = SesClient::new(&config);

    let dynamodb_table = env::var("DYNAMODB_TABLE")
        .map_err(|_| Error::from("DYNAMODB_TABLE environment variable must be set"))?;
    let from_address = env::var("EMAIL_FROM")
        .map_err(|_| Error::from("EMAIL_FROM environment variable must be set"))?;
    let reply_to = env::var("EMAIL_REPLY_TO")
        .map_err(|_| Error::from("EMAIL_REPLY_TO environment variable must be set"))?;
    let base_url = env::var("BASE_URL")
        .map_err(|_| Error::from("BASE_URL environment variable must be set"))?;
    let ses_config_set = env::var("SES_CONFIGURATION_SET")
        .map_err(|_| Error::from("SES_CONFIGURATION_SET environment variable must be set"))?;
    let turnstile_param = env::var("TURNSTILE_SECRET_KEY_PARAM")
        .map_err(|_| Error::from("TURNSTILE_SECRET_KEY_PARAM environment variable must be set"))?;

    // In an ideal world this secret would be passed as an environment variable. Using AWS secrets manager
    // costs money, and SSM is effectively free, so instead we do this.
    let ssm_client = SsmClient::new(&config);
    let turnstile_secret = ssm_client
        .get_parameter()
        .name(&turnstile_param)
        .with_decryption(true)
        .send()
        .await?
        .parameter()
        .and_then(|p| p.value.clone())
        .ok_or_else(|| anyhow::format_err!("SSM parameter not found: {}", turnstile_param))?;

    let storage = Arc::new(LambdaStorage::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(AppState::new(
        storage,
        Arc::new(SesMailer::new(
            ses_client,
            from_address,
            reply_to,
            ses_config_set,
        )),
        TurnstileCaptcha::new(turnstile_secret),
        base_url,
    ));

    run(service_fn(|event| handler(event, state.clone()))).await
}

async fn handler(
    event: Request,
    state: Arc<AppState<LambdaStorage, SesMailer, TurnstileCaptcha>>,
) -> Result<Response<Body>, Error> {
    let mut query = HashMap::new();
    for (k, v) in event.query_string_parameters().iter() {
        query.insert(k.to_string(), v.to_string());
    }
    let body_str = match event.body() {
        Body::Text(s) => Some(s.clone()),
        Body::Binary(b) => std::str::from_utf8(b).ok().map(String::from),
        Body::Empty => None,
        // Body is #[non_exhaustive]; this arm guards against future variants.
        _ => None,
    };

    let request = ApiRequest {
        method: event.method().to_string(),
        path: event.uri().path().to_string(),
        query,
        body: body_str,
    };

    let api_response = handle(&request, &state).await;

    let response = match api_response {
        ApiResponse::Html(body) => Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(body))
            .expect("Failed to build HTML response"),
        ApiResponse::Json { status, body } => Response::builder()
            .status(status)
            .header("Content-Type", "application/json; charset=utf-8")
            .body(Body::from(body))
            .expect("Failed to build JSON response"),
        ApiResponse::Text { status, body } => Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from(body))
            .expect("Failed to build text response"),
        ApiResponse::Redirect(location) => Response::builder()
            .status(303)
            .header("Location", location)
            .body(Body::Empty)
            .expect("Failed to build redirect response"),
    };

    Ok(response)
}
