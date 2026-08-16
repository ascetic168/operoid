//! IMAP 進信（sync——由通道以 `spawn_blocking` 呼叫）。`imap` crate 3.0.0-alpha＋rustls
//! （`ClientBuilder` 預設 AutoTls——port 993 走 implicit TLS）。
//!
//! 已見信追蹤：`(UIDVALIDITY, last_uid)`。UIDVALIDITY 變動（信箱重建）→ 重掃全部
//! （Operoid 端 `(source, external_ref)` 去重兜底，不會重複喚醒）。

use serde::{Deserialize, Serialize};

use crate::core::config::ImapCfg;

/// 一封原始信（完整 RFC822 bytes——MIME 解析在通道層做，此處保持薄）。
#[derive(Debug, Clone)]
pub struct RawMail {
    pub uid: u32,
    pub uidvalidity: u32,
    pub raw: Vec<u8>,
}

/// 已見信狀態（JSON 持久化）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Seen {
    pub uidvalidity: u32,
    pub last_uid: u32,
}

/// 進信抽象（測試塞 fake——見 mod tests）。
pub trait MailSource: Send + Sync {
    /// 取上次 `last_uid` 之後的新信；回傳 (新信, 新狀態)。失敗 Err（通道記 log 下輪重試）。
    fn fetch_new(&self, seen: &Seen) -> anyhow::Result<(Vec<RawMail>, Seen)>;
}

/// 真實 IMAP 實作：連線 → select → UID 搜尋 → fetch RFC822。每次 poll 建新連線
/// （poll 間隔 60s 級，長連線的斷線處理不值得 v1 複雜度）。
pub struct ImapSource {
    cfg: ImapCfg,
}

impl ImapSource {
    pub fn new(cfg: ImapCfg) -> Self {
        Self { cfg }
    }
}

impl MailSource for ImapSource {
    fn fetch_new(&self, seen: &Seen) -> anyhow::Result<(Vec<RawMail>, Seen)> {
        let client = imap::ClientBuilder::new(&self.cfg.host, self.cfg.port).connect()?;
        let mut session = client
            .login(&self.cfg.username, &self.cfg.password)
            .map_err(|e| anyhow::anyhow!("IMAP login 失敗：{}", e.0))?;
        let result = (|| -> anyhow::Result<(Vec<RawMail>, Seen)> {
            let mailbox = session.select(&self.cfg.folder)?;
            let uidvalidity = mailbox.uid_validity.unwrap_or(0);
            // UIDVALIDITY 變動 → 從頭掃（Operoid 去重兜底）。
            let last_uid = if uidvalidity == seen.uidvalidity {
                seen.last_uid
            } else {
                0
            };
            let uids = session.uid_search("ALL")?;
            let new_uids: Vec<u32> = {
                let mut v: Vec<u32> = uids.into_iter().filter(|&u| u > last_uid).collect();
                v.sort_unstable();
                v
            };
            if new_uids.is_empty() {
                return Ok((Vec::new(), Seen { uidvalidity, last_uid }));
            }
            let set = new_uids
                .iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let fetches = session.uid_fetch(set, "(UID RFC822)")?;
            let mut mails = Vec::new();
            for f in fetches.iter() {
                if let (Some(uid), Some(body)) = (f.uid, f.body()) {
                    mails.push(RawMail { uid, uidvalidity, raw: body.to_vec() });
                }
            }
            let new_last = new_uids.last().copied().unwrap_or(last_uid);
            Ok((mails, Seen { uidvalidity, last_uid: new_last }))
        })();
        let _ = session.logout();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真實信箱 smoke（#[ignore]——需 OBRIDGE_IMAP_HOST/USER/PASS 環境變數）。
    /// 驗證：連線、搜尋、取信不炸；第二次 poll 只取新信（last_uid 推進）。
    #[test]
    #[ignore]
    fn real_imap_fetch() {
        let cfg = ImapCfg {
            host: std::env::var("OBRIDGE_IMAP_HOST").unwrap(),
            port: 993,
            username: std::env::var("OBRIDGE_IMAP_USER").unwrap(),
            password: std::env::var("OBRIDGE_IMAP_PASS").unwrap(),
            folder: "INBOX".into(),
        };
        let src = ImapSource::new(cfg);
        let (mails, seen) = src.fetch_new(&Seen::default()).unwrap();
        println!("首輪取 {} 封（last_uid={}）", mails.len(), seen.last_uid);
        let (mails2, seen2) = src.fetch_new(&seen).unwrap();
        assert!(
            mails2.iter().all(|m| m.uid > seen.last_uid),
            "第二輪只應有新信"
        );
        assert_eq!(seen2.uidvalidity, seen.uidvalidity);
    }
}
