pub mod client;
pub mod config;
pub mod error;
pub mod imap;
pub mod message;
pub mod provider;
pub mod smtp;

pub use client::MailClient;
pub use client::MailSession;
pub use config::MailAccountConfig;
pub use error::MailError;
pub use message::{MailAddress, MailAttachment, MailFolder, MailMessage, MailMessageSummary};
pub use provider::{MailProvider, ProviderPreset};

/// Decode an IMAP modified UTF-7 mailbox name to UTF-8.
/// Plain ASCII names pass through unchanged.
pub fn decode_mailbox_name(raw: &str) -> String {
    utf7_imap::decode_utf7_imap(raw.to_string())
}
