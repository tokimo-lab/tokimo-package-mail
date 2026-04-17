use std::sync::Arc;

use async_imap::types::{Fetch, Flag, Name};
use futures::TryStreamExt;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use tracing::{debug, warn};

use crate::config::{MailAccountConfig, SecurityMode};
use crate::error::MailError;
use crate::message::{MailAddress, MailAttachment, MailFolder, MailMessage, MailMessageSummary};

pub(crate) type ImapSession = async_imap::Session<Compat<tokio_rustls::client::TlsStream<TcpStream>>>;

/// Build a rustls `ClientConfig` that trusts webpki root certificates.
fn tls_config() -> Arc<rustls::ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("TLS protocol versions")
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Arc::new(config)
}

/// Connect to an IMAP server using the given config and return an
/// authenticated session.
pub async fn connect(cfg: &MailAccountConfig) -> Result<ImapSession, MailError> {
    let tls_cfg = tls_config();
    let connector = tokio_rustls::TlsConnector::from(tls_cfg);

    let addr = format!("{}:{}", cfg.imap_host, cfg.imap_port);
    let tcp = TcpStream::connect(&addr)
        .await
        .map_err(|e| MailError::Connection(format!("TCP connect to {addr}: {e}")))?;

    let server_name: rustls_pki_types::ServerName<'_> = cfg
        .imap_host
        .clone()
        .try_into()
        .map_err(|e| MailError::Tls(format!("invalid server name: {e}")))?;

    match cfg.imap_security {
        SecurityMode::Tls | SecurityMode::StartTls | SecurityMode::None => {
            // Direct TLS (implicit TLS).
            // For STARTTLS we'd need to upgrade; async-imap doesn't natively
            // support it. We use direct TLS for all modes — most servers accept it.
            let tls_stream = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| MailError::Tls(format!("TLS handshake: {e}")))?;
            // Wrap with compat layer so tokio-rustls (tokio traits) satisfies
            // the futures::AsyncRead/AsyncWrite bounds expected by async-imap.
            let compat_stream = tls_stream.compat();
            let client = async_imap::Client::new(compat_stream);
            let session = client
                .login(&cfg.imap_username, &cfg.imap_password)
                .await
                .map_err(|e| MailError::Auth(format!("IMAP login: {}", e.0)))?;
            Ok(session)
        }
    }
}

/// List all mailbox folders.
pub async fn list_folders(session: &mut ImapSession) -> Result<Vec<MailFolder>, MailError> {
    let names: Vec<Name> = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|e| MailError::Imap(format!("LIST: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("LIST stream: {e}")))?;

    let mut folders = Vec::with_capacity(names.len());
    for n in &names {
        let attrs: Vec<String> = n.attributes().iter().map(|a| format!("{a:?}")).collect();
        folders.push(MailFolder {
            name: n.name().to_string(),
            delimiter: n.delimiter().map(std::string::ToString::to_string),
            attributes: attrs,
            total: None,
            unseen: None,
        });
    }
    Ok(folders)
}

/// Query folder counts via STATUS (returns actual unseen **count**, unlike
/// SELECT whose UNSEEN is the sequence-number of the first unseen message).
pub async fn folder_status(session: &mut ImapSession, folder: &str) -> Result<(u32, u32), MailError> {
    let mailbox = session
        .status(folder, "(MESSAGES UNSEEN)")
        .await
        .map_err(|e| MailError::Imap(format!("STATUS {folder}: {e}")))?;
    let total = mailbox.exists;
    let unseen = mailbox.unseen.unwrap_or(0);
    Ok((total, unseen))
}

/// List all UIDs in the currently selected mailbox.
pub async fn list_all_uids(session: &mut ImapSession) -> Result<Vec<u32>, MailError> {
    let result = session
        .uid_search("ALL")
        .await
        .map_err(|e| MailError::Imap(format!("UID SEARCH ALL: {e}")))?;
    Ok(result.into_iter().collect())
}

/// Select a mailbox and return (total, unseen).
pub async fn select_folder(session: &mut ImapSession, folder: &str) -> Result<(u32, u32), MailError> {
    let (total, unseen, _) = select_folder_ex(session, folder).await?;
    Ok((total, unseen))
}

/// Select a mailbox and return (total, unseen, `uid_validity`).
///
/// RFC 3501 §2.3.1.1: `UIDVALIDITY` changes if the server's UID space is reset
/// (mailbox deleted + recreated, Dovecot rebuild, etc.). Callers that persist
/// UIDs locally MUST purge their cache and resync when this value changes.
pub async fn select_folder_ex(
    session: &mut ImapSession,
    folder: &str,
) -> Result<(u32, u32, Option<u32>), MailError> {
    let mailbox = session
        .select(folder)
        .await
        .map_err(|e| MailError::Imap(format!("SELECT '{folder}': {e}")))?;
    let total = mailbox.exists;
    let unseen = mailbox.unseen.unwrap_or(0);
    let uid_validity = mailbox.uid_validity;
    Ok((total, unseen, uid_validity))
}

/// Fetch message summaries (headers only) for a sequence number range.
/// `seq_range` is an IMAP sequence set like "1:50" or "100:200" (sequence numbers, not UIDs).
/// The `UID` item is included in the fetch so we get the real UID of each message.
///
/// Uses ENVELOPE (server-cached metadata) which is ~3x faster than BODY.PEEK[HEADER.FIELDS].
pub async fn fetch_summaries(session: &mut ImapSession, seq_range: &str) -> Result<Vec<MailMessageSummary>, MailError> {
    let fetches: Vec<Fetch> = session
        .fetch(seq_range, "(UID FLAGS RFC822.SIZE INTERNALDATE ENVELOPE)")
        .await
        .map_err(|e| MailError::Imap(format!("FETCH headers: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("FETCH stream: {e}")))?;

    let mut summaries = Vec::with_capacity(fetches.len());
    for fetch in &fetches {
        summaries.push(build_summary_from_fetch(fetch));
    }

    summaries.sort_by(|a, b| b.uid.cmp(&a.uid));
    Ok(summaries)
}

/// Fetch message summaries by UID range (e.g. "64853:*").
/// Used for incremental sync — fetch only messages newer than a known UID.
///
/// Resilience strategy (some servers like `new.mail.taobao.com` emit
/// malformed ENVELOPE responses with unescaped quotes inside Message-ID,
/// which poisons the entire IMAP stream on the first bad response):
///
/// 1. Try ENVELOPE (fast path — server-cached metadata, ~3x faster).
/// 2. On stream/parse failure, retry using `BODY.PEEK[HEADER.FIELDS (...)]`
///    which uses length-prefixed IMAP literals and is immune to
///    quoted-string escaping bugs.
/// 3. If the header-fields fetch also fails on a plain `low:high` range,
///    bisect and recurse — at size=1 log the poisoned UID and skip it.
pub async fn fetch_summaries_by_uid_range(
    session: &mut ImapSession,
    uid_range: &str,
) -> Result<Vec<MailMessageSummary>, MailError> {
    match fetch_summaries_envelope(session, uid_range).await {
        Ok(summaries) => Ok(summaries),
        Err(e) => {
            warn!("ENVELOPE fetch failed for '{uid_range}': {e}; retrying with HEADER.FIELDS");
            fetch_summaries_header_fields_resilient(session, uid_range).await
        }
    }
}

async fn fetch_summaries_envelope(
    session: &mut ImapSession,
    uid_range: &str,
) -> Result<Vec<MailMessageSummary>, MailError> {
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_range, "(UID FLAGS RFC822.SIZE INTERNALDATE ENVELOPE)")
        .await
        .map_err(|e| MailError::Imap(format!("UID FETCH headers: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("UID FETCH stream: {e}")))?;

    let mut summaries = Vec::with_capacity(fetches.len());
    for fetch in &fetches {
        summaries.push(build_summary_from_fetch(fetch));
    }

    summaries.sort_by(|a, b| b.uid.cmp(&a.uid));
    Ok(summaries)
}

/// UID FETCH using HEADER.FIELDS literal (binary-safe). Bisects on failure
/// when the range is a plain `low:high`.
fn fetch_summaries_header_fields_resilient<'a>(
    session: &'a mut ImapSession,
    uid_range: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<MailMessageSummary>, MailError>> + Send + 'a>> {
    Box::pin(async move {
        match fetch_summaries_header_fields(session, uid_range).await {
            Ok(summaries) => Ok(summaries),
            Err(e) => {
                // Try to bisect plain numeric ranges "low:high". Anything else
                // (wildcards, comma lists) we can't safely split — propagate.
                let Some((low, high)) = parse_plain_range(uid_range) else {
                    return Err(e);
                };
                if low >= high {
                    warn!("Skipping poisoned UID {low} in folder: {e}");
                    return Ok(Vec::new());
                }
                let mid = low + (high - low) / 2;
                let left = format!("{low}:{mid}");
                let right = format!("{}:{high}", mid + 1);
                warn!("HEADER.FIELDS fetch failed for '{uid_range}': {e}; bisecting into '{left}' + '{right}'");
                let mut out = fetch_summaries_header_fields_resilient(session, &left).await?;
                let mut right_out = fetch_summaries_header_fields_resilient(session, &right).await?;
                out.append(&mut right_out);
                out.sort_by(|a, b| b.uid.cmp(&a.uid));
                Ok(out)
            }
        }
    })
}

async fn fetch_summaries_header_fields(
    session: &mut ImapSession,
    uid_range: &str,
) -> Result<Vec<MailMessageSummary>, MailError> {
    // HEADER.FIELDS is returned as a length-prefixed literal — immune to
    // quoted-string escaping bugs in the server's ENVELOPE output.
    let query = "(UID FLAGS RFC822.SIZE INTERNALDATE \
                 BODY.PEEK[HEADER.FIELDS (SUBJECT FROM TO DATE MESSAGE-ID CONTENT-TYPE)])";
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_range, query)
        .await
        .map_err(|e| MailError::Imap(format!("UID FETCH header-fields: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("UID FETCH header-fields stream: {e}")))?;

    let mut summaries = Vec::with_capacity(fetches.len());
    for fetch in &fetches {
        summaries.push(build_summary_from_header_fields(fetch));
    }

    summaries.sort_by(|a, b| b.uid.cmp(&a.uid));
    Ok(summaries)
}

/// Parse a plain "low:high" numeric IMAP range. Returns None for wildcards
/// (`n:*`) or comma-separated sets — those can't be safely bisected.
fn parse_plain_range(range: &str) -> Option<(u32, u32)> {
    if range.contains(',') || range.contains('*') {
        return None;
    }
    let (l, h) = range.split_once(':')?;
    let low: u32 = l.trim().parse().ok()?;
    let high: u32 = h.trim().parse().ok()?;
    Some((low, high))
}

fn build_summary_from_header_fields(fetch: &Fetch) -> MailMessageSummary {
    let uid = fetch.uid.unwrap_or(0);
    let flags = extract_flags(fetch);
    let size = fetch.size.unwrap_or(0);
    let header_bytes = fetch
        .header()
        .or_else(|| fetch.body().or(fetch.text()))
        .unwrap_or_default();
    let (subject, from, to, date, message_id, has_attachments) = parse_header_fields(header_bytes);
    let date = date.or_else(|| fetch.internal_date());

    MailMessageSummary {
        uid,
        message_id,
        subject,
        from,
        to,
        date,
        flags,
        has_attachments,
        preview: String::new(),
        size,
    }
}

/// Fetch a single complete message by UID.
/// Uses BODY.PEEK[] to avoid implicitly marking the message as \Seen.
pub async fn fetch_message(session: &mut ImapSession, uid: u32) -> Result<MailMessage, MailError> {
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid.to_string(), "(UID FLAGS RFC822.SIZE INTERNALDATE BODY.PEEK[])")
        .await
        .map_err(|e| MailError::Imap(format!("FETCH message: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("FETCH stream: {e}")))?;

    let fetch = fetches.first().ok_or(MailError::MessageNotFound(uid))?;
    let flags = extract_flags(fetch);
    let size = fetch.size.unwrap_or(0);

    let raw = fetch.body().unwrap_or_default();
    let parsed = mailparse::parse_mail(raw).map_err(|e| MailError::Parse(format!("parse mail: {e}")))?;

    let mut msg = build_full_message(uid, &parsed, flags, size);
    if msg.date.is_none() {
        msg.date = fetch.internal_date();
    }
    Ok(msg)
}

/// Batch-fetch only UID + FLAGS for a set of UIDs.
/// Returns Vec<(uid, is_read, is_flagged)>.
pub async fn fetch_flags_batch(session: &mut ImapSession, uid_set: &str) -> Result<Vec<(u32, bool, bool)>, MailError> {
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_set, "(UID FLAGS)")
        .await
        .map_err(|e| MailError::Imap(format!("FETCH flags: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("FETCH flags stream: {e}")))?;

    let mut result = Vec::with_capacity(fetches.len());
    for fetch in &fetches {
        let uid = fetch.uid.unwrap_or(0);
        let flags = extract_flags(fetch);
        let is_read = flags.iter().any(|f| f == "\\Seen");
        let is_flagged = flags.iter().any(|f| f == "\\Flagged");
        result.push((uid, is_read, is_flagged));
    }
    Ok(result)
}

/// Batch-fetch full messages by UID set (e.g. "123,456,789" or "100:200").
/// Uses BODY.PEEK[] to avoid implicitly marking messages as \Seen.
pub async fn fetch_messages_batch(session: &mut ImapSession, uid_set: &str) -> Result<Vec<MailMessage>, MailError> {
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_set, "(UID FLAGS RFC822.SIZE INTERNALDATE BODY.PEEK[])")
        .await
        .map_err(|e| MailError::Imap(format!("FETCH batch: {e}")))?
        .try_collect()
        .await
        .map_err(|e| MailError::Imap(format!("FETCH batch stream: {e}")))?;

    let mut messages = Vec::with_capacity(fetches.len());
    for fetch in &fetches {
        let uid = fetch.uid.unwrap_or(0);
        let flags = extract_flags(fetch);
        let size = fetch.size.unwrap_or(0);
        let raw = fetch.body().unwrap_or_default();
        match mailparse::parse_mail(raw) {
            Ok(parsed) => {
                let mut msg = build_full_message(uid, &parsed, flags, size);
                if msg.date.is_none() {
                    msg.date = fetch.internal_date();
                }
                messages.push(msg);
            }
            Err(e) => {
                tracing::warn!("Failed to parse message uid={uid}: {e}");
            }
        }
    }
    Ok(messages)
}

/// Mark messages as seen/read.
pub async fn mark_seen(session: &mut ImapSession, uids: &[u32]) -> Result<(), MailError> {
    let uid_set = uids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    session
        .uid_store(&uid_set, "+FLAGS (\\Seen)")
        .await
        .map_err(|e| MailError::Imap(format!("STORE +Seen: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("STORE stream: {e}")))?;
    Ok(())
}

/// Mark messages as unseen/unread.
pub async fn mark_unseen(session: &mut ImapSession, uids: &[u32]) -> Result<(), MailError> {
    let uid_set = uids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    session
        .uid_store(&uid_set, "-FLAGS (\\Seen)")
        .await
        .map_err(|e| MailError::Imap(format!("STORE -Seen: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("STORE stream: {e}")))?;
    Ok(())
}

/// Flag messages for deletion and expunge.
pub async fn delete_messages(session: &mut ImapSession, uids: &[u32]) -> Result<(), MailError> {
    let uid_set = uids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    session
        .uid_store(&uid_set, "+FLAGS (\\Deleted)")
        .await
        .map_err(|e| MailError::Imap(format!("STORE +Deleted: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("STORE stream: {e}")))?;
    session
        .expunge()
        .await
        .map_err(|e| MailError::Imap(format!("EXPUNGE: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("EXPUNGE stream: {e}")))?;
    Ok(())
}

/// Move messages to another folder (using IMAP MOVE or COPY+DELETE).
pub async fn move_messages(session: &mut ImapSession, uids: &[u32], target_folder: &str) -> Result<(), MailError> {
    let uid_set = uids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");

    // Use COPY + DELETE as a fallback (MOVE extension not universally supported).
    session
        .uid_copy(&uid_set, target_folder)
        .await
        .map_err(|e| MailError::Imap(format!("COPY to {target_folder}: {e}")))?;
    session
        .uid_store(&uid_set, "+FLAGS (\\Deleted)")
        .await
        .map_err(|e| MailError::Imap(format!("STORE +Deleted: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("STORE stream: {e}")))?;
    session
        .expunge()
        .await
        .map_err(|e| MailError::Imap(format!("EXPUNGE: {e}")))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| MailError::Imap(format!("EXPUNGE stream: {e}")))?;
    Ok(())
}

/// IMAP IDLE — wait for server push notifications.
/// Returns the session back along with whether new mail was detected.
/// `timeout` is in seconds; 0 means wait indefinitely (up to server limit).
pub async fn idle_wait(session: ImapSession, timeout_secs: u64) -> Result<(ImapSession, bool), MailError> {
    let mut idle = session.idle();
    idle.init()
        .await
        .map_err(|e| MailError::Imap(format!("IDLE init: {e}")))?;

    let duration = if timeout_secs > 0 {
        std::time::Duration::from_secs(timeout_secs)
    } else {
        // RFC recommends re-issuing IDLE every 29 minutes.
        std::time::Duration::from_secs(29 * 60)
    };

    let (idle_wait, _interrupt) = idle.wait_with_timeout(duration);
    let response = idle_wait
        .await
        .map_err(|e| MailError::Imap(format!("IDLE wait: {e}")))?;

    let has_new_data = matches!(response, async_imap::extensions::idle::IdleResponse::NewData(_));
    debug!("IDLE completed: {response:?}");

    // Call done() on the handle to recover the session.
    // Note: we need to call done() on the Handle, but after wait_with_timeout
    // borrowed it, the Handle is still alive. However, wait_with_timeout borrows
    // &mut self so we can't call done() directly. We need to restructure.
    // Actually, the idle handle was moved into this scope — just call done().
    let session = idle
        .done()
        .await
        .map_err(|e| MailError::Imap(format!("IDLE done: {e}")))?;
    Ok((session, has_new_data))
}

/// Search messages by query string (IMAP SEARCH).
pub async fn search(session: &mut ImapSession, query: &str) -> Result<Vec<u32>, MailError> {
    // Build IMAP search criteria — search in subject, from, body.
    let criteria = format!("OR OR SUBJECT \"{query}\" FROM \"{query}\" BODY \"{query}\"");
    let result = session
        .uid_search(&criteria)
        .await
        .map_err(|e| MailError::Imap(format!("SEARCH: {e}")))?;
    let uids: Vec<u32> = result.into_iter().collect();
    Ok(uids)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn extract_flags(fetch: &Fetch) -> Vec<String> {
    fetch
        .flags()
        .map(|f| match f {
            Flag::Seen => "\\Seen".to_string(),
            Flag::Answered => "\\Answered".to_string(),
            Flag::Flagged => "\\Flagged".to_string(),
            Flag::Deleted => "\\Deleted".to_string(),
            Flag::Draft => "\\Draft".to_string(),
            Flag::Recent => "\\Recent".to_string(),
            Flag::MayCreate => "\\MayCreate".to_string(),
            Flag::Custom(cow) => cow.to_string(),
        })
        .collect()
}

/// Build a `MailMessageSummary` from an ENVELOPE FETCH response.
/// ENVELOPE is server-cached metadata (~3x faster than BODY.PEEK[HEADER.FIELDS]).
/// `has_attachments` is always false here — corrected during body fetch.
fn build_summary_from_fetch(fetch: &Fetch) -> MailMessageSummary {
    let uid = fetch.uid.unwrap_or(0);
    let flags = extract_flags(fetch);
    let size = fetch.size.unwrap_or(0);

    let (subject, from, to, date, message_id) = if let Some(env) = fetch.envelope() {
        let subject = env.subject.as_deref().map(decode_envelope_bytes).unwrap_or_default();
        let from = parse_envelope_addresses(env.from.as_ref());
        let to = parse_envelope_addresses(env.to.as_ref());
        let date = env
            .date
            .as_deref()
            .and_then(|d| {
                let s = String::from_utf8_lossy(d);
                mailparse::dateparse(&s)
                    .ok()
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.fixed_offset()))
            })
            .or_else(|| fetch.internal_date());
        let message_id = env
            .message_id
            .as_deref()
            .map(|m| String::from_utf8_lossy(m).into_owned());
        (subject, from, to, date, message_id)
    } else {
        // Fallback: parse raw header bytes if ENVELOPE is absent
        let header_bytes = fetch
            .header()
            .or_else(|| fetch.body().or(fetch.text()))
            .unwrap_or_default();
        let (subject, from, to, date, message_id, _) = parse_header_fields(header_bytes);
        let date = date.or_else(|| fetch.internal_date());
        (subject, from, to, date, message_id)
    };

    MailMessageSummary {
        uid,
        message_id,
        subject,
        from,
        to,
        date,
        flags,
        has_attachments: false,
        preview: String::new(),
        size,
    }
}

/// Decode IMAP envelope bytes (potentially RFC 2047 encoded) to a String.
/// Uses mailparse's encoded-word decoder via a synthetic header.
fn decode_envelope_bytes(raw: &[u8]) -> String {
    let mut header = b"X: ".to_vec();
    header.extend_from_slice(raw);
    header.extend_from_slice(b"\r\n");
    mailparse::parse_header(&header).map_or_else(|_| String::from_utf8_lossy(raw).into_owned(), |(h, _)| h.get_value())
}

/// Convert IMAP ENVELOPE address list to `Vec<MailAddress>`.
fn parse_envelope_addresses(addrs: Option<&Vec<async_imap::imap_proto::Address<'_>>>) -> Vec<MailAddress> {
    let Some(list) = addrs else {
        return vec![];
    };
    list.iter()
        .filter_map(|a| {
            let address = match (&a.mailbox, &a.host) {
                (Some(mb), Some(host)) => format!("{}@{}", String::from_utf8_lossy(mb), String::from_utf8_lossy(host)),
                _ => return None,
            };
            let name = a.name.as_deref().map(decode_envelope_bytes);
            Some(MailAddress { name, address })
        })
        .collect()
}

type HeaderFields = (
    String,
    Vec<MailAddress>,
    Vec<MailAddress>,
    Option<chrono::DateTime<chrono::FixedOffset>>,
    Option<String>,
    bool,
);

fn parse_header_fields(header_bytes: &[u8]) -> HeaderFields {
    let headers = mailparse::parse_headers(header_bytes)
        .map(|(h, _)| h)
        .unwrap_or_default();

    let mut subject = String::new();
    let mut from = Vec::new();
    let mut to = Vec::new();
    let mut date = None;
    let mut message_id = None;
    let mut has_attachments = false;

    for h in &headers {
        match h.get_key().to_lowercase().as_str() {
            "subject" => subject = h.get_value(),
            "from" => from = parse_address_list(&h.get_value()),
            "to" => to = parse_address_list(&h.get_value()),
            "date" => {
                date = mailparse::dateparse(&h.get_value())
                    .ok()
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.fixed_offset()));
            }
            "message-id" => message_id = Some(h.get_value()),
            "content-type" => {
                let ct = h.get_value().to_lowercase();
                if ct.contains("multipart/mixed") {
                    has_attachments = true;
                }
            }
            _ => {}
        }
    }

    (subject, from, to, date, message_id, has_attachments)
}

fn parse_address_list(raw: &str) -> Vec<MailAddress> {
    mailparse::addrparse(raw)
        .map(|addrs| {
            addrs
                .iter()
                .flat_map(|a| match a {
                    mailparse::MailAddr::Single(info) => {
                        vec![MailAddress {
                            name: info.display_name.clone(),
                            address: info.addr.clone(),
                        }]
                    }
                    mailparse::MailAddr::Group(group) => group
                        .addrs
                        .iter()
                        .map(|info| MailAddress {
                            name: info.display_name.clone(),
                            address: info.addr.clone(),
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn build_full_message(uid: u32, parsed: &mailparse::ParsedMail<'_>, flags: Vec<String>, size: u32) -> MailMessage {
    let headers = &parsed.headers;
    let mut subject = String::new();
    let mut from = Vec::new();
    let mut to = Vec::new();
    let mut cc = Vec::new();
    let mut bcc = Vec::new();
    let mut reply_to = Vec::new();
    let mut date = None;
    let mut message_id = None;
    let mut in_reply_to = None;
    let mut references = Vec::new();

    for h in headers {
        match h.get_key().to_lowercase().as_str() {
            "subject" => subject = h.get_value(),
            "from" => from = parse_address_list(&h.get_value()),
            "to" => to = parse_address_list(&h.get_value()),
            "cc" => cc = parse_address_list(&h.get_value()),
            "bcc" => bcc = parse_address_list(&h.get_value()),
            "reply-to" => reply_to = parse_address_list(&h.get_value()),
            "date" => {
                date = mailparse::dateparse(&h.get_value())
                    .ok()
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.fixed_offset()));
            }
            "message-id" => message_id = Some(h.get_value()),
            "in-reply-to" => in_reply_to = Some(h.get_value()),
            "references" => {
                references = h.get_value().split_whitespace().map(String::from).collect();
            }
            _ => {}
        }
    }

    let mut text_body = None;
    let mut html_body = None;
    let mut attachments = Vec::new();

    collect_parts(parsed, &mut text_body, &mut html_body, &mut attachments);

    MailMessage {
        uid,
        message_id,
        subject,
        from,
        to,
        cc,
        bcc,
        reply_to,
        date,
        flags,
        in_reply_to,
        references,
        text_body,
        html_body,
        attachments,
        size,
    }
}

fn collect_parts(
    part: &mailparse::ParsedMail<'_>,
    text_body: &mut Option<String>,
    html_body: &mut Option<String>,
    attachments: &mut Vec<MailAttachment>,
) {
    let ctype = &part.ctype;
    let mime = &ctype.mimetype;

    // Check if this is an attachment via Content-Disposition
    let is_attachment = part.headers.iter().any(|h| {
        h.get_key().to_lowercase() == "content-disposition" && h.get_value().to_lowercase().starts_with("attachment")
    });

    if is_attachment || ctype.params.contains_key("name") {
        if let Ok(body) = part.get_body_raw() {
            let filename = ctype
                .params
                .get("name")
                .cloned()
                .unwrap_or_else(|| "attachment".to_string());
            attachments.push(MailAttachment {
                filename,
                content_type: ctype.mimetype.clone(),
                size: body.len(),
                data: Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &body,
                )),
            });
        }
        return;
    }

    if part.subparts.is_empty() {
        // Leaf part.
        if let Ok(body) = part.get_body() {
            if mime.starts_with("text/plain") && text_body.is_none() {
                *text_body = Some(body);
            } else if mime.starts_with("text/html") && html_body.is_none() {
                *html_body = Some(body);
            }
        }
    } else {
        for sub in &part.subparts {
            collect_parts(sub, text_body, html_body, attachments);
        }
    }
}
