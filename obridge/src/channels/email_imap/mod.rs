//! email-imap 通道（內建 native）——IMAP 收信 → `InboundEvent`；`/send` → SMTP 寄信。
//!
//! **無狀態錨點**：`reply_to = <source>:msg:<URL-encoded Message-ID>?to=<回覆地址>`——
//! Message-ID 供還原 `In-Reply-To`/`References`（thread），`?to=` 供回信送達地址（Message-ID
//! 本身不含地址，無狀態設計需兩者都編進錨點）。bridge 重啟後照樣可回（不依賴本地映射）。
//!
//! 可測性：真實 IMAP/SMTP 各包在 [`MailSource`]/[`MailSink`] trait 後（[`imap::ImapSource`]／
//! [`smtp::SmtpSink`]），測試塞 fake——序列化／路由／錨點／管線全離線可測。

mod imap;
mod smtp;

use std::sync::Arc;

use ocontract::{EventKind, InboundEvent};
use mail_parser::MimeHeaders;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use tokio::sync::mpsc;

use crate::core::config::{ChannelCfg, EmailImapCfg, RouteCfg};

pub use imap::{ImapSource, MailSource, RawMail, Seen};
pub use smtp::{MailSink, OutgoingMail, SmtpSink};

/// email-imap 通道實例。
pub struct EmailImapChannel {
    cfg: EmailImapCfg,
    source: String,
    mail_source: Arc<dyn MailSource>,
    mail_sink: Arc<dyn MailSink>,
    /// 去重狀態檔（UIDVALIDITY＋last UID；重啟不重撈。路徑由建構端決定）。
    state_path: std::path::PathBuf,
}

impl EmailImapChannel {
    /// 正式建構（真實 IMAP＋SMTP）。`state_dir`：去重狀態檔的目錄。
    pub fn new(cfg: &ChannelCfg, state_dir: &std::path::Path) -> anyhow::Result<Self> {
        let e = cfg
            .email_imap
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("email-imap 通道 {} 缺少設定", cfg.source))?;
        let state_path = match &e.state_file {
            Some(p) => std::path::PathBuf::from(p),
            None => state_dir.join(format!("{}-state.json", cfg.source)),
        };
        Ok(Self {
            cfg: e.clone(),
            source: cfg.source.clone(),
            mail_source: Arc::new(ImapSource::new(e.imap.clone())),
            mail_sink: Arc::new(SmtpSink::new(e.smtp.clone())),
            state_path,
        })
    }

    /// 測試建構（注入 fake IO）。
    #[cfg(test)]
    pub fn with_io(
        cfg: &ChannelCfg,
        mail_source: Arc<dyn MailSource>,
        mail_sink: Arc<dyn MailSink>,
        state_path: std::path::PathBuf,
    ) -> Self {
        Self {
            cfg: cfg.email_imap.clone().expect("email_imap cfg"),
            source: cfg.source.clone(),
            mail_source,
            mail_sink,
            state_path,
        }
    }

    /// 讀去重狀態（無檔 → 重掃全部；Operoid 端 `(source, external_ref)` 去重兜底）。
    fn load_state(&self) -> Seen {
        std::fs::read(&self.state_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Seen::default())
    }

    fn save_state(&self, seen: &Seen) {
        if let Ok(json) = serde_json::to_vec_pretty(seen) {
            let _ = std::fs::write(&self.state_path, json);
        }
    }

    /// 一輪 poll：fetch → parse → 序列化 → 路由 → 逐員工投遞事件。回傳投遞數。
    /// 離線管線測試直接呼叫此函數。
    pub async fn poll_once(&self, tx: &mpsc::Sender<InboundEvent>) -> usize {
        let seen = self.load_state();
        // IMAP 是 sync（專屬 blocking thread），不佔 tokio worker。
        let src = Arc::clone(&self.mail_source);
        let fetched = tokio::task::spawn_blocking(move || src.fetch_new(&seen))
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("fetch task join 失敗：{e}")));
        let (mails, new_seen) = match fetched {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[obridge:{}] IMAP fetch 失敗（下輪重試）：{e}", self.source);
                return 0;
            }
        };
        let mut delivered = 0;
        for m in &mails {
            for ev in self.build_events(m) {
                let _ = tx.send(ev).await;
                delivered += 1;
            }
        }
        self.save_state(&new_seen);
        delivered
    }

    /// 一封信 → 0..n 個事件（路由命中 To/Cc 的每個 route 各一投；無命中 → 0，記 log）。
    fn build_events(&self, m: &RawMail) -> Vec<InboundEvent> {
        let Some(parsed) = mail_parser::MessageParser::default().parse(&m.raw) else {
            eprintln!("[obridge:{}] UID {} MIME 解析失敗，跳過", self.source, m.uid);
            return Vec::new();
        };
        let from = addr_list(parsed.from()).into_iter().next().unwrap_or_default();
        let tos = addr_list(parsed.to());
        let ccs = addr_list(parsed.cc());
        let to_addrs: Vec<String> = tos
            .iter()
            .chain(ccs.iter())
            .map(|s| s.to_lowercase())
            .collect();
        let subject = parsed.subject().unwrap_or("(無主旨)").to_string();
        let message_id = parsed.message_id().map(|id| id.to_string()).unwrap_or_default();
        let body = body_text(&parsed);
        let attachments: Vec<&str> = parsed
            .attachments()
            .filter_map(|a| a.attachment_name())
            .collect();

        // 去重鍵：Message-ID；缺 → UIDVALIDITY+UID（穩定性次佳但同信箱內唯一）。
        let external_ref = if message_id.is_empty() {
            format!("uidv{}-uid{}", m.uidvalidity, m.uid)
        } else {
            message_id.clone()
        };
        // 回覆錨點：msgid（URL-encoded）＋回覆地址（原寄件者）。缺 Message-ID 或 From → 無錨點
        // （Operoid 端回覆不外發，僅留內部歷史）。
        let reply_to = if message_id.is_empty() || from.is_empty() {
            None
        } else {
            Some(format!(
                "{}:msg:{}?to={}",
                self.source,
                utf8_percent_encode(&message_id, NON_ALPHANUMERIC),
                utf8_percent_encode(&from, NON_ALPHANUMERIC),
            ))
        };
        let content = format!(
            "From: {from}\nTo: {}\nCc: {}\nSubject: {subject}\nDate: {}\n\n{body}{}",
            tos.join("; "),
            ccs.join("; "),
            parsed.date().map(|d| d.to_rfc822()).unwrap_or_default(),
            if attachments.is_empty() {
                String::new()
            } else {
                format!("\n\n（附件：{}）", attachments.join("、"))
            },
        );

        let hits: Vec<&RouteCfg> = self
            .cfg
            .routes
            .iter()
            .filter(|r| to_addrs.contains(&r.address.to_lowercase()))
            .collect();
        if hits.is_empty() {
            eprintln!(
                "[obridge:{}] 信件〈{subject}〉未命中路由（To/Cc：{to_addrs:?}）——不投遞",
                self.source
            );
            return Vec::new();
        }
        hits.iter()
            .map(|r| InboundEvent {
                kind: EventKind::ExternalMessage,
                source: self.source.clone(),
                brain_id: r.brain.clone(),
                employee_id: r.employee.clone(),
                title: subject.clone(),
                content: content.clone(),
                external_ref: Some(external_ref.clone()),
                occurred_at: parsed.date().map(|d| d.to_rfc3339()),
                reply_to: reply_to.clone(),
                category: None,
            })
            .collect()
    }

    /// 解析 `to`：錨點 → (Message-ID, 回覆地址)；否則視為明示地址 → (None, to)。
    fn parse_to(&self, to: &str) -> anyhow::Result<(Option<String>, String)> {
        let prefix = format!("{}:msg:", self.source);
        let Some(rest) = to.strip_prefix(&prefix) else {
            return Ok((None, to.to_string()));
        };
        let (mid_enc, to_enc) = match rest.split_once("?to=") {
            Some((m, t)) => (m, Some(t)),
            None => (rest, None),
        };
        let mid = percent_decode_str(mid_enc)
            .decode_utf8()
            .map_err(|e| anyhow::anyhow!("錨點 Message-ID 解碼失敗：{e}"))?
            .to_string();
        let addr = match to_enc {
            Some(t) => percent_decode_str(t)
                .decode_utf8()
                .map_err(|e| anyhow::anyhow!("錨點回覆地址解碼失敗：{e}"))?
                .to_string(),
            None => anyhow::bail!("錨點缺少 ?to= 回覆地址（無法送達）"),
        };
        Ok((Some(mid), addr))
    }
}

#[async_trait::async_trait]
impl crate::core::channel::Channel for EmailImapChannel {
    fn source(&self) -> &str {
        &self.source
    }

    async fn run_inbound(&self, tx: mpsc::Sender<InboundEvent>) {
        loop {
            self.poll_once(&tx).await;
            tokio::time::sleep(std::time::Duration::from_secs(self.cfg.poll_secs.max(1))).await;
        }
    }

    async fn send(&self, to: &str, employee_id: &str, text: &str) -> anyhow::Result<()> {
        let (in_reply_to, addr) = self.parse_to(to)?;
        // RFC5322：In-Reply-To/References 需 <msg-id> 形式（mail-parser 解析時剝了括號，
        // 錨點可能帶或不帶——統一補上）。
        let in_reply_to = in_reply_to.map(|mid| {
            if mid.starts_with('<') && mid.ends_with('>') {
                mid
            } else {
                format!("<{mid}>")
            }
        });
        let sender = self.cfg.senders.iter().find(|s| s.employee == employee_id);
        let (from_addr, from_name) = match sender {
            Some(s) => (s.address.clone(), s.name.clone()),
            None => (self.cfg.smtp.username.clone(), None),
        };
        let subject = match &in_reply_to {
            Some(_) => format!("Re: {}", self.cfg.smtp.subject),
            None => self.cfg.smtp.subject.clone(),
        };
        self.mail_sink
            .send_mail(OutgoingMail {
                to: addr,
                from_addr,
                from_name,
                subject,
                body: text.to_string(),
                in_reply_to,
            })
            .await
    }
}

/// `Address`（List/Group）→ 地址字串清單（Group 攤平成成員地址）。
fn addr_list(a: Option<&mail_parser::Address<'_>>) -> Vec<String> {
    match a {
        Some(mail_parser::Address::List(v)) => v
            .iter()
            .filter_map(|x| x.address())
            .map(str::to_string)
            .collect(),
        Some(mail_parser::Address::Group(gs)) => gs
            .iter()
            .flat_map(|g| g.addresses.iter())
            .filter_map(|x| x.address())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

/// 取信本文：text/plain 優先；只有 HTML 時剝標籤（粗略——HTML 富文本轉換屬 v1 刻意不做）。
fn body_text(msg: &mail_parser::Message<'_>) -> String {
    if let Some(t) = msg.body_text(0) {
        return t.into_owned();
    }
    if let Some(h) = msg.body_html(0) {
        let mut out = String::with_capacity(h.len() / 2);
        let mut in_tag = false;
        for c in h.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        return out;
    }
    "(無本文)".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::channel::Channel;
    use crate::core::config;

    /// 最小可解析信件（含 Message-ID/From/To/Subject/Date/plain body）。
    fn sample_mail(msg_id: &str, to: &str) -> Vec<u8> {
        format!(
            "Message-ID: {msg_id}\r\nFrom: 張雅婷 <yt@corp.com>\r\nTo: {to}\r\n\
             Subject: RE: E-07 良率\r\nDate: Fri, 15 Aug 2026 09:30:00 +0800\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\r\n良率降到 91%，請查收附件報告。\r\n"
        )
        .into_bytes()
    }

    pub(crate) fn channel_cfg() -> ChannelCfg {
        let cfg = config::parse(config::tests::SAMPLE).unwrap();
        cfg.channels[0].clone()
    }

    pub(crate) struct FakeSource {
        pub mails: Vec<RawMail>,
    }
    impl MailSource for FakeSource {
        fn fetch_new(&self, _seen: &Seen) -> anyhow::Result<(Vec<RawMail>, Seen)> {
            Ok((
                self.mails.clone(),
                Seen { uidvalidity: 7, last_uid: 3 },
            ))
        }
    }

    fn tmp_state(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "obridge-test-{tag}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ))
    }

    /// 序列化：title/external_ref/reply_to 錨點（msgid＋回覆地址）/content 標頭齊全。
    #[tokio::test]
    async fn serializes_mail_to_contract_event() {
        let cfg = channel_cfg();
        let ch = EmailImapChannel::with_io(
            &cfg,
            Arc::new(FakeSource {
                mails: vec![RawMail { uid: 3, uidvalidity: 7, raw: sample_mail("<CAB123@mailer>", "steve@corp.com") }],
            }),
            smtp::tests::fake_sink(),
            tmp_state("ser"),
        );
        let (tx, mut rx) = mpsc::channel(10);
        let n = ch.poll_once(&tx).await;
        assert_eq!(n, 1);
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.source, "email");
        assert_eq!(ev.title, "RE: E-07 良率");
        assert_eq!(ev.external_ref.as_deref(), Some("CAB123@mailer"));
        assert!(ev.content.contains("From: yt@corp.com"));
        assert!(ev.content.contains("91%"));
        let anchor = ev.reply_to.unwrap();
        assert!(anchor.starts_with("email:msg:"), "{anchor}");
        assert!(anchor.contains("?to="), "錨點應含回覆地址：{anchor}");
        assert_eq!(ev.employee_id.as_deref(), Some("Steve-TW"));
        assert!(ev.occurred_at.as_deref().unwrap().starts_with("2026-"));
        // 狀態檔已持久化（重啟不重撈）。
        assert!(ch.state_path.exists());
        let _ = std::fs::remove_file(&ch.state_path);
    }

    /// 路由：未命中（收件地址不在 routes）→ 不投遞。
    #[tokio::test]
    async fn unrouted_mail_not_delivered() {
        let cfg = channel_cfg();
        let ch = EmailImapChannel::with_io(
            &cfg,
            Arc::new(FakeSource {
                mails: vec![RawMail { uid: 3, uidvalidity: 7, raw: sample_mail("<x@y>", "someone@else.com") }],
            }),
            smtp::tests::fake_sink(),
            tmp_state("unrouted"),
        );
        let (tx, _rx) = mpsc::channel(10);
        assert_eq!(ch.poll_once(&tx).await, 0);
    }

    /// 錨點解析：send 收到錨點 → In-Reply-To 還原＋送達原寄件者；明示地址 → 直寄。
    #[tokio::test]
    async fn anchor_and_plain_send_parse() {
        let cfg = channel_cfg();
        let sink = smtp::tests::fake_sink();
        let ch = EmailImapChannel::with_io(
            &cfg,
            Arc::new(FakeSource { mails: vec![] }),
            sink.clone(),
            tmp_state("anchor"),
        );
        // 錨點（模擬 build_events 產生的形狀）。
        let anchor = format!(
            "email:msg:{}?to={}",
            utf8_percent_encode("<CAB123@mailer>", NON_ALPHANUMERIC),
            utf8_percent_encode("yt@corp.com", NON_ALPHANUMERIC),
        );
        ch.send(&anchor, "Steve-TW", "已收到，處理中。").await.unwrap();
        {
            let sent = sink.sent.lock().unwrap();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].to, "yt@corp.com");
            assert_eq!(sent[0].in_reply_to.as_deref(), Some("<CAB123@mailer>"));
            assert_eq!(sent[0].from_addr, "steve@corp.com", "senders 應選 From 身分");
            assert!(sent[0].subject.starts_with("Re:"));
        }
        // 明示地址（無錨點）→ 直寄、無 In-Reply-To。
        ch.send("boss@corp.com", "Steve-TW", "結論報告").await.unwrap();
        let sent = sink.sent.lock().unwrap();
        assert_eq!(sent[1].to, "boss@corp.com");
        assert_eq!(sent[1].in_reply_to, None);
    }
}
