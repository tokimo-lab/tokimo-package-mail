use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MessageBuilder, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{MailAccountConfig, SecurityMode};
use crate::error::MailError;
use crate::message::ComposeMessage;

/// Send an email via SMTP.
pub async fn send_message(
    cfg: &MailAccountConfig,
    compose: &ComposeMessage,
) -> Result<(), MailError> {
    let from_mailbox: Mailbox = if let Some(ref name) = cfg.sender_name {
        format!("{name} <{}>", cfg.email)
    } else {
        cfg.email.clone()
    }
    .parse()
    .map_err(|e| MailError::Smtp(format!("invalid From address: {e}")))?;

    let mut builder: MessageBuilder = Message::builder().from(from_mailbox);

    for addr in &compose.to {
        let to: Mailbox = addr
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid To address '{addr}': {e}")))?;
        builder = builder.to(to);
    }
    for addr in &compose.cc {
        let cc: Mailbox = addr
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid Cc address '{addr}': {e}")))?;
        builder = builder.cc(cc);
    }
    for addr in &compose.bcc {
        let bcc: Mailbox = addr
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid Bcc address '{addr}': {e}")))?;
        builder = builder.bcc(bcc);
    }

    builder = builder.subject(&compose.subject);

    if let Some(ref reply_to) = compose.in_reply_to {
        builder = builder.in_reply_to(reply_to.clone());
    }
    for r in &compose.references {
        builder = builder.references(r.clone());
    }

    // Build message body.
    let message = if compose.attachments.is_empty() {
        // Simple text or HTML message.
        if let Some(ref html) = compose.html_body {
            builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(
                            SinglePart::builder()
                                .content_type(ContentType::TEXT_PLAIN)
                                .body(
                                    compose
                                        .text_body
                                        .clone()
                                        .unwrap_or_default(),
                                ),
                        )
                        .singlepart(
                            SinglePart::builder()
                                .content_type(ContentType::TEXT_HTML)
                                .body(html.clone()),
                        ),
                )
                .map_err(|e| MailError::Smtp(format!("build message: {e}")))?
        } else {
            builder
                .body(compose.text_body.clone().unwrap_or_default())
                .map_err(|e| MailError::Smtp(format!("build message: {e}")))?
        }
    } else {
        // Message with attachments.
        let text_part = if let Some(ref html) = compose.html_body {
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .content_type(ContentType::TEXT_PLAIN)
                        .body(compose.text_body.clone().unwrap_or_default()),
                )
                .singlepart(
                    SinglePart::builder()
                        .content_type(ContentType::TEXT_HTML)
                        .body(html.clone()),
                )
        } else {
            MultiPart::alternative().singlepart(
                SinglePart::builder()
                    .content_type(ContentType::TEXT_PLAIN)
                    .body(compose.text_body.clone().unwrap_or_default()),
            )
        };

        let mut mixed = MultiPart::mixed().multipart(text_part);

        for att in &compose.attachments {
            let data = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &att.data,
            )
            .map_err(|e| MailError::Smtp(format!("decode attachment: {e}")))?;

            let content_type: ContentType = att
                .content_type
                .parse()
                .unwrap_or(ContentType::TEXT_PLAIN);

            let attachment =
                Attachment::new(att.filename.clone()).body(data, content_type);
            mixed = mixed.singlepart(attachment);
        }

        builder
            .multipart(mixed)
            .map_err(|e| MailError::Smtp(format!("build message: {e}")))?
    };

    // Build SMTP transport.
    let creds = Credentials::new(cfg.smtp_username.clone(), cfg.smtp_password.clone());

    let transport = match cfg.smtp_security {
        SecurityMode::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.smtp_host)
            .map_err(|e| MailError::Smtp(format!("SMTP relay: {e}")))?
            .port(cfg.smtp_port)
            .credentials(creds)
            .build(),
        SecurityMode::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)
                .map_err(|e| MailError::Smtp(format!("SMTP starttls: {e}")))?
                .port(cfg.smtp_port)
                .credentials(creds)
                .build()
        }
        SecurityMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            &cfg.smtp_host,
        )
        .port(cfg.smtp_port)
        .credentials(creds)
        .build(),
    };

    transport
        .send(message)
        .await
        .map_err(|e| MailError::Smtp(format!("send: {e}")))?;

    Ok(())
}
