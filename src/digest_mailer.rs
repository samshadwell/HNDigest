use aws_sdk_sesv2::Client;
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message, MessageHeader};

use anyhow::{Context, Result};
use tracing::info;

pub struct DigestMailer {
    ses_client: Client,
    from_address: String,
    reply_to_address: String,
}

impl DigestMailer {
    pub fn new(ses_client: Client, from_address: String, reply_to_address: String) -> Self {
        Self {
            ses_client,
            from_address,
            reply_to_address,
        }
    }

    /// Send an email to a single recipient with RFC 8058 unsubscribe headers.
    pub async fn send_mail(
        &self,
        subject: &str,
        html_content: &str,
        text_content: &str,
        recipient: &str,
        unsubscribe_url: &str,
    ) -> Result<()> {
        // Build RFC 8058 List-Unsubscribe headers
        let list_unsubscribe_header = MessageHeader::builder()
            .name("List-Unsubscribe")
            .value(format!("<{}>", unsubscribe_url))
            .build()?;

        let list_unsubscribe_post_header = MessageHeader::builder()
            .name("List-Unsubscribe-Post")
            .value("List-Unsubscribe=One-Click")
            .build()?;

        // Build email content
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
        let message = Message::builder()
            .subject(subject_content)
            .body(body)
            .headers(list_unsubscribe_header)
            .headers(list_unsubscribe_post_header)
            .build();

        let destination = Destination::builder().to_addresses(recipient).build();

        let email_content = EmailContent::builder().simple(message).build();

        let response = self
            .ses_client
            .send_email()
            .from_email_address(&self.from_address)
            .reply_to_addresses(&self.reply_to_address)
            .destination(destination)
            .content(email_content)
            .send()
            .await
            .context(format!("Failed to send email to {}", recipient))?;

        info!(
            message_id = ?response.message_id(),
            recipient = recipient,
            "Email sent"
        );

        Ok(())
    }
}
