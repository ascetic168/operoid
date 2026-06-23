//! CSV → gbrain-legal people/*.md —— 忠實移植自 `csv_to_people.py`。
//!
//! Google Contacts 匯出 contacts.csv → people 頁（frontmatter type/title/tags +
//! compiled_truth body + `公司/組織:` bullets）。關鍵 gotcha 全部移植：
//!   - 單一編號欄位可塞多值（` ::: `），見於 Phone/Email/Website/Address 與 Org 的
//!     Name/Title/Department → expand_split / first_seg / split_multi
//!   - type label 前導 `*` 表 primary → norm_type 剝除
//!   - 同人 dedup key = (name, primary org, first phone)，聯集欄位 → merge_contacts
//!   - slug 由 slug::slugify 產生（與既有語料一致），碰撞 `-2/-3`
//!
//! 產出未寫檔；由 factory/command 決定寫入 people/<slug>.md。

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{frontmatter, slug};

// ── 欄位上限（與 Python 常數一致）─────────────────────────────────────────
const N_PHONE: usize = 5;
const EMAIL_MAX: usize = 3;
const ADDR_MAX: usize = 2;
const WEB_MAX: usize = 3;
const ORG_MAX: usize = 2;

// ── 資料結構 ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default)]
struct Org {
    name: String,
    title: String,
    dept: String,
}

#[derive(Debug, Clone, Default)]
struct Typed {
    typ: String,
    value: String,
}

#[derive(Debug, Clone, Default)]
struct Address {
    typ: String,
    addr: String,
}

#[derive(Debug, Clone, Default)]
struct Contact {
    name: String,
    nick: String,
    birthday: String,
    orgs: Vec<Org>,
    phones: Vec<Typed>,
    emails: Vec<Typed>,
    websites: Vec<Typed>,
    addresses: Vec<Address>,
    notes: Vec<String>,
}

/// 一個要寫入 people/ 的頁面。
#[derive(Debug, Clone, Serialize)]
pub struct PersonPage {
    pub slug: String,
    pub name: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeopleImport {
    pub rows_read: usize,
    pub rows_skipped: usize,
    pub groups_merged: usize,
    pub redundant_folded: usize,
    pub pages: Vec<PersonPage>,
}

// ── 純文字輔助（對應 Python 的 clean / norm_type / first_seg / split_multi）──

fn clean(v: &str) -> String {
    let v = v.trim();
    if v.is_empty() {
        return String::new();
    }
    if v.chars().all(|c| matches!(c, ' ' | ':' | '.' | '-' | '_')) {
        return String::new();
    }
    v.to_string()
}

fn norm_type(v: &str) -> String {
    let mut v = clean(v);
    while let Some(rest) = v.strip_prefix('*') {
        v = rest.trim_start().to_string();
    }
    v
}

fn first_seg(v: &str) -> String {
    let v = clean(v);
    if v.is_empty() {
        return String::new();
    }
    v.split(":::").next().unwrap_or("").trim().to_string()
}

fn split_multi(v: &str) -> Vec<String> {
    if v.is_empty() {
        return Vec::new();
    }
    v.split(":::")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// 把每個 item 的 value 依 ` ::: ` 拆成多個 item（沿用父項 type）。
fn expand_split_value(items: Vec<Typed>) -> Vec<Typed> {
    let mut out = Vec::new();
    for it in items {
        for seg in it.value.split(":::") {
            let seg = clean(seg);
            if !seg.is_empty() {
                let mut nit = it.clone();
                nit.value = seg;
                out.push(nit);
            }
        }
    }
    out
}

// ── 列解析 ─────────────────────────────────────────────────────────────

type Row = HashMap<String, String>;

fn row_get<'a>(row: &'a Row, key: &str) -> &'a str {
    row.get(key).map(|s| s.as_str()).unwrap_or("")
}

/// 收集編號欄位，例如 Phone 1..5 with fields [Type, Value]。
/// 傳回的 Row 以「欄位名」為 key（Type/Value），value 取自 `<base> <i> - <field>` 欄。
fn numbered(row: &Row, base: &str, fields: &[&str], count: usize) -> Vec<Row> {
    let mut items = Vec::new();
    for i in 1..=count {
        let vals: Row = fields
            .iter()
            .map(|f| {
                let col = format!("{base} {i} - {f}");
                (f.to_string(), clean(row_get(row, &col)))
            })
            .collect();
        if fields
            .iter()
            .any(|f| !vals.get(*f).map(|s| s.is_empty()).unwrap_or(true))
        {
            items.push(vals);
        }
    }
    items
}

fn compose_address(row: &Row, i: usize) -> String {
    let fmt = clean(row_get(row, &format!("Address {i} - Formatted")));
    if !fmt.is_empty() {
        return fmt;
    }
    let part_keys: [String; 6] = [
        format!("Address {i} - Street"),
        format!("Address {i} - Extended Address"),
        format!("Address {i} - City"),
        format!("Address {i} - Region"),
        format!("Address {i} - Postal Code"),
        format!("Address {i} - Country"),
    ];
    let joined: Vec<String> = part_keys
        .iter()
        .map(|k| row_get(row, k).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    clean(&joined.join(", "))
}

fn parse_contact(row: &Row) -> Contact {
    let name = {
        let n = clean(row_get(row, "Name"));
        if !n.is_empty() {
            n
        } else {
            first_seg(row_get(row, "Organization 1 - Name"))
        }
    };

    let mut orgs = Vec::new();
    for i in 1..=ORG_MAX {
        let nm = first_seg(row_get(row, &format!("Organization {i} - Name")));
        let title = first_seg(row_get(row, &format!("Organization {i} - Title")));
        let dept = first_seg(row_get(row, &format!("Organization {i} - Department")));
        if !nm.is_empty() || !title.is_empty() || !dept.is_empty() {
            orgs.push(Org { name: nm, title, dept });
        }
    }

    let phones = expand_split_value(
        numbered(row, "Phone", &["Type", "Value"], N_PHONE)
            .into_iter()
            .map(|v| Typed {
                typ: norm_type(&v["Type"]),
                value: v["Value"].clone(),
            })
            .collect(),
    );
    let emails = expand_split_value(
        numbered(row, "E-mail", &["Type", "Value"], EMAIL_MAX)
            .into_iter()
            .map(|v| Typed {
                typ: norm_type(&v["Type"]),
                value: v["Value"].clone(),
            })
            .collect(),
    );
    let websites = expand_split_value(
        numbered(row, "Website", &["Type", "Value"], WEB_MAX)
            .into_iter()
            .map(|v| Typed {
                typ: norm_type(&v["Type"]),
                value: v["Value"].clone(),
            })
            .collect(),
    );

    let mut addresses = Vec::new();
    for i in 1..=ADDR_MAX {
        let a = compose_address(row, i);
        for seg in a.split(":::") {
            let seg = clean(seg);
            if !seg.is_empty() {
                addresses.push(Address {
                    typ: norm_type(row_get(row, &format!("Address {i} - Type"))),
                    addr: seg,
                });
            }
        }
    }

    Contact {
        name,
        nick: clean(row_get(row, "Nickname")),
        birthday: clean(row_get(row, "Birthday")),
        orgs,
        phones,
        emails,
        websites,
        addresses,
        notes: split_multi(row_get(row, "Notes")),
    }
}

// ── dedup / merge ──────────────────────────────────────────────────────

fn dedupe_by<T, K, F>(items: Vec<T>, key: F) -> Vec<T>
where
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    let mut seen: HashSet<K> = HashSet::new();
    let mut out = Vec::new();
    for it in items {
        let k = key(&it);
        if !seen.contains(&k) {
            seen.insert(k);
            out.push(it);
        }
    }
    out
}

fn digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// 同人判定：(name, primary org, first phone)。
fn merge_key(c: &Contact) -> (String, String, String) {
    let org = c.orgs.first().map(|o| o.name.trim().to_lowercase()).unwrap_or_default();
    let phone = c.phones.first().map(|p| p.value.trim().to_string()).unwrap_or_default();
    (c.name.trim().to_lowercase(), org, phone)
}

fn merge_contacts(group: Vec<Contact>) -> Contact {
    let primary = group[0].clone();
    let first_nonempty = |sel: &dyn Fn(&Contact) -> String| -> String {
        group.iter().map(sel).find(|s| !s.is_empty()).unwrap_or_default()
    };
    let orgs = dedupe_by(
        group.iter().flat_map(|c| c.orgs.clone()).collect(),
        |o: &Org| (o.name.clone(), o.title.clone(), o.dept.clone()),
    );
    let phones = dedupe_by(
        group.iter().flat_map(|c| c.phones.clone()).collect(),
        |p: &Typed| digits(&p.value),
    );
    let emails = dedupe_by(
        group.iter().flat_map(|c| c.emails.clone()).collect(),
        |e: &Typed| e.value.trim().to_lowercase(),
    );
    let websites = dedupe_by(
        group.iter().flat_map(|c| c.websites.clone()).collect(),
        |w: &Typed| w.value.trim().to_lowercase(),
    );
    let addresses = dedupe_by(
        group.iter().flat_map(|c| c.addresses.clone()).collect(),
        |a: &Address| a.addr.trim().to_lowercase(),
    );
    let notes = dedupe_by(
        group.iter().flat_map(|c| c.notes.clone()).collect(),
        |n: &String| n.trim().to_string(),
    );
    Contact {
        name: primary.name,
        nick: first_nonempty(&|c| c.nick.clone()),
        birthday: first_nonempty(&|c| c.birthday.clone()),
        orgs,
        phones,
        emails,
        websites,
        addresses,
        notes,
    }
}

// ── 頁面渲染（對應 build_page）──────────────────────────────────────────

fn section(b: &mut Vec<String>, title: &str, items: &[Typed], val_key: ValKey) {
    let filtered: Vec<&Typed> = match val_key {
        ValKey::Value => items.iter().filter(|it| !it.value.is_empty()).collect(),
        ValKey::Addr => Vec::new(), // addresses 走另一條
    };
    if filtered.is_empty() {
        return;
    }
    b.push(format!("**{title}:**"));
    for it in filtered {
        let v = &it.value;
        if !it.typ.is_empty() {
            b.push(format!("- ({}) {v}", it.typ));
        } else {
            b.push(format!("- {v}"));
        }
    }
    b.push(String::new());
}

#[derive(Clone, Copy)]
enum ValKey {
    Value,
    #[allow(dead_code)]
    Addr,
}

fn section_addr(b: &mut Vec<String>, title: &str, items: &[Address]) {
    let filtered: Vec<&Address> = items.iter().filter(|it| !it.addr.is_empty()).collect();
    if filtered.is_empty() {
        return;
    }
    b.push(format!("**{title}:**"));
    for it in filtered {
        if !it.typ.is_empty() {
            b.push(format!("- ({}) {}", it.typ, it.addr));
        } else {
            b.push(format!("- {}", it.addr));
        }
    }
    b.push(String::new());
}

fn build_page(c: &Contact) -> (String, String) {
    let name = if c.name.is_empty() {
        "未命名聯絡人".to_string()
    } else {
        c.name.clone()
    };
    let mut b: Vec<String> = vec![format!("# {name}"), String::new()];

    // lead line
    let mut lead: Vec<String> = vec![name.clone()];
    let o0 = c.orgs.first();
    if let Some(o) = o0 {
        if !o.name.is_empty() {
            lead.push(format!("任職於 {}", o.name));
        }
        if !o.title.is_empty() {
            lead.push(format!("職稱 {}", o.title));
        }
    }
    if !c.nick.is_empty() {
        lead.push(format!("暱稱 {}", c.nick));
    }
    if !c.birthday.is_empty() {
        lead.push(format!("生日 {}", c.birthday));
    }
    b.push(format!("{}.", lead.join("，")));
    b.push(String::new());

    // 組織 bullets（這些是 extract_companies 建公司頁/邊的來源）
    for o in &c.orgs {
        if !o.name.is_empty() {
            b.push(format!("- 公司/組織: {}", o.name));
        }
        if !o.title.is_empty() {
            b.push(format!("- 職稱: {}", o.title));
        }
        if !o.dept.is_empty() {
            b.push(format!("- 部門: {}", o.dept));
        }
    }
    if !c.orgs.is_empty() {
        b.push(String::new());
    }

    section(&mut b, "電話", &c.phones, ValKey::Value);
    section(&mut b, "Email", &c.emails, ValKey::Value);
    section_addr(&mut b, "地址", &c.addresses);
    section(&mut b, "網站", &c.websites, ValKey::Value);

    if !c.notes.is_empty() {
        b.push("## 備註".to_string());
        b.push(String::new());
        for n in &c.notes {
            b.push(n.clone());
        }
        b.push(String::new());
    }

    let fm = frontmatter::build(&[
        ("type", "person".to_string()),
        ("title", frontmatter::yaml_single_quote(&name)),
        ("tags", "[people, contact]".to_string()),
    ]);

    // 與 Python 一致：front 區塊 + body（收尾 rstrip + 結尾換行）
    let body = b.join("\n").trim_end().to_string();
    (name, format!("{fm}{body}\n"))
}

// ── 主管線 ─────────────────────────────────────────────────────────────

/// 解析 CSV 文字（須已解碼、去 BOM）。`dedup`=true 時合併同人。
pub fn parse(content: &str, dedup: bool) -> anyhow::Result<PeopleImport> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(content.as_bytes());
    let headers = rdr.headers()?.clone();

    let mut rows_raw: Vec<Row> = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let row: Row = headers
            .iter()
            .zip(rec.iter())
            .map(|(h, v)| (h.to_string(), v.to_string()))
            .collect();
        rows_raw.push(row);
    }
    let rows_read = rows_raw.len();

    let skip_keys = [
        "Name",
        "Organization 1 - Name",
        "Phone 1 - Value",
        "E-mail 1 - Value",
        "Notes",
    ];
    let mut contacts: Vec<Contact> = Vec::new();
    let mut rows_skipped = 0usize;
    for row in &rows_raw {
        let any = skip_keys.iter().any(|k| !clean(row_get(row, k)).is_empty());
        if !any {
            rows_skipped += 1;
            continue;
        }
        contacts.push(parse_contact(row));
    }

    let mut groups_merged = 0usize;
    let mut redundant_folded = 0usize;
    if dedup {
        // 以 merge_key 分組，保留插入順序
        let mut order: Vec<(String, String, String)> = Vec::new();
        let mut buckets: HashMap<(String, String, String), Vec<Contact>> = HashMap::new();
        for c in contacts.into_iter() {
            let k = merge_key(&c);
            if !buckets.contains_key(&k) {
                order.push(k.clone());
            }
            buckets.entry(k).or_default().push(c);
        }
        let mut merged: Vec<Contact> = Vec::new();
        for k in order {
            let grp = buckets.remove(&k).unwrap();
            if grp.len() > 1 {
                groups_merged += 1;
                redundant_folded += grp.len() - 1;
            }
            merged.push(merge_contacts(grp));
        }
        contacts = merged;
    }

    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut pages: Vec<PersonPage> = Vec::new();
    for (idx, c) in contacts.iter().enumerate() {
        let (name, markdown) = build_page(c);
        let base = slug::slugify(&name, &format!("person-{}", idx + 1));
        let slug = slug::unique_slug(base, &mut seen);
        pages.push(PersonPage { slug, name, markdown });
    }

    Ok(PeopleImport {
        rows_read,
        rows_skipped,
        groups_merged,
        redundant_folded,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv_body() -> &'static str {
        // 兩列同人（name+org+phone 相同）→ dedup 合併；phone 與 email 各帶 ::: 多值。
        "Name,Organization 1 - Name,Organization 1 - Title,Phone 1 - Type,Phone 1 - Value,E-mail 1 - Value,Notes\n\
         Jane Doe,Acme,CEO,Mobile,+15550001111 ::: +15550002222,jane@acme.com ::: jane@home.com,note a\n\
         Jane Doe,Acme,CEO,Mobile,+15550001111,jane@acme.com,note b\n"
    }

    #[test]
    fn parses_and_dedups() {
        let imp = parse(csv_body(), true).unwrap();
        assert_eq!(imp.rows_read, 2);
        assert_eq!(imp.pages.len(), 1); // 合併為一人
        assert_eq!(imp.groups_merged, 1);
        let p = &imp.pages[0];
        assert_eq!(p.slug, "jane-doe");
        // 多值展開：兩支電話、兩個 email 都進來
        assert!(p.markdown.contains("+15550001111"));
        assert!(p.markdown.contains("+15550002222"));
        assert!(p.markdown.contains("jane@acme.com"));
        assert!(p.markdown.contains("jane@home.com"));
        // frontmatter
        assert!(p.markdown.starts_with("---\ntype: person\ntitle: 'Jane Doe'\ntags: [people, contact]\n---"));
        // 組織 bullet
        assert!(p.markdown.contains("- 公司/組織: Acme"));
        assert!(p.markdown.contains("- 職稱: CEO"));
        // notes 合併兩條
        assert!(p.markdown.contains("note a") && p.markdown.contains("note b"));
    }

    #[test]
    fn no_dedup_keeps_both() {
        let imp = parse(csv_body(), false).unwrap();
        assert_eq!(imp.pages.len(), 2);
    }

    #[test]
    fn primary_star_stripped_from_type() {
        let csv = "Name,Phone 1 - Type,Phone 1 - Value\nDoe,* Home,+15558887777\n";
        let imp = parse(csv, true).unwrap();
        assert!(imp.pages[0].markdown.contains("(Home) +15558887777"));
        assert!(!imp.pages[0].markdown.contains("(* Home)"));
    }

    /// 對真實 contacts.csv 做忠實度驗證（需該檔存在；以 --ignored 執行）。
    /// 預期 ~1963 頁（與 Python csv_to_people.py 的輸出量級一致）。
    #[test]
    #[ignore]
    fn real_contacts_csv() {
        let path = dirs::home_dir().unwrap().join("notes").join("contacts.csv");
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8(bytes).unwrap_or_else(|e| {
            // 退而嘗試去掉 BOM / lossy
            let strip = e.into_bytes();
            String::from_utf8_lossy(&strip).into_owned()
        });
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let imp = parse(text, true).unwrap();
        let summary = format!(
            "rows_read={} skipped={} merged={} folded={} pages={}\n",
            imp.rows_read, imp.rows_skipped, imp.groups_merged, imp.redundant_folded, imp.pages.len()
        );
        std::fs::write(std::env::temp_dir().join("gbs_csv_people_summary.txt"), &summary).ok();
        eprintln!("{summary}");
        assert!(imp.pages.len() > 1000, "頁數異常少：{}", imp.pages.len());
        // 抽查一頁的 frontmatter 合規
        let sample = &imp.pages[0];
        assert!(sample.markdown.starts_with("---\ntype: person\n"));
        assert!(sample.markdown.contains("tags: [people, contact]"));
    }

    /// 黃金比對：Rust 產出 vs 磁碟上既有（Python 產生）的 people/*.md，逐檔比對。
    /// 以 --ignored 執行；結果寫到 temp，便於在 RTK 環境讀取。
    #[test]
    #[ignore]
    fn parity_vs_existing_people_files() {
        let notes = dirs::home_dir().unwrap().join("notes");
        let csv = std::fs::read_to_string(notes.join("contacts.csv")).unwrap();
        let csv = csv.strip_prefix('\u{feff}').unwrap_or(&csv);
        let imp = parse(csv, true).unwrap();

        let mut gen: HashMap<String, String> = HashMap::new();
        for p in &imp.pages {
            gen.insert(p.slug.clone(), p.markdown.clone());
        }

        let people_dir = notes.join("people");
        let mut exact = 0usize;
        let mut differ = 0usize;
        let mut norm_match = 0usize; // 換行正規化（\r\n → \n）後一致
        let mut only_rust = 0usize;
        let mut only_disk = 0usize;
        let mut diffs_sample: Vec<String> = Vec::new();
        let norm = |s: &str| s.replace("\r\n", "\n");
        for (slug, md) in &gen {
            let path = people_dir.join(format!("{slug}.md"));
            match std::fs::read_to_string(&path) {
                Ok(existing) => {
                    if existing == *md {
                        exact += 1;
                    }
                    if norm(&existing) == norm(md) {
                        norm_match += 1;
                    }
                    if existing != *md && norm(&existing) != norm(md) {
                        differ += 1;
                        if diffs_sample.len() < 5 {
                            diffs_sample.push(slug.clone());
                        }
                    }
                }
                Err(_) => only_rust += 1,
            }
        }
        let disk_files: Vec<String> = std::fs::read_dir(&people_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .filter_map(|e| e.path().file_stem().and_then(|s| s.to_str()).map(String::from))
            .collect();
        only_disk = disk_files.iter().filter(|s| !gen.contains_key(*s)).count();

        let summary = format!(
            "rust_pages={} exact_match={} norm_match={} real_differ={} only_rust={} only_disk={} diff_sample={:?}\n",
            gen.len(),
            exact,
            norm_match,
            differ,
            only_rust,
            only_disk,
            diffs_sample
        );
        std::fs::write(std::env::temp_dir().join("gbs_parity.txt"), &summary).ok();
        eprintln!("{summary}");
        // 正規化換行後應幾乎全一致（容許少數被手工編輯）
        assert!(norm_match > 1900, "正規化後一致太少：{summary}");
    }
}
