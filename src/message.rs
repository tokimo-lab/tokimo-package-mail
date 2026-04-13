use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// A parsed email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailAddress {
    pub name: Option<String>,
    pub address: String,
}

impl std::fmt::Display for MailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref name) = self.name {
            write!(f, "{name} <{}>", self.address)
        } else {
            write!(f, "{}", self.address)
        }
    }
}

/// Email attachment metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailAttachment {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    /// Base64-encoded content (only populated when fetching full message).
    pub data: Option<String>,
}

/// Summary of a message (for list view — no body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessageSummary {
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: String,
    pub from: Vec<MailAddress>,
    pub to: Vec<MailAddress>,
    pub date: Option<DateTime<FixedOffset>>,
    pub flags: Vec<String>,
    pub has_attachments: bool,
    pub preview: String,
    /// Size in bytes (from IMAP RFC822.SIZE).
    pub size: u32,
}

/// Full parsed email message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessage {
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: String,
    pub from: Vec<MailAddress>,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub reply_to: Vec<MailAddress>,
    pub date: Option<DateTime<FixedOffset>>,
    pub flags: Vec<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,

    /// Plain-text body.
    pub text_body: Option<String>,
    /// HTML body.
    pub html_body: Option<String>,

    pub attachments: Vec<MailAttachment>,
    pub size: u32,
}

/// An IMAP mailbox / folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailFolder {
    pub name: String,
    pub delimiter: Option<String>,
    /// Attributes like \Sent, \Drafts, \Trash, \Junk, etc.
    pub attributes: Vec<String>,
    pub total: Option<u32>,
    pub unseen: Option<u32>,
}

/// Parameters for composing a new email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeMessage {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub attachments: Vec<ComposeAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeAttachment {
    pub filename: String,
    pub content_type: String,
    /// Base64-encoded file data.
    pub data: String,
}
