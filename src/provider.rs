use serde::{Deserialize, Serialize};

use crate::config::{MailAccountConfig, SecurityMode};

/// Known email provider with preset IMAP/SMTP settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailProvider {
    QqMail,
    Netease126,
    Netease163,
    ICloud,
    Gmail,
    Outlook,
    Yahoo,
    Zoho,
    ProtonMail,
    Yandex,
    QqExmail,
    AliMail,
    Custom,
}

/// Preset configuration for a known provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub provider: MailProvider,
    pub display_name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SecurityMode,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SecurityMode,
    /// Instructions for user — how to get app password, enable IMAP, etc.
    pub setup_instructions: Vec<String>,
    /// Whether an app-specific password is needed (most providers do).
    pub requires_app_password: bool,
    /// URL to the provider's app-password or security settings page.
    pub app_password_url: Option<String>,
    /// Domains associated with this provider (for auto-detection).
    pub domains: Vec<String>,
}

/// Get all known provider presets.
#[allow(clippy::too_many_lines)]
pub fn all_provider_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            provider: MailProvider::QqMail,
            display_name: "QQ 邮箱".into(),
            imap_host: "imap.qq.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.qq.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "登录 QQ 邮箱网页版 → 设置 → 账户".into(),
                "找到 \"POP3/IMAP/SMTP/Exchange/CardDAV/CalDAV服务\"".into(),
                "开启 IMAP/SMTP 服务".into(),
                "按提示发送短信验证，获取授权码".into(),
                "将授权码作为密码填入（不是 QQ 密码）".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://mail.qq.com/".into()),
            domains: vec!["qq.com".into(), "foxmail.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::Netease163,
            display_name: "网易 163 邮箱".into(),
            imap_host: "imap.163.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.163.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "登录 163 邮箱网页版 → 设置 → POP3/SMTP/IMAP".into(),
                "开启 IMAP/SMTP 服务".into(),
                "按提示设置授权码（需要绑定手机号）".into(),
                "将授权码作为密码填入（不是邮箱登录密码）".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://mail.163.com/".into()),
            domains: vec!["163.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::Netease126,
            display_name: "网易 126 邮箱".into(),
            imap_host: "imap.126.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.126.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "登录 126 邮箱网页版 → 设置 → POP3/SMTP/IMAP".into(),
                "开启 IMAP/SMTP 服务".into(),
                "按提示设置授权码".into(),
                "将授权码作为密码填入".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://mail.126.com/".into()),
            domains: vec!["126.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::ICloud,
            display_name: "iCloud Mail".into(),
            imap_host: "imap.mail.me.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.mail.me.com".into(),
            smtp_port: 587,
            smtp_security: SecurityMode::StartTls,
            setup_instructions: vec![
                "Go to appleid.apple.com → Sign In".into(),
                "Navigate to Sign-In and Security → App-Specific Passwords".into(),
                "Click 'Generate an app-specific password'".into(),
                "Name it 'Tokimo Mail' and click Create".into(),
                "Copy the generated password and use it here".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://appleid.apple.com/account/manage".into()),
            domains: vec!["icloud.com".into(), "me.com".into(), "mac.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::Gmail,
            display_name: "Gmail".into(),
            imap_host: "imap.gmail.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.gmail.com".into(),
            smtp_port: 587,
            smtp_security: SecurityMode::StartTls,
            setup_instructions: vec![
                "Go to myaccount.google.com → Security".into(),
                "Enable 2-Step Verification if not already on".into(),
                "Go to Security → 2-Step Verification → App passwords".into(),
                "Select 'Mail' and your device, then Generate".into(),
                "Copy the 16-character app password and use it here".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://myaccount.google.com/apppasswords".into()),
            domains: vec!["gmail.com".into(), "googlemail.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::Outlook,
            display_name: "Outlook / Hotmail".into(),
            imap_host: "outlook.office365.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.office365.com".into(),
            smtp_port: 587,
            smtp_security: SecurityMode::StartTls,
            setup_instructions: vec![
                "Go to account.microsoft.com → Security".into(),
                "Enable Two-step verification".into(),
                "Go to Security → Advanced security options → App passwords".into(),
                "Create a new app password and use it here".into(),
            ],
            requires_app_password: true,
            app_password_url: Some(
                "https://account.live.com/proofs/AppPassword".into(),
            ),
            domains: vec![
                "outlook.com".into(),
                "hotmail.com".into(),
                "live.com".into(),
                "msn.com".into(),
            ],
        },
        ProviderPreset {
            provider: MailProvider::Yahoo,
            display_name: "Yahoo Mail".into(),
            imap_host: "imap.mail.yahoo.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.mail.yahoo.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "Go to login.yahoo.com → Account Security".into(),
                "Enable Two-step verification".into(),
                "Click 'Generate app password'".into(),
                "Select 'Other App', name it 'Tokimo Mail'".into(),
                "Copy the generated password and use it here".into(),
            ],
            requires_app_password: true,
            app_password_url: Some(
                "https://login.yahoo.com/account/security".into(),
            ),
            domains: vec!["yahoo.com".into(), "yahoo.co.jp".into()],
        },
        ProviderPreset {
            provider: MailProvider::Zoho,
            display_name: "Zoho Mail".into(),
            imap_host: "imap.zoho.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.zoho.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "Log in to Zoho Mail → Settings → Mail Accounts".into(),
                "Enable IMAP Access under the account".into(),
                "If 2FA is enabled, generate an App-Specific Password".into(),
                "Use the app password or your regular password here".into(),
            ],
            requires_app_password: false,
            app_password_url: Some("https://accounts.zoho.com/home".into()),
            domains: vec!["zoho.com".into(), "zohomail.com".into()],
        },
        ProviderPreset {
            provider: MailProvider::ProtonMail,
            display_name: "ProtonMail (Bridge)".into(),
            imap_host: "127.0.0.1".into(),
            imap_port: 1143,
            imap_security: SecurityMode::StartTls,
            smtp_host: "127.0.0.1".into(),
            smtp_port: 1025,
            smtp_security: SecurityMode::StartTls,
            setup_instructions: vec![
                "Install and run Proton Mail Bridge on your machine".into(),
                "Log in to the Bridge with your Proton account".into(),
                "The Bridge will show IMAP/SMTP credentials".into(),
                "Use the Bridge-generated password (not your Proton password)".into(),
                "IMAP/SMTP connect to localhost via the Bridge".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://proton.me/mail/bridge".into()),
            domains: vec!["protonmail.com".into(), "proton.me".into(), "pm.me".into()],
        },
        ProviderPreset {
            provider: MailProvider::Yandex,
            display_name: "Yandex Mail".into(),
            imap_host: "imap.yandex.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.yandex.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "Go to Yandex Passport → Manage account".into(),
                "Enable IMAP in Yandex Mail settings".into(),
                "Create an app password at id.yandex.com".into(),
                "Use the app password here".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://id.yandex.com/security/app-passwords".into()),
            domains: vec!["yandex.com".into(), "yandex.ru".into(), "ya.ru".into()],
        },
        ProviderPreset {
            provider: MailProvider::QqExmail,
            display_name: "腾讯企业邮箱".into(),
            imap_host: "imap.exmail.qq.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.exmail.qq.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "登录腾讯企业邮箱 → 设置 → 客户端设置".into(),
                "开启 IMAP/SMTP 服务".into(),
                "生成客户端专用密码".into(),
                "使用生成的密码填入".into(),
            ],
            requires_app_password: true,
            app_password_url: Some("https://exmail.qq.com/".into()),
            domains: vec![], // Custom domains
        },
        ProviderPreset {
            provider: MailProvider::AliMail,
            display_name: "阿里企业邮箱".into(),
            imap_host: "imap.mxhichina.com".into(),
            imap_port: 993,
            imap_security: SecurityMode::Tls,
            smtp_host: "smtp.mxhichina.com".into(),
            smtp_port: 465,
            smtp_security: SecurityMode::Tls,
            setup_instructions: vec![
                "登录阿里企业邮箱 → 设置 → 客户端设置".into(),
                "确保 IMAP 协议已开启".into(),
                "使用邮箱登录密码即可".into(),
            ],
            requires_app_password: false,
            app_password_url: Some("https://qiye.aliyun.com/".into()),
            domains: vec![], // Custom domains
        },
    ]
}

/// Try to detect provider from email domain.
pub fn detect_provider(email: &str) -> Option<ProviderPreset> {
    let domain = email.rsplit('@').next()?.to_lowercase();
    all_provider_presets()
        .into_iter()
        .find(|p| p.domains.iter().any(|d| d == &domain))
}

/// Build a `MailAccountConfig` from a preset + user credentials.
pub fn config_from_preset(
    preset: &ProviderPreset,
    email: &str,
    password: &str,
    display_name: Option<&str>,
) -> MailAccountConfig {
    MailAccountConfig {
        display_name: display_name
            .unwrap_or(&preset.display_name)
            .to_string(),
        email: email.to_string(),
        imap_host: preset.imap_host.clone(),
        imap_port: preset.imap_port,
        imap_security: preset.imap_security,
        imap_username: email.to_string(),
        imap_password: password.to_string(),
        smtp_host: preset.smtp_host.clone(),
        smtp_port: preset.smtp_port,
        smtp_security: preset.smtp_security,
        smtp_username: email.to_string(),
        smtp_password: password.to_string(),
        sender_name: display_name.map(str::to_string),
    }
}
