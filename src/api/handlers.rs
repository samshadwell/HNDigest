use super::{ApiResponse, AppState};
use crate::captcha::Captcha;
use crate::mailer::Mailer;
use crate::storage::Storage;
use crate::strategies::DigestStrategy;
use crate::types::Token;
use crate::{subscribe, unsubscribe};
use askama::Template;
use email_address::EmailAddress;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// Templates
// ============================================================================

#[derive(Template)]
#[template(path = "unsubscribe_confirm.html")]
struct UnsubscribeConfirmTemplate<'a> {
    email: &'a str,
    token: &'a str,
}

// ============================================================================
// Request body
// ============================================================================

#[derive(Debug, Deserialize)]
struct SubscribeRequest {
    email: String,
    strategy: String,
    #[serde(default)]
    website: String, // Honeypot field — should be empty
    #[serde(default)]
    turnstile_token: String,
}

// ============================================================================
// Response helpers
// ============================================================================

fn json(status: u16, body: &str) -> ApiResponse {
    ApiResponse::Json {
        status,
        body: body.to_string(),
    }
}

fn text(status: u16, body: &str) -> ApiResponse {
    ApiResponse::Text {
        status,
        body: body.to_string(),
    }
}

fn redirect(location: &str) -> ApiResponse {
    ApiResponse::Redirect(location.to_string())
}

fn subscribe_200() -> ApiResponse {
    json(
        200,
        r#"{"message": "Check your email to confirm your subscription"}"#,
    )
}

fn subscribe_500() -> ApiResponse {
    json(
        500,
        r#"{"error": "Internal server error, please try again later"}"#,
    )
}

// ============================================================================
// Route handlers
// ============================================================================

/// GET /api/unsubscribe?token=...
///
/// Shows the confirmation page if the token is valid, redirects to error otherwise.
pub(super) async fn unsubscribe_get<S: Storage>(storage: &Arc<S>, token_str: &str) -> ApiResponse {
    let token = match Token::from_str(token_str) {
        Ok(t) => t,
        Err(_) => return redirect("/unsubscribe-error.html"),
    };

    match storage.get_subscriber_by_unsubscribe_token(&token).await {
        Ok(Some(subscriber)) => {
            let email_str = subscriber.email.to_string();
            let token_str = token.to_string();
            match (UnsubscribeConfirmTemplate {
                email: &email_str,
                token: &token_str,
            })
            .render()
            {
                Ok(html) => ApiResponse::Html(html),
                Err(e) => {
                    error!(error = %e, "Failed to render unsubscribe template");
                    text(500, "Internal server error")
                }
            }
        }
        Ok(None) => redirect("/unsubscribe-error.html"),
        Err(e) => {
            error!(error = %e, "Error looking up subscriber by token");
            redirect("/unsubscribe-error.html")
        }
    }
}

/// POST /api/unsubscribe?token=...
///
/// Handles both browser form submissions (redirects) and RFC 8058 one-click
/// unsubscribe requests (plain-text response).
pub(super) async fn unsubscribe_post<S: Storage>(
    storage: &Arc<S>,
    token_str: &str,
    body: Option<&str>,
) -> ApiResponse {
    let token = match Token::from_str(token_str) {
        Ok(t) => t,
        Err(_) => return text(400, "Invalid token"),
    };

    let is_one_click = body
        .map(|b| b.contains("List-Unsubscribe=One-Click"))
        .unwrap_or(false);

    if is_one_click {
        info!(token = %token, "Processing RFC 8058 one-click unsubscribe");
    }

    match unsubscribe::remove_subscriber(storage, &token).await {
        Ok(true) => {
            if is_one_click {
                text(200, "Unsubscribed successfully")
            } else {
                redirect("/unsubscribe-success.html")
            }
        }
        Ok(false) => {
            if is_one_click {
                text(404, "Token not found")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
        Err(e) => {
            error!(error = %e, "Error removing subscriber");
            if is_one_click {
                text(500, "Internal server error")
            } else {
                redirect("/unsubscribe-error.html")
            }
        }
    }
}

/// POST /api/subscribe
///
/// Creates a pending subscription and sends a verification email.
pub(super) async fn subscribe_post<S, M, C>(
    state: &Arc<AppState<S, M, C>>,
    body: &str,
) -> ApiResponse
where
    S: Storage,
    M: Mailer,
    C: Captcha,
{
    let req: SubscribeRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to parse subscribe request");
            return json(400, r#"{"error": "Invalid request body"}"#);
        }
    };

    if !req.website.is_empty() {
        info!("Honeypot field filled — rejecting bot submission");
        return subscribe_200();
    }

    let strategy = match DigestStrategy::from_str(&req.strategy) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, strategy = %req.strategy, "Invalid strategy");
            return json(400, r#"{"error": "Invalid strategy"}"#);
        }
    };

    let email = match EmailAddress::from_str(&req.email) {
        Ok(e) => e,
        Err(_) => return json(400, r#"{"error": "Invalid email address"}"#),
    };

    if req.turnstile_token.is_empty() {
        warn!("Missing Turnstile CAPTCHA token");
        return json(400, r#"{"error": "CAPTCHA verification required"}"#);
    }

    match state.captcha.verify(&req.turnstile_token).await {
        Ok(true) => {}
        Ok(false) => {
            warn!("CAPTCHA verification failed");
            return json(400, r#"{"error": "CAPTCHA verification failed"}"#);
        }
        Err(e) => {
            error!(error = %e, "Captcha API request failed");
            return subscribe_500();
        }
    }

    match state.storage.get_subscriber_by_email(&email).await {
        Err(e) => {
            error!(error = %e, "Failed to check for existing subscriber");
            return subscribe_500();
        }
        Ok(Some(existing)) => {
            match subscribe::update_subscription_strategy(&state.storage, existing, strategy).await
            {
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
                        return subscribe_500();
                    }
                    return subscribe_200();
                }
                Err(e) => {
                    error!(error = %e, "Failed to update existing subscription");
                    return subscribe_500();
                }
            }
        }
        Ok(None) => {}
    }

    let pending =
        match subscribe::create_pending_subscription(&state.storage, &email, strategy).await {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "Failed to create pending subscription");
                return subscribe_500();
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
        return subscribe_500();
    }

    info!(email = %pending.email, strategy = %strategy, "Verification email sent");
    subscribe_200()
}

/// GET /api/verify?email=...&token=...
///
/// Verifies a pending subscription and activates the subscriber.
pub(super) async fn verify_get<S, M, C>(
    state: &Arc<AppState<S, M, C>>,
    maybe_email: &str,
    maybe_token: &str,
) -> ApiResponse
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
        Ok(Some(_)) => {
            // Note: this log line drives a custom CloudWatch metric
            info!(email = %email, "Subscription verified successfully");
            redirect("/verify-success.html")
        }
        Ok(None) => {
            warn!(email = %email, "Verification failed: not found, expired, or token mismatch");
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
    use crate::api::AppState;
    use crate::captcha::test_utils::FakeCaptcha;
    use crate::mailer::test_utils::SpyMailer;
    use crate::storage::test_utils::InMemoryStorage;
    use crate::strategies::DigestStrategy;
    use crate::types::{PendingSubscription, Subscriber};
    use email_address::EmailAddress;
    use std::str::FromStr;

    fn make_state(
        storage: InMemoryStorage,
        mailer: SpyMailer,
        captcha: FakeCaptcha,
    ) -> Arc<AppState<InMemoryStorage, SpyMailer, FakeCaptcha>> {
        Arc::new(AppState::new(
            Arc::new(storage),
            Arc::new(mailer),
            captcha,
            "https://example.com".to_string(),
        ))
    }

    fn subscribe_body(email: &str, strategy: &str, turnstile: &str) -> String {
        serde_json::json!({
            "email": email,
            "strategy": strategy,
            "turnstile_token": turnstile,
        })
        .to_string()
    }

    // -----------------------------------------------------------------------
    // POST /api/subscribe
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn subscribe_honeypot_filled_returns_200_no_side_effects() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let body = serde_json::json!({
            "email": "bot@example.com",
            "strategy": "TOP_N#10",
            "turnstile_token": "tok",
            "website": "http://spam.com",
        })
        .to_string();

        let resp = subscribe_post(&state, &body).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(state.mailer.email_count(), 0);
        assert!(state.storage.get_pending("bot@example.com").is_none());
    }

    #[tokio::test]
    async fn subscribe_invalid_email_returns_400() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let resp = subscribe_post(&state, &subscribe_body("not-an-email", "TOP_N#10", "tok")).await;
        assert_eq!(resp.status(), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_invalid_strategy_returns_400() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let resp =
            subscribe_post(&state, &subscribe_body("user@example.com", "BOGUS", "tok")).await;
        assert_eq!(resp.status(), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_captcha_fails_returns_400() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::fail(),
        );
        let resp = subscribe_post(
            &state,
            &subscribe_body("user@example.com", "TOP_N#10", "bad"),
        )
        .await;
        assert_eq!(resp.status(), 400);
        assert_eq!(state.mailer.email_count(), 0);
    }

    #[tokio::test]
    async fn subscribe_new_user_stores_pending_and_sends_verification_email() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let resp = subscribe_post(
            &state,
            &subscribe_body("new@example.com", "TOP_N#10", "tok"),
        )
        .await;
        assert_eq!(resp.status(), 200);
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
        let state = make_state(
            InMemoryStorage::new().with_subscriber(existing),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let resp = subscribe_post(
            &state,
            &subscribe_body("existing@example.com", "TOP_N#50", "tok"),
        )
        .await;
        assert_eq!(resp.status(), 200);
        assert_eq!(
            state
                .storage
                .get_subscriber("existing@example.com")
                .unwrap()
                .strategy,
            DigestStrategy::TopN(50)
        );
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
        let state = make_state(
            InMemoryStorage::new().with_pending(pending),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );

        let resp = verify_get(&state, "verify@example.com", &token_str).await;

        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/verify-success.html"));
        assert!(state.storage.get_subscriber("verify@example.com").is_some());
    }

    #[tokio::test]
    async fn verify_wrong_token_redirects_to_error() {
        let pending = PendingSubscription::new(
            EmailAddress::from_str("verify@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let state = make_state(
            InMemoryStorage::new().with_pending(pending),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );

        let resp = verify_get(&state, "verify@example.com", "wrong-token").await;

        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/verify-error.html"));
        assert!(state.storage.get_subscriber("verify@example.com").is_none());
    }

    #[tokio::test]
    async fn verify_invalid_email_redirects_to_error() {
        let state = make_state(
            InMemoryStorage::new(),
            SpyMailer::new(),
            FakeCaptcha::pass(),
        );
        let resp = verify_get(&state, "not-an-email", "some-token").await;
        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/verify-error.html"));
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
        let token = sub.unsubscribe_token.to_string();
        let storage = Arc::new(InMemoryStorage::new().with_subscriber(sub));

        let resp = unsubscribe_get(&storage, &token).await;

        assert_eq!(resp.status(), 200);
        assert!(resp.body_contains("unsub@example.com"));
    }

    #[tokio::test]
    async fn unsubscribe_get_unknown_token_redirects_to_error() {
        let storage = Arc::new(InMemoryStorage::new());
        let resp = unsubscribe_get(&storage, "unknown-token").await;
        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/unsubscribe-error.html"));
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
        let token = sub.unsubscribe_token.to_string();
        let storage = Arc::new(InMemoryStorage::new().with_subscriber(sub));

        let resp = unsubscribe_post(&storage, &token, None).await;

        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/unsubscribe-success.html"));
        assert!(!storage.has_subscriber("unsub@example.com"));
    }

    #[tokio::test]
    async fn unsubscribe_post_one_click_returns_200_text() {
        let sub = Subscriber::new(
            EmailAddress::from_str("oneclick@example.com").unwrap(),
            DigestStrategy::TopN(10),
        );
        let token = sub.unsubscribe_token.to_string();
        let storage = Arc::new(InMemoryStorage::new().with_subscriber(sub));

        let resp = unsubscribe_post(&storage, &token, Some("List-Unsubscribe=One-Click")).await;

        assert_eq!(resp.status(), 200);
        assert!(!storage.has_subscriber("oneclick@example.com"));
    }

    #[tokio::test]
    async fn unsubscribe_post_unknown_token_browser_redirects_to_error() {
        let storage = Arc::new(InMemoryStorage::new());
        let resp = unsubscribe_post(&storage, "bogus", None).await;
        assert_eq!(resp.status(), 303);
        assert_eq!(resp.redirect_location(), Some("/unsubscribe-error.html"));
    }

    #[tokio::test]
    async fn unsubscribe_post_unknown_token_one_click_returns_404() {
        let storage = Arc::new(InMemoryStorage::new());
        let resp = unsubscribe_post(&storage, "bogus", Some("List-Unsubscribe=One-Click")).await;
        assert_eq!(resp.status(), 404);
    }
}
