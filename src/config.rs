use serde::{Deserialize, Serialize};

/// IMAP/SMTP connection security mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    /// Direct TLS connection (port 993 for IMAP, 465 for SMTP).
    Tls,
    /// STARTTLS upgrade after plaintext connection (port 143/587).
    StartTls,
    /// No encryption (not recommended, port 143/25).
    None,
}

/// Full configuration for a mail account (IMAP + SMTP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailAccountConfig {
    /// Display name for the account (e.g. "Work Email").
    pub display_name: String,
    /// Email address (used as default From).
    pub email: String,

    // ── IMAP (reading) ──
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SecurityMode,
    pub imap_username: String,
    pub imap_password: String,

    // ── SMTP (sending) ──
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SecurityMode,
    pub smtp_username: String,
    pub smtp_password: String,

    /// Optional: sender display name in From header.
    pub sender_name: Option<String>,
}

impl MailAccountConfig {
    /// Quick validation that required fields are non-empty.
    pub fn validate(&self) -> Result<(), String> {
        if self.email.is_empty() {
            return Err("Email address is required".into());
        }
        if self.imap_host.is_empty() {
            return Err("IMAP host is required".into());
        }
        if self.smtp_host.is_empty() {
            return Err("SMTP host is required".into());
        }
        if self.imap_username.is_empty() {
            return Err("IMAP username is required".into());
        }
        if self.smtp_username.is_empty() {
            return Err("SMTP username is required".into());
        }
        Ok(())
    }
}
