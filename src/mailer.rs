use anyhow::{Context, Result};
use askama::Template;
use aws_sdk_sesv2::Client;
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message, MessageHeader};
use email_address::EmailAddress;
use tracing::info;

// ============================================================================
// Email templates (used by default Mailer method implementations)
// ============================================================================

#[derive(Template)]
#[template(path = "verification.html")]
struct VerificationEmailHtmlTemplate<'a> {
    verify_url: &'a str,
    strategy_description: &'a str,
}

#[derive(Template)]
#[template(path = "verification.txt")]
struct VerificationEmailTextTemplate<'a> {
    verify_url: &'a str,
    strategy_description: &'a str,
}

#[derive(Template)]
#[template(path = "preference_update.html")]
struct PreferenceUpdateEmailHtmlTemplate<'a> {
    old_strategy_description: &'a str,
    new_strategy_description: &'a str,
}

#[derive(Template)]
#[template(path = "preference_update.txt")]
struct PreferenceUpdateEmailTextTemplate<'a> {
    old_strategy_description: &'a str,
    new_strategy_description: &'a str,
}

// ============================================================================
// Mailer trait
// ============================================================================

/// Core email-sending primitive. Implementations handle transport concerns
/// (SES request construction, credentials, etc.).
///
/// Extra headers are passed as `(name, value)` string pairs so that
/// implementations are not coupled to AWS SDK types.
#[allow(async_fn_in_trait)]
pub trait Mailer: Send + Sync {
    async fn send_email(
        &self,
        recipient: &EmailAddress,
        subject: &str,
        html_content: &str,
        text_content: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<()>;

    /// Send a subscription verification email.
    /// Template rendering happens here; only `send_email` is transport-coupled.
    async fn send_verification_email(
        &self,
        recipient: &EmailAddress,
        verify_url: &str,
        strategy_description: &str,
    ) -> Result<()> {
        let html = VerificationEmailHtmlTemplate {
            verify_url,
            strategy_description,
        }
        .render()
        .context("Failed to render HTML template")?;
        let text = VerificationEmailTextTemplate {
            verify_url,
            strategy_description,
        }
        .render()
        .context("Failed to render text template")?;
        self.send_email(
            recipient,
            "Confirm your Hacker Digest subscription",
            &html,
            &text,
            &[],
        )
        .await
    }

    /// Send a preference update notification email.
    async fn send_preference_update_email(
        &self,
        recipient: &EmailAddress,
        old_strategy_description: &str,
        new_strategy_description: &str,
    ) -> Result<()> {
        let html = PreferenceUpdateEmailHtmlTemplate {
            old_strategy_description,
            new_strategy_description,
        }
        .render()
        .context("Failed to render HTML template")?;
        let text = PreferenceUpdateEmailTextTemplate {
            old_strategy_description,
            new_strategy_description,
        }
        .render()
        .context("Failed to render text template")?;
        self.send_email(
            recipient,
            "Your Hacker Digest preferences have been updated",
            &html,
            &text,
            &[],
        )
        .await
    }

    /// Send a digest email with RFC 8058 List-Unsubscribe headers.
    ///
    /// The caller is responsible for rendering `html_content` and `text_content`
    /// (typically from Askama templates in the digest lambda handler).
    async fn send_digest(
        &self,
        subject: &str,
        html_content: &str,
        text_content: &str,
        recipient: &EmailAddress,
        unsubscribe_url: &str,
    ) -> Result<()> {
        let list_unsub_value = format!("<{}>", unsubscribe_url);
        let extra_headers: &[(&str, &str)] = &[
            ("List-Unsubscribe", &list_unsub_value),
            ("List-Unsubscribe-Post", "List-Unsubscribe=One-Click"),
        ];
        self.send_email(
            recipient,
            subject,
            html_content,
            text_content,
            extra_headers,
        )
        .await
    }
}

// ============================================================================
// SesMailer — AWS SES v2 implementation
// ============================================================================

pub struct SesMailer {
    ses_client: Client,
    from_address: String,
    reply_to_address: String,
    configuration_set_name: String,
}

impl SesMailer {
    pub fn new(
        ses_client: Client,
        from_address: String,
        reply_to_address: String,
        configuration_set_name: String,
    ) -> Self {
        Self {
            ses_client,
            from_address,
            reply_to_address,
            configuration_set_name,
        }
    }
}

impl Mailer for SesMailer {
    async fn send_email(
        &self,
        recipient: &EmailAddress,
        subject: &str,
        html_content: &str,
        text_content: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<()> {
        let subject_content = Content::builder().data(subject).charset("UTF-8").build()?;
        let html_body = Content::builder()
            .data(html_content)
            .charset("UTF-8")
            .build()?;
        let text_body = Content::builder()
            .data(text_content)
            .charset("UTF-8")
            .build()?;

        let body = Body::builder().html(html_body).text(text_body).build();

        let mut message_builder = Message::builder().subject(subject_content).body(body);
        for (name, value) in extra_headers {
            let header = MessageHeader::builder().name(*name).value(*value).build()?;
            message_builder = message_builder.headers(header);
        }
        let message = message_builder.build();

        let destination = Destination::builder()
            .to_addresses(recipient.to_string())
            .build();
        let email_content = EmailContent::builder().simple(message).build();

        let response = self
            .ses_client
            .send_email()
            .from_email_address(&self.from_address)
            .reply_to_addresses(&self.reply_to_address)
            .destination(destination)
            .content(email_content)
            .configuration_set_name(&self.configuration_set_name)
            .send()
            .await
            .context(format!("Failed to send email to {}", recipient))?;

        info!(
            message_id = ?response.message_id(),
            recipient = %recipient,
            "Email sent"
        );

        Ok(())
    }
}
