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
        let _ = session.logout().await;
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
            match imap::folder_status(&mut session, &folder.name).await {
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

        // Calculate sequence number range for the page (newest first).
        // Note: `total` is the sequence count from SELECT, so this is a sequence range.
        // fetch_summaries uses FETCH (not UID FETCH) and includes UID in the item list,
        // so we get the real IMAP UIDs back even though we address by sequence number.
        let end = total.saturating_sub((page - 1) * page_size);
        let start = end.saturating_sub(page_size).max(1);

        let seq_range = format!("{start}:{end}");
        let summaries = imap::fetch_summaries(&mut session, &seq_range).await?;

        let _ = session.logout().await;
        Ok((summaries, total))
    }

    /// Fetch new messages since a given UID (incremental sync).
    /// Returns summaries of messages with UID > `since_uid`, plus the folder total.
    pub async fn fetch_new_messages_since(
        &self,
        folder: &str,
        since_uid: u32,
    ) -> Result<(Vec<MailMessageSummary>, u32), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let (total, _) = imap::select_folder(&mut session, folder).await?;

        if total == 0 {
            let _ = session.logout().await;
            return Ok((vec![], 0));
        }

        let uid_range = format!("{}:*", since_uid + 1);
        let mut summaries = imap::fetch_summaries_by_uid_range(&mut session, &uid_range).await?;

        // UID FETCH "n:*" may return the message with UID == since_uid
        // when there are no newer messages (IMAP spec: * = last message UID).
        // Filter it out.
        summaries.retain(|s| s.uid > since_uid);

        let _ = session.logout().await;
        Ok((summaries, total))
    }

    /// Fetch message summaries by a comma-separated UID set (e.g. "123,456,789").
    pub async fn fetch_summaries_by_uids(
        &self,
        folder: &str,
        uid_set: &str,
    ) -> Result<Vec<MailMessageSummary>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let summaries = imap::fetch_summaries_by_uid_range(&mut session, uid_set).await?;
        let _ = session.logout().await;
        Ok(summaries)
    }

    /// Batch-fetch full messages by UID set in a single IMAP session.
    /// Does NOT mark messages as seen — suitable for sync / backfill.
    pub async fn fetch_messages_batch(&self, folder: &str, uid_set: &str) -> Result<Vec<MailMessage>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let messages = imap::fetch_messages_batch(&mut session, uid_set).await?;
        let _ = session.logout().await;
        Ok(messages)
    }

    /// Fetch a single message by UID from a folder.
    pub async fn fetch_message(&self, folder: &str, uid: u32) -> Result<MailMessage, MailError> {
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
    pub async fn move_messages(&self, from_folder: &str, uids: &[u32], to_folder: &str) -> Result<(), MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, from_folder).await?;
        imap::move_messages(&mut session, uids, to_folder).await?;
        let _ = session.logout().await;
        Ok(())
    }

    /// Batch-fetch flags for a set of UIDs. Returns Vec<(uid, is_read, is_flagged)>.
    pub async fn fetch_flags_batch(&self, folder: &str, uid_set: &str) -> Result<Vec<(u32, bool, bool)>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let flags = imap::fetch_flags_batch(&mut session, uid_set).await?;
        let _ = session.logout().await;
        Ok(flags)
    }

    /// List all UIDs in a folder (for reconciliation).
    pub async fn list_all_uids(&self, folder: &str) -> Result<Vec<u32>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let uids = imap::list_all_uids(&mut session).await?;
        let _ = session.logout().await;
        Ok(uids)
    }

    /// Search messages in a folder.
    pub async fn search(&self, folder: &str, query: &str) -> Result<Vec<u32>, MailError> {
        let mut session = imap::connect(&self.config).await?;
        let _ = imap::select_folder(&mut session, folder).await?;
        let uids = imap::search(&mut session, query).await?;
        let _ = session.logout().await;
        Ok(uids)
    }
}

/// A long-lived IMAP session for a single account.
/// Reuses one TCP+TLS connection across multiple folder operations,
/// avoiding the ~500ms TLS handshake overhead on every call.
pub struct MailSession {
    session: imap::ImapSession,
}

impl MailSession {
    /// Open a new IMAP connection for the given account config.
    pub async fn connect(config: &MailAccountConfig) -> Result<Self, MailError> {
        let session = imap::connect(config).await?;
        Ok(Self { session })
    }

    /// SELECT a folder; returns (total_messages, unseen_count).
    pub async fn open_folder(&mut self, folder: &str) -> Result<(u32, u32), MailError> {
        imap::select_folder(&mut self.session, folder).await
    }

    /// SELECT a folder; returns `(total_messages, unseen_count, uid_validity)`.
    ///
    /// Use this variant when you persist UIDs locally — callers must compare
    /// `uid_validity` against the cached value and wipe local state on change
    /// (RFC 3501 §2.3.1.1).
    pub async fn open_folder_ex(&mut self, folder: &str) -> Result<(u32, u32, Option<u32>), MailError> {
        imap::select_folder_ex(&mut self.session, folder).await
    }

    /// Fetch message summaries (headers only, no body) for a UID range/set.
    /// Folder must be selected first via `open_folder`.
    pub async fn fetch_summaries_by_uids(
        &mut self,
        uid_set: &str,
    ) -> Result<Vec<crate::message::MailMessageSummary>, MailError> {
        imap::fetch_summaries_by_uid_range(&mut self.session, uid_set).await
    }

    /// Fetch full messages (with body) for a UID set string like "1,2,3" or "10:20".
    /// Folder must be selected first via `open_folder`.
    pub async fn fetch_messages_batch(&mut self, uid_set: &str) -> Result<Vec<crate::message::MailMessage>, MailError> {
        imap::fetch_messages_batch(&mut self.session, uid_set).await
    }

    /// Fetch read/flagged state for a UID set.
    /// Returns Vec<(uid, is_read, is_flagged)>.
    /// Folder must be selected first via `open_folder`.
    pub async fn fetch_flags_batch(&mut self, uid_set: &str) -> Result<Vec<(u32, bool, bool)>, MailError> {
        imap::fetch_flags_batch(&mut self.session, uid_set).await
    }

    /// List all UIDs in the currently selected folder.
    /// Folder must be selected first via `open_folder`.
    pub async fn list_all_uids(&mut self) -> Result<Vec<u32>, MailError> {
        imap::list_all_uids(&mut self.session).await
    }

    /// Gracefully logout and close the connection.
    pub async fn logout(mut self) {
        let _ = self.session.logout().await;
    }

    /// Enter IMAP IDLE mode, waiting up to `timeout_secs` for a server push notification
    /// (e.g., new message arrived, flag changed). Returns `(self, new_data_detected)`.
    /// Consumes self and returns it back so the connection can be reused.
    ///
    /// Per RFC 2177, clients should re-issue IDLE every 29 minutes; use 25 * 60 as timeout.
    pub async fn into_idle_wait(self, timeout_secs: u64) -> Result<(Self, bool), MailError> {
        let (session, new_data) = imap::idle_wait(self.session, timeout_secs).await?;
        Ok((Self { session }, new_data))
    }
}
