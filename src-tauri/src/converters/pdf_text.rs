//! PDF → 純文字（companies/meeting 工廠用）。

use std::path::Path;

use anyhow::{Context, Result};

pub fn extract(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("讀取 PDF 失敗：{}", path.display()))?;
    let text = pdf_extract::extract_text_from_mem(&bytes).context("PDF 文字解析失敗")?;
    Ok(text)
}
