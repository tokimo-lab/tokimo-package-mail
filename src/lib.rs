pub mod client;
pub mod config;
pub mod error;
pub mod imap;
pub mod message;
pub mod provider;
pub mod smtp;

pub use client::MailClient;
pub use config::MailAccountConfig;
pub use error::MailError;
pub use message::{MailAddress, MailAttachment, MailMessage, MailMessageSummary};
pub use provider::{MailProvider, ProviderPreset};
