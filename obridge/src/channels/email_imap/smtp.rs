//! SMTP 寄信（lettre，async）。`MailSink` 抽象供測試注入 fake。

use serde::{Deserialize, Serialize};

use crate::core::config::SmtpCfg;

/// 一封外寄信（錨點已在通道層解析完——此處只管寄）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMail {
    pub to: String,
    pub from_addr: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub body: String,
    /// 回原 thread 的 Message-ID（設 In-Reply-To/References）。
    pub in_reply_to: Option<String>,
}

/// 寄信抽象（測試塞 fake——見 tests::fake_sink）。
#[async_trait::async_trait]
pub trait MailSink: Send + Sync {
    async fn send_mail(&self, mail: OutgoingMail) -> anyhow::Result<()>;
}

/// 真實 SMTP 實作（lettre Tokio1Transport；port 465 → implicit TLS，其他 → STARTTLS relay）。
pub struct SmtpSink {
    cfg: SmtpCfg,
}

impl SmtpSink {
    pub fn new(cfg: SmtpCfg) -> Self {
        Self { cfg }
    }
}

#[async_trait::async_trait]
impl MailSink for SmtpSink {
    async fn send_mail(&self, mail: OutgoingMail) -> anyhow::Result<()> {
        use lettre::message::header::ContentType;
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

        let from = match &mail.from_name {
            Some(name) => format!("{} <{}>", name, mail.from_addr)
                .parse()
                .map_err(|e| anyhow::anyhow!("From 地址解析失敗：{e}"))?,
            None => mail
                .from_addr
                .parse()
                .map_err(|e| anyhow::anyhow!("From 地址解析失敗：{e}"))?,
        };
        let mut builder = lettre::Message::builder()
            .from(from)
            .to(mail
                .to
                .parse()
                .map_err(|e| anyhow::anyhow!("To 地址解析失敗：{e}"))?)
            .subject(&mail.subject);
        // thread 還原：In-Reply-To/References 同指原 Message-ID（無整串 References——
        // 無狀態錨點只編了 Message-ID，主流客戶端足以 threading）。
        if let Some(mid) = &mail.in_reply_to {
            builder = builder
                .header(lettre::message::header::InReplyTo::from(mid.clone()))
                .header(lettre::message::header::References::from(mid.clone()));
        }
        let email = builder
            .header(ContentType::TEXT_PLAIN)
            .body(mail.body.clone())?;

        let creds = Credentials::new(
            self.cfg.username.clone(),
            self.cfg.password.clone(),
        );
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if self.cfg.port == 465 {
            // implicit TLS（SMTPS）
            let tls = lettre::transport::smtp::client::Tls::Wrapper(
                lettre::transport::smtp::client::TlsParameters::new(self.cfg.host.clone())?,
            );
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(self.cfg.host.clone())
                .port(465)
                .tls(tls)
                .credentials(creds)
                .build()
        } else {
            // STARTTLS（587 等）
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.cfg.host)?
                .port(self.cfg.port)
                .credentials(creds)
                .build()
        };
        mailer.send(email).await?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 測試用 fake sink：記錄外寄信供斷言。
    pub(crate) fn fake_sink() -> Arc<FakeSink> {
        Arc::new(FakeSink {
            sent: Mutex::new(Vec::new()),
        })
    }

    pub(crate) struct FakeSink {
        pub sent: Mutex<Vec<OutgoingMail>>,
    }

    #[async_trait::async_trait]
    impl MailSink for FakeSink {
        async fn send_mail(&self, mail: OutgoingMail) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(mail);
            Ok(())
        }
    }

    /// 真實 SMTP smoke（#[ignore]——需 OBRIDGE_SMTP_HOST/USER/PASS/TO 環境變數）。
    #[tokio::test]
    #[ignore]
    async fn real_smtp_send() {
        let cfg = SmtpCfg {
            host: std::env::var("OBRIDGE_SMTP_HOST").unwrap(),
            port: std::env::var("OBRIDGE_SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(465),
            username: std::env::var("OBRIDGE_SMTP_USER").unwrap(),
            password: std::env::var("OBRIDGE_SMTP_PASS").unwrap(),
            subject: "Operoid 測試".into(),
        };
        let sink = SmtpSink::new(cfg);
        sink.send_mail(OutgoingMail {
            to: std::env::var("OBRIDGE_SMTP_TO").unwrap(),
            from_addr: std::env::var("OBRIDGE_SMTP_USER").unwrap(),
            from_name: Some("Obridge 測試".into()),
            subject: "Obridge smoke test".into(),
            body: "這是 Obridge SMTP 寄信 smoke test。".into(),
            in_reply_to: None,
        })
        .await
        .unwrap();
    }
}
