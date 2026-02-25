//! HTTP API Lambda for subscription management.
//!
//! Handles HTTP requests for subscribe, verify, and unsubscribe endpoints.
//! Business logic is in the `subscribe` and `unsubscribe` modules; this file
//! handles HTTP concerns (routing, templates, responses, redirects).

use askama::Template;
use aws_config::BehaviorVersion;
use aws_sdk_sesv2::Client as SesClient;
use aws_sdk_ssm::Client as SsmClient;
use email_address::EmailAddress;
use hndigest::mailer::{Mailer, SesMailer};
use hndigest::storage::{LambdaStorage, Storage};
use hndigest::strategies::DigestStrategy;
use hndigest::subscribe;
use hndigest::types::Token;
use hndigest::unsubscribe;
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};
use reqwest::Method;
use serde::Deserialize;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// Captcha trait
// ============================================================================

#[allow(async_fn_in_trait)]
trait Captcha: Send + Sync {
    async fn verify(&self, token: &str) -> anyhow::Result<bool>;
}

struct TurnstileCaptcha {
    http_client: reqwest::Client,
    secret_key: String,
}

#[derive(Deserialize)]
struct TurnstileVerifyResponse {
    success: bool,
}

impl Captcha for TurnstileCaptcha {
    async fn verify(&self, token: &str) -> anyhow::Result<bool> {
        let params = [("secret", self.secret_key.as_str()), ("response", token)];
        let response = self
            .http_client
            .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
            .form(&params)
            .send()
            .await?;
        let body: TurnstileVerifyResponse = response.json().await?;
        Ok(body.success)
    }
}

// ============================================================================
// Templates
// ============================================================================

/// Template for the unsubscribe confirmation page.
/// Success and error pages are static HTML served from S3.
#[derive(Template)]
#[template(path = "unsubscribe_confirm.html")]
struct UnsubscribeConfirmTemplate<'a> {
    email: &'a str,
    token: &'a str,
}

/// Request body for the subscribe endpoint.
#[derive(Debug, Deserialize)]
struct SubscribeRequest {
    email: String,
    strategy: String,
    #[serde(default)]
    website: String, // Honeypot field - should be empty
    #[serde(default)]
    turnstile_token: String,
}

// ============================================================================
// HTTP Response Helpers
// ============================================================================

fn html_response(status_code: u16, body: String) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(body))
        .expect("Failed to build response")
}

fn redirect(location: &str) -> Response<Body> {
    Response::builder()
        .status(303)
        .header("Location", location)
        .body(Body::Empty)
        .expect("Failed to build response")
}

fn text_response(status_code: u16, message: &str) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .expect("Failed to build response")
}

fn json_response(status_code: u16, body: &str) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(Body::from(body.to_string()))
        .expect("Failed to build response")
}

/// Extract request body as a string.
fn body_to_string(body: &Body) -> Option<String> {
    match body {
        Body::Text(s) => Some(s.clone()),
        Body::Binary(b) => std::str::from_utf8(b).ok().map(String::from),
        Body::Empty => None,
        // Body is #[non_exhaustive]; this arm guards against future variants.
        _ => None,
    }
}

// ============================================================================
// Lambda Handler
// ============================================================================

struct AppState<S, M, C> {
    storage: Arc<S>,
    mailer: Arc<M>,
    captcha: C,
    base_url: String,
}

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
    let reply_to_address = env::var("EMAIL_REPLY_TO")
        .map_err(|_| Error::from("EMAIL_REPLY_TO environment variable must be set"))?;
    let base_url = env::var("BASE_URL")
        .map_err(|_| Error::from("BASE_URL environment variable must be set"))?;
    let ses_configuration_set = env::var("SES_CONFIGURATION_SET")
        .map_err(|_| Error::from("SES_CONFIGURATION_SET environment variable must be set"))?;

    let mailer = SesMailer::new(
        ses_client,
        from_address,
        reply_to_address,
        ses_configuration_set,
    );

    let ssm_client = SsmClient::new(&config);
    let turnstile_secret_key_param = env::var("TURNSTILE_SECRET_KEY_PARAM")
        .map_err(|_| Error::from("TURNSTILE_SECRET_KEY_PARAM environment variable must be set"))?;
    let turnstile_secret_key = ssm_client
        .get_parameter()
        .name(&turnstile_secret_key_param)
        .with_decryption(true)
        .send()
        .await?
        .parameter()
        .and_then(|p| p.value.clone())
        .ok_or_else(|| {
            anyhow::format_err!(
                "SSM parameter value not found for name {}",
                turnstile_secret_key_param
            )
        })?;

    let storage = Arc::new(LambdaStorage::new(dynamodb_client, dynamodb_table));
    let state = Arc::new(AppState {
        storage,
        mailer: Arc::new(mailer),
        captcha: TurnstileCaptcha {
            http_client: reqwest::Client::new(),
            secret_key: turnstile_secret_key,
        },
        base_url,
    });

    run(service_fn(|event| handler(event, state.clone()))).await
}

async fn handler<S, M, C>(
    event: Request,
    state: Arc<AppState<S, M, C>>,
) -> Result<Response<Body>, Error>
where
    S: Storage,
    M: Mailer,
    C: Captcha,
{
    let method = event.method();
    let path = event.uri().path();

    info!(method = %method, path = %path, "Handling request");

    let query_params = event.query_string_parameters();
    let token = query_params.first("token").unwrap_or("");
    let email = query_params.first("email").unwrap_or("");

    match (method, path) {
        (&Method::POST, "/api/subscribe") => {
            let body_str = body_to_string(event.body()).unwrap_or_default();
            Ok(handle_subscribe_post(&state, &body_str).await)
        }
        (&Method::GET, "/api/verify") => Ok(handle_verify_get(&state, email, token).await),
        (&Method::GET, "/api/unsubscribe") => {
            let token = match Token::from_str(token) {
                Ok(t) => t,
                Err(_) => return Ok(redirect("/unsubscribe-error.html")),
            };
            Ok(handle_unsubscribe_get(&state.storage, &token).await)
        }
        (&Method::POST, "/api/unsubscribe") => {
            let token = match Token::from_str(token) {
                Ok(t) => t,
                Err(_) => return Ok(text_response(400, "Invalid token")),
            };
            let body_str = body_to_string(event.body());
            Ok(handle_unsubscribe_post(&state.storage, &token, body_str.as_deref()).await)
        }
        _ => Ok(text_response(404, "Not Found")),
    }
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET /api/unsubscribe?token=...
///
/// Shows confirmation page if token is valid, redirects to error page otherwise.
async fn handle_unsubscribe_get<S: Storage>(storage: &Arc<S>, token: &Token) -> Response<Body> {
    match storage.get_subscriber_by_unsubscribe_token(token).await {
        Ok(Some(subscriber)) => {
            let email_str = subscriber.email.to_string();
            let token_str = token.to_string();
            let template = UnsubscribeConfirmTemplate {
                email: &email_str,
                token: &token_str,
            };
            match template.render() {
                Ok(html) => html_response(200, html),
                Err(e) => {
                    error!(error = %e, "Failed to render template");
                    text_response(500, "Internal server error")
                }
            }
        }
        Ok(None) => redirect("/unsubscribe-error.html"),
        Err(e) => {
            error!(error = %e, "Error looking up subscriber");
            redirect("/unsubscribe-error.html")
        }
    }
}

/// POST /api/unsubscribe?token=...
///
/// Processes unsubscribe request. Handles both browser form submissions
/// (redirects to success/error page) and RFC 8058 one-click unsubscribe
/// (returns plain text response).
async fn handle_unsubscribe_post<S: Storage>(
    storage: &Arc<S>,
    token: &Token,
    body: Option<&str>,
) -> Response<Body> {
    // RFC 8058 one-click unsubscribe sends "List-Unsubscribe=One-Click" in body
    let is_one_click = body
        .map(|b| b.contains("List-Unsubscribe=One-Click"))
        .unwrap_or(false);

    if is_one_click {
        info!(token = %token, "Processing RFC 8058 one-click unsubscribe");
    }

    match unsubscribe::remove_subscriber(storage, token).await {
        Ok(true) => {
            if is_one_click {
                text_response(200, "Unsubscribed successfully")
            } else {
                redirect("/unsubscribe-success.html")
            }
        }
        Ok(false) => {
            if is_one_click {
                text_response(404, "Token not found")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
        Err(e) => {
            error!(error = %e, "Error removing subscriber");
            if is_one_click {
                text_response(500, "Internal server error")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
    }
}

fn subscribe_200_response() -> Response<Body> {
    json_response(
        200,
        r#"{"message": "Check your email to confirm your subscription"}"#,
    )
}

fn subscribe_500_response() -> Response<Body> {
    json_response(
        500,
        r#"{"error": "Internal server error, please try again later"}"#,
    )
}

/// POST /api/subscribe
///
/// Creates a pending subscription and sends a verification email.
/// Expects JSON body with `email` and `strategy` fields.
async fn handle_subscribe_post<S, M, C>(
    state: &Arc<AppState<S, M, C>>,
    body: &str,
) -> Response<Body>
where
    S: Storage,
    M: Mailer,
    C: Captcha,
{
    let request: SubscribeRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to parse subscribe request");
            return json_response(400, r#"{"error": "Invalid request body"}"#);
        }
    };

    // Check honeypot field
    if !request.website.is_empty() {
        info!("Honeypot field filled - rejecting bot submission");
        return subscribe_200_response();
    }

    // Parse strategy
    let strategy = match DigestStrategy::from_str(&request.strategy) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, strategy = %request.strategy, "Invalid strategy");
            return json_response(400, r#"{"error": "Invalid strategy"}"#);
        }
    };

    let email = match EmailAddress::from_str(&request.email) {
        Ok(e) => e,
        Err(_) => return json_response(400, r#"{"error": "Invalid email address"}"#),
    };

    // Verify Turnstile CAPTCHA token
    if request.turnstile_token.is_empty() {
        warn!("Missing Turnstile CAPTCHA token");
        return json_response(400, r#"{"error": "CAPTCHA verification required"}"#);
    }

    match state.captcha.verify(&request.turnstile_token).await {
        Ok(true) => {}
        Ok(false) => {
            warn!("CAPTCHA verification failed");
            return json_response(400, r#"{"error": "CAPTCHA verification failed"}"#);
        }
        Err(e) => {
            error!(error = %e, "Captcha API request failed");
            return subscribe_500_response();
        }
    }

    // Check if a verified subscriber already exists for this email
    let existing_subscriber = match state.storage.get_subscriber_by_email(&email).await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to check for existing subscriber");
            return subscribe_500_response();
        }
    };

    if let Some(existing) = existing_subscriber {
        match subscribe::update_subscription_strategy(&state.storage, existing, strategy).await {
            Ok(old_strategy) => {
                if let Err(e) = state
                    .mailer
                    .send_preference_update_email(
                        &email,
                        &old_strategy.description(),
                        &strategy.description(),
                    )
                    .await
                {
                    error!(error = %e, "Failed to send preference update email");
                    return subscribe_500_response();
                }
                return subscribe_200_response();
            }
            Err(e) => {
                error!(error = %e, "Failed to update existing subscription");
                return subscribe_500_response();
            }
        }
    }

    // No existing verified subscriber - create pending subscription and send verification email
    let pending =
        match subscribe::create_pending_subscription(&state.storage, &email, strategy).await {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "Failed to create pending subscription");
                return subscribe_500_response();
            }
        };

    let email_str = email.to_string().to_lowercase();
    let verify_url = format!(
        "{}/api/verify?email={}&token={}",
        state.base_url,
        urlencoding::encode(&email_str),
        pending.token
    );
    if let Err(e) = state
        .mailer
        .send_verification_email(&pending.email, &verify_url, &strategy.description())
        .await
    {
        error!(error = %e, email = %pending.email, "Failed to send verification email");
        return subscribe_500_response();
    }

    info!(email = %pending.email, strategy = %strategy, "Verification email sent");
    subscribe_200_response()
}

/// GET /api/verify?email=...&token=...
///
/// Verifies a pending subscription and creates the subscriber.
async fn handle_verify_get<S, M, C>(
    state: &Arc<AppState<S, M, C>>,
    maybe_email: &str,
    maybe_token: &str,
) -> Response<Body>
where
    S: Storage,
    M: Mailer,
    C: Captcha,
{
    let email = match EmailAddress::from_str(maybe_email) {
        Ok(e) => e,
        Err(_) => return redirect("/verify-error.html"),
    };
    let token = match Token::from_str(maybe_token) {
        Ok(t) => t,
        Err(_) => return redirect("/verify-error.html"),
    };

    match subscribe::verify_subscription(&state.storage, &email, &token).await {
        Ok(Some(_subscriber)) => {
            // Note: this log line is used for a custom metric
            info!(email = %email, "Subscription verified successfully");
            redirect("/verify-success.html")
        }
        Ok(None) => {
            warn!(email = %email, "Verification failed: not found, expired, or invalid token");
            redirect("/verify-error.html")
        }
        Err(e) => {
            error!(error = %e, email = %email, "Error verifying subscription");
            redirect("/verify-error.html")
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use email_address::EmailAddress;
    use hndigest::storage::Storage;
    use hndigest::strategies::DigestStrategy;
    use hndigest::types::{PendingSubscription, Post, Subscriber, Token};
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Mutex;

    // -----------------------------------------------------------------------
    // FakeStorage
    // -----------------------------------------------------------------------

    #[derive(Default)]
    struct FakeStorage {
        subscribers: Mutex<HashMap<String, Subscriber>>,
        pending: Mutex<HashMap<String, PendingSubscription>>,
    }

    impl FakeStorage {
        fn new() -> Self {
            Self::default()
        }

        fn with_subscriber(self, s: Subscriber) -> Self {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s);
            self
        }

        fn with_pending(self, p: PendingSubscription) -> Self {
            self.pending
                .lock()
                .unwrap()
                .insert(p.email.to_string().to_lowercase(), p);
            self
        }

        fn get_subscriber(&self, email: &str) -> Option<Subscriber> {
            self.subscribers
                .lock()
                .unwrap()
                .get(&email.to_lowercase())
                .cloned()
        }

        fn get_pending(&self, email: &str) -> Option<PendingSubscription> {
            self.pending
                .lock()
                .unwrap()
                .get(&email.to_lowercase())
                .cloned()
        }
    }

    impl Storage for FakeStorage {
        async fn snapshot_posts(
            &self,
            _: &HashMap<String, Post>,
            _: DateTime<Utc>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn save_digest(&self, _: &str, _: DateTime<Utc>, _: &[Post]) -> anyhow::Result<()> {
            Ok(())
        }
        async fn fetch_digest(
            &self,
            _: &str,
            _: DateTime<Utc>,
        ) -> anyhow::Result<Option<Vec<Post>>> {
            Ok(None)
        }
        async fn get_subscriber_by_unsubscribe_token(
            &self,
            token: &Token,
        ) -> anyhow::Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .values()
                .find(|s| s.unsubscribe_token == *token)
                .cloned())
        }
        async fn get_all_subscribers(&self) -> anyhow::Result<Vec<Subscriber>> {
            Ok(self.subscribers.lock().unwrap().values().cloned().collect())
        }
        async fn upsert_subscriber(&self, s: &Subscriber) -> anyhow::Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .insert(s.email.to_string().to_lowercase(), s.clone());
            Ok(())
        }
        async fn remove_subscriber(&self, email: &EmailAddress) -> anyhow::Result<()> {
            self.subscribers
                .lock()
                .unwrap()
                .remove(&email.to_string().to_lowercase());
            Ok(())
        }
        async fn upsert_pending_subscription(&self, p: &PendingSubscription) -> anyhow::Result<()> {
            self.pending
                .lock()
                .unwrap()
                .insert(p.email.to_string().to_lowercase(), p.clone());
            Ok(())
        }
        async fn get_pending_subscription(
            &self,
            email: &EmailAddress,
        ) -> anyhow::Result<Option<PendingSubscription>> {
            Ok(self
                .pending
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }
        async fn get_subscriber_by_email(
            &self,
            email: &EmailAddress,
        ) -> anyhow::Result<Option<Subscriber>> {
            Ok(self
                .subscribers
                .lock()
                .unwrap()
                .get(&email.to_string().to_lowercase())
                .cloned())
        }
    }

    // -----------------------------------------------------------------------
    // SpyMailer
    // -----------------------------------------------------------------------

    #[derive(Default)]
    struct SpyMailer {
        emails: Mutex<Vec<(String, String)>>, // (recipient, subject)
    }

    impl SpyMailer {
        fn new() -> Self {
            Self::default()
        }

        fn email_count(&self) -> usize {
            self.emails.lock().unwrap().len()
        }

        fn sent_subjects(&self) -> Vec<String> {
            self.emails
                .lock()
                .unwrap()
                .iter()
                .map(|(_, s)| s.clone())
                .collect()
        }
    }

    impl Mailer for SpyMailer {
        async fn send_email(
            &self,
            recipient: &EmailAddress,
            subject: &str,
            _html: &str,
            _text: &str,
            _extra_headers: &[(&str, &str)],
        ) -> anyhow::Result<()> {
            self.emails
                .lock()
                .unwrap()
                .push((recipient.to_string(), subject.to_string()));
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // FakeCaptcha
    // -----------------------------------------------------------------------

    struct FakeCaptcha {
        success: bool,
    }

    impl FakeCaptcha {
        fn pass() -> Self {
            Self { success: true }
        }

        fn fail() -> Self {
            Self { success: false }
        }
    }

    impl Captcha for FakeCaptcha {
        async fn verify(&self, _token: &str) -> anyhow::Result<bool> {
            Ok(self.success)
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_state(
        storage: FakeStorage,
        mailer: SpyMailer,
        captcha: FakeCaptcha,
    ) -> Arc<AppState<FakeStorage, SpyMailer, FakeCaptcha>> {
        Arc::new(AppState {
            storage: Arc::new(storage),
            mailer: Arc::new(mailer),
            captcha,
            base_url: "https://example.com".to_string(),
        })
    }

    fn subscribe_body(email: &str, strategy: &str, turnstile: &str) -> String {
        serde_json::json!({
            "email": email,
            "strategy": strategy,
            "turnstile_token": turnstile,
        })
        .to_string()
    }

    fn status(r: &Response<Body>) -> u16 {
        r.status().as_u16()
    }

    fn location(r: &Response<Body>) -> &str {
        r.headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    // -----------------------------------------------------------------------
    // POST /api/subscribe
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn subscribe_honeypot_filled_returns_200_no_side_effects() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::pass());
        let body = serde_json::json!({
            "email": "bot@example.com",
            "strategy": "TOP_N#10",
            "turnstile_token": "tok",
            "website": "http://spam.com",
        })
        .to_string();

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 200);
        assert_eq!(state.mailer.email_count(), 0);
        assert!(state.storage.get_pending("bot@example.com").is_none());
    }

    #[tokio::test]
    async fn subscribe_invalid_email_returns_400() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::pass());
        let body = subscribe_body("not-an-email", "TOP_N#10", "tok");

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_invalid_strategy_returns_400() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::pass());
        let body = subscribe_body("user@example.com", "BOGUS_STRATEGY", "tok");

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_captcha_fails_returns_400() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::fail());
        let body = subscribe_body("user@example.com", "TOP_N#10", "bad-tok");

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_new_user_stores_pending_and_sends_verification_email() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::pass());
        let body = subscribe_body("new@example.com", "TOP_N#10", "valid-tok");

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 200);
        assert!(state.storage.get_pending("new@example.com").is_some());
        assert_eq!(state.mailer.email_count(), 1);
        assert!(
            state
                .mailer
                .sent_subjects()
                .contains(&"Confirm your Hacker Digest subscription".to_string())
        );
    }

    #[tokio::test]
    async fn subscribe_existing_subscriber_updates_strategy_and_sends_preference_email() {
        let existing = Subscriber::new(
            EmailAddress::from_str("existing@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let storage = FakeStorage::new().with_subscriber(existing);
        let state = make_state(storage, SpyMailer::new(), FakeCaptcha::pass());
        let body = subscribe_body("existing@example.com", "TOP_N#50", "valid-tok");

        let resp = handle_subscribe_post(&state, &body).await;

        assert_eq!(status(&resp), 200);
        let updated = state
            .storage
            .get_subscriber("existing@example.com")
            .unwrap();
        assert_eq!(updated.strategy, DigestStrategy::TopN(50));
        assert_eq!(state.mailer.email_count(), 1);
        assert!(
            state
                .mailer
                .sent_subjects()
                .contains(&"Your Hacker Digest preferences have been updated".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // GET /api/verify
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn verify_valid_token_creates_subscriber_and_redirects_to_success() {
        let pending = PendingSubscription::new(
            EmailAddress::from_str("verify@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let token_str = pending.token.to_string();
        let storage = FakeStorage::new().with_pending(pending);
        let state = make_state(storage, SpyMailer::new(), FakeCaptcha::pass());

        let resp = handle_verify_get(&state, "verify@example.com", &token_str).await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/verify-success.html");
        assert!(state.storage.get_subscriber("verify@example.com").is_some());
    }

    #[tokio::test]
    async fn verify_wrong_token_redirects_to_error() {
        let pending = PendingSubscription::new(
            EmailAddress::from_str("verify@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let storage = FakeStorage::new().with_pending(pending);
        let state = make_state(storage, SpyMailer::new(), FakeCaptcha::pass());

        let resp = handle_verify_get(&state, "verify@example.com", "wrong-token").await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/verify-error.html");
        assert!(state.storage.get_subscriber("verify@example.com").is_none());
    }

    #[tokio::test]
    async fn verify_invalid_email_redirects_to_error() {
        let state = make_state(FakeStorage::new(), SpyMailer::new(), FakeCaptcha::pass());

        let resp = handle_verify_get(&state, "not-an-email", "some-token").await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/verify-error.html");
    }

    // -----------------------------------------------------------------------
    // GET /api/unsubscribe
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn unsubscribe_get_valid_token_returns_confirmation_page() {
        let sub = Subscriber::new(
            EmailAddress::from_str("unsub@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let token = sub.unsubscribe_token.clone();
        let storage = Arc::new(FakeStorage::new().with_subscriber(sub));

        let resp = handle_unsubscribe_get(&storage, &token).await;

        assert_eq!(status(&resp), 200);
        // Response body should be HTML containing the email address
        if let Body::Text(html) = resp.into_body() {
            assert!(html.contains("unsub@example.com"));
        } else {
            panic!("Expected text body");
        }
    }

    #[tokio::test]
    async fn unsubscribe_get_unknown_token_redirects_to_error() {
        let storage = Arc::new(FakeStorage::new());
        let token: Token = "unknown".parse().unwrap();

        let resp = handle_unsubscribe_get(&storage, &token).await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/unsubscribe-error.html");
    }

    // -----------------------------------------------------------------------
    // POST /api/unsubscribe
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn unsubscribe_post_browser_valid_token_redirects_to_success() {
        let sub = Subscriber::new(
            EmailAddress::from_str("unsub@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let token = sub.unsubscribe_token.clone();
        let storage = Arc::new(FakeStorage::new().with_subscriber(sub));

        let resp = handle_unsubscribe_post(&storage, &token, None).await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/unsubscribe-success.html");
        assert!(storage.get_subscriber("unsub@example.com").is_none());
    }

    #[tokio::test]
    async fn unsubscribe_post_one_click_returns_200_text() {
        let sub = Subscriber::new(
            EmailAddress::from_str("oneclick@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let token = sub.unsubscribe_token.clone();
        let storage = Arc::new(FakeStorage::new().with_subscriber(sub));

        let resp =
            handle_unsubscribe_post(&storage, &token, Some("List-Unsubscribe=One-Click")).await;

        assert_eq!(status(&resp), 200);
        assert!(storage.get_subscriber("oneclick@example.com").is_none());
    }

    #[tokio::test]
    async fn unsubscribe_post_unknown_token_browser_redirects_to_error() {
        let storage = Arc::new(FakeStorage::new());
        let token: Token = "bogus".parse().unwrap();

        let resp = handle_unsubscribe_post(&storage, &token, None).await;

        assert_eq!(status(&resp), 303);
        assert_eq!(location(&resp), "/unsubscribe-error.html");
    }

    #[tokio::test]
    async fn unsubscribe_post_unknown_token_one_click_returns_404() {
        let storage = Arc::new(FakeStorage::new());
        let token: Token = "bogus".parse().unwrap();

        let resp =
            handle_unsubscribe_post(&storage, &token, Some("List-Unsubscribe=One-Click")).await;

        assert_eq!(status(&resp), 404);
    }
}
