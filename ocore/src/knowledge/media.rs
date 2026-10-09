//! K5/P1 媒體簽名 URL——企業瀏覽器（無 Tauri asset protocol）讀取檢索圖片的
//! 認證面擴充：**HMAC-SHA256 短時簽名**（綁 path＋principal＋過期），取代
//! 「token 入 query」反模式（CWE-598）。
//!
//! 紀律：
//! - 金鑰**每次開機隨機**——重啟即失效；前端渲染時重簽（一次批量 POST）。
//! - 簽名前先過**知識織網授權**（圖的 source ∈ principal 的 authorized_sources）
//!   ——媒體層繼承 M1 鐵律，不繞過（C4）。
//! - serve 端再雙重查驗：簽名驗證＋sidecar allowlist（路徑必須登記過）。

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 媒體簽名器（金鑰每次開機隨機；per-boot 重啟即失效是設計不是缺陷——
/// 前端在渲染訊息時重簽）。
pub struct MediaSigner {
    key: [u8; 32],
}

impl MediaSigner {
    pub fn from_random() -> Result<Self> {
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key).map_err(|e| anyhow::anyhow!("熵源不可用：{e}"))?;
        Ok(Self { key })
    }

    /// 供測試/固定金鑰場景。
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key }
    }

    fn mac(&self, path: &str, principal: &str, exp_unix: u64) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("hmac key");
        mac.update(path.as_bytes());
        mac.update(principal.as_bytes());
        mac.update(&exp_unix.to_be_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// 簽名（hex）。
    pub fn sign(&self, path: &str, principal: &str, exp_unix: u64) -> String {
        hex(&self.mac(path, principal, exp_unix))
    }

    /// 驗證：簽名吻合且未過期（常數時間比較）。
    pub fn verify(
        &self,
        path: &str,
        principal: &str,
        exp_unix: u64,
        now_unix: u64,
        sig: &str,
    ) -> bool {
        if exp_unix <= now_unix {
            return false;
        }
        let expected = self.mac(path, principal, exp_unix);
        match hex_decode(sig) {
            Some(sig_bytes) => {
                sig_bytes.len() == expected.len()
                    && sig_bytes
                        .iter()
                        .zip(&expected)
                        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                        == 0
            }
            None => false,
        }
    }
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 簽名/驗證往返；竄改任一成分（path/principal/exp/sig）即拒。
    #[test]
    fn sign_verify_and_tamper() {
        let signer = MediaSigner::from_key([7u8; 32]);
        let sig = signer.sign("img/a.jpg", "user-bob", 2_000_000_000);
        assert!(signer.verify("img/a.jpg", "user-bob", 2_000_000_000, 1_999_999_999, &sig));
        // 過期
        assert!(!signer.verify("img/a.jpg", "user-bob", 2_000_000_000, 2_000_000_001, &sig));
        // 竄改
        assert!(!signer.verify("img/other.jpg", "user-bob", 2_000_000_000, 1_999_999_999, &sig));
        assert!(!signer.verify("img/a.jpg", "user-evil", 2_000_000_000, 1_999_999_999, &sig));
        let bad = format!("{}00", &sig[..sig.len() - 2]);
        assert!(!signer.verify("img/a.jpg", "user-bob", 2_000_000_000, 1_999_999_999, &bad));
        // 不同 principal 的簽名不互通（sig 由 path+principal+exp 決定）。
        let sig_b = signer.sign("img/a.jpg", "user-amy", 2_000_000_000);
        assert_ne!(sig, sig_b);
    }
}

/// 查詢字串值的百分號編碼（僅供簽名 URL；不需完整 application/x-www-form-urlencoded）。
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn percent_encode_covers_reserved() {
        assert_eq!(percent_encode("a b/c?d"), "a%20b%2Fc%3Fd");
        assert_eq!(percent_encode("plain"), "plain");
    }
}
