//! 轉換器：把原始檔轉成 gbrain-legal markdown。
//!
//! - `slug`：slug 函式（移植自 Python；與既有語料一致）
//! - `frontmatter`：YAML frontmatter 產生/解析
//! - `wikilink`：dir-qualified `[[dir/slug|Name]]`
//! - `csv_people`：Google Contacts CSV → people/*.md（移植自 csv_to_people.py）
//! - （Phase 4/5）text_to_md、extract_companies、pdf_text

pub mod csv_people;
pub mod extract_companies;
pub mod frontmatter;
pub mod pdf_text;
pub mod slug;
pub mod text_to_md;
pub mod wikilink;
