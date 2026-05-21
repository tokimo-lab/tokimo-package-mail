# tokimo-package-mail

Async email client library for Rust — IMAP/SMTP with multi-provider support, full message parsing, and IDLE push.

## Features

- **IMAP** — connect, list folders, fetch messages (headers / body / attachments), UID-based operations, IDLE push
- **SMTP** — send with TLS/STARTTLS, HTML + plain text, attachments, inline images
- **Provider presets** — auto-detect IMAP/SMTP settings from email domain (Gmail, Outlook, QQ Mail, NetEase, iCloud, Yahoo, Zoho, ProtonMail, Yandex, Ali Mail, ...)
- **Full parsing** — MIME multipart, headers, addresses, attachments, HTML→text fallback
- **Async** — built on tokio + tokio-rustls, non-blocking throughout
- **TLS** — rustls with aws-lc-rs, webpki-roots certificate store

## Usage

```rust
use tokimo_package_mail::{MailClient, MailAccountConfig, SecurityMode};

let config = MailAccountConfig {
    display_name: "Work".into(),
    email: "user@gmail.com".into(),
    imap_host: "imap.gmail.com".into(),
    imap_port: 993,
    imap_security: SecurityMode::Tls,
    imap_username: "user@gmail.com".into(),
    imap_password: "app-password".into(),
    smtp_host: "smtp.gmail.com".into(),
    smtp_port: 465,
    smtp_security: SecurityMode::Tls,
    smtp_username: "user@gmail.com".into(),
    smtp_password: "app-password".into(),
    sender_name: None,
};

let client = MailClient::new(config);
client.test_connection().await?;

let folders = client.list_folders().await?;
let messages = client.fetch_messages("INBOX", None, Some(20)).await?;
```

### Provider auto-detection

```rust
use tokimo_package_mail::provider;

let preset = provider::detect_provider("user@gmail.com");
// preset.imap_host == "imap.gmail.com", preset.smtp_host == "smtp.gmail.com", ...
```

## Cargo

```toml
tokimo-package-mail = { git = "https://github.com/tokimo-lab/tokimo-package-mail" }
```

## License

MIT
