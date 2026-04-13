use thiserror::Error;

#[derive(Debug, Error)]
pub enum MailError {
    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("SMTP error: {0}")]
    Smtp(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Mailbox not found: {0}")]
    MailboxNotFound(String),

    #[error("Message not found: uid={0}")]
    MessageNotFound(u32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl From<lettre::transport::smtp::Error> for MailError {
    fn from(e: lettre::transport::smtp::Error) -> Self {
        Self::Smtp(e.to_string())
    }
}

impl From<lettre::error::Error> for MailError {
    fn from(e: lettre::error::Error) -> Self {
        Self::Smtp(e.to_string())
    }
}
