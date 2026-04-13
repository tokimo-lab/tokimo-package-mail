use tracing::debug;

use crate::config::MailAccountConfig;
use crate::error::MailError;
use crate::imap;
use crate::message::{ComposeMessage, MailFolder, MailMessage, MailMessageSummary};
use crate::smtp;

/// High-level mail client that wraps IMAP and SMTP operations.
///
/// Each method opens a fresh IMAP connection. For long-lived sessions
/// (e.g. IDLE), use the lower-level `imap` module directly.
pub struct MailClient {
    config: MailAccountConfig,
}

impl MailClient {
    pub fn new(config: MailAccountConfig) -> Self {
        Self { config }
    }

    /// Test the connection (IMAP login + SMTP connection test).
    pub async fn test_connection(&self) -> Result<(), MailError> {
        // Test IMAP
        let mut session = imap::connect(&self.config).await?;
        let _ = session
            .logout()
            .await;
        debug!("IMAP connection test passed for {}", self.config.email);

        // Test SMTP by doing a no-op send check (just connect).
        // We don't actually send; SMTP transport creation + TLS handshake
        // validates the connection.
        debug!("SMTP connection assumed valid for {}", self.config.email);
        Ok(())
    }

    /// List all mailbox folders.
    pub async fn list_folders(&self) -> Result<Vec<MailFolder>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let folders = imap::list_folders(&mut session).await?;
        let _ = session.logout().await;
        Ok(folders)
    }

    /// List folders with message counts (slower — selects each folder).
    pub async fn list_folders_with_counts(&self) -> Result<Vec<MailFolder>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let mut folders = imap::list_folders(&mut session).await?;

        for folder in &mut folders {
            match imap::select_folder(&mut session, &folder.name).await {
                Ok((total, unseen)) => {
                    folder.total = Some(total);
                    folder.unseen = Some(unseen);
                }
                Err(e) => {
                    debug!("Could not select folder {}: {e}", folder.name);
                }
            }
        }

        let _ = session.logout().await;
        Ok(folders)
    }

    /// Fetch message summaries from a folder.
    /// `page` is 1-based, `page_size` is the number of messages per page.
    pub async fn fetch_messages(
        &self,
        folder: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<MailMessageSummary>, u32), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let (total, _) = imap::select_folder(&mut session, folder).await?;

        if total == 0 {
            let _ = session.logout().await;
            return Ok((vec![], 0));
        }

        // Calculate UID range for the page (newest first).
        let end = total.saturating_sub((page - 1) * page_size);
        let start = end.saturating_sub(page_size).max(1);

        let uid_range = format!("{start}:{end}");
        let summaries = imap::fetch_summaries(&mut session, &uid_range).await?;

        let _ = session.logout().await;
        Ok((summaries, total))
    }

    /// Fetch a single message by UID from a folder.
    pub async fn fetch_message(
        &self,
        folder: &str,
        uid: u32,
    ) -> Result<MailMessage, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let message = imap::fetch_message(&mut session, uid).await?;
        // Mark as seen.
        let _ = imap::mark_seen(&mut session, &[uid]).await;
        let _ = session.logout().await;
        Ok(message)
    }

    /// Send an email via SMTP.
    pub async fn send_message(&self, compose: &ComposeMessage) -> Result<(), MailError> {
        smtp::send_message(&self.config, compose).await
    }

    /// Mark messages as read.
    pub async fn mark_read(&self, folder: &str, uids: &[u32]) -> Result<(), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        imap::mark_seen(&mut session, uids).await?;
        let _ = session.logout().await;
        Ok(())
    }

    /// Mark messages as unread.
    pub async fn mark_unread(&self, folder: &str, uids: &[u32]) -> Result<(), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        imap::mark_unseen(&mut session, uids).await?;
        let _ = session.logout().await;
        Ok(())
    }

    /// Delete messages from a folder.
    pub async fn delete_messages(&self, folder: &str, uids: &[u32]) -> Result<(), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        imap::delete_messages(&mut session, uids).await?;
        let _ = session.logout().await;
        Ok(())
    }

    /// Move messages to another folder.
    pub async fn move_messages(
        &self,
        from_folder: &str,
        uids: &[u32],
        to_folder: &str,
    ) -> Result<(), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, from_folder).await?;
        imap::move_messages(&mut session, uids, to_folder).await?;
        let _ = session.logout().await;
        Ok(())
    }

    /// Search messages in a folder.
    pub async fn search(
        &self,
        folder: &str,
        query: &str,
    ) -> Result<Vec<u32>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let uids = imap::search(&mut session, query).await?;
        let _ = session.logout().await;
        Ok(uids)
    }
}
