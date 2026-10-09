//! 轉換器：把原始檔轉成 gbrain-legal markdown。
//!
//! - `slug`：slug 函式（移植自 Python；與既有語料一致）——已搬入 `ocore::slug`（P1a）
//! - `frontmatter`：YAML frontmatter 產生/解析
//! - `wikilink`：dir-qualified `[[dir/slug|Name]]`
//! - `csv_people`：Google Contacts CSV → people/*.md（移植自 csv_to_people.py）
//! - （Phase 4/5）text_to_md、extract_companies、pdf_text
//! - `mineru`：K1 多模態檢索升級——複雜 PDF → MinerU（複雜度分流＋取用階梯＋
//!   egress 信任分層）→ 章節切塊筆記＋圖片筆記＋figures.sqlite sidecar 來源資料

pub mod csv_people;
pub mod extract_companies;
pub mod frontmatter;
pub mod mineru;
pub use crate::slug;
pub mod pdf_text;
pub mod text_to_md;
pub mod wikilink;

#[cfg(test)]
mod mineru_real_tests;

