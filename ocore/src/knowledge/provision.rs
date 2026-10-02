//! 寫入端自動供給（C13c，D-C13g）——人類只選「**圈子×等級**」，系統推導並自動建立
//! scope＋source，人類永遠不做「部門×等級」矩陣簿記。
//!
//! 慣例（D-C13g）：provisioned 的 scope 與 source **1:1 同名**
//! （`co-{level}`／`dept-{circle}-{level}`／`proj-{circle}-{level}`；GBrain source id
//! 限定 `[a-z0-9-]{1,32}`）；等級决定 visibility（Secret→Restricted）；source 目錄落在
//! notes repo 旁的 `scopes/` 下，種子 README＋git（供 sync 流使用）。
//!
//! 語意（D-C13i）：寫入端天花板與 I9 對稱——作者 clearance ≥ 目標等級才可寫；
//! scope.owner＝發起人 principal，供給時自動合成 owner 白名單 allow 規則（作者至少
//! 讀得到自己的產出）＋`grant` 不可破天花板照舊。

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use super::types::{Effect, KnowledgeScope, PolicyRule, SecurityLevel, Visibility};
use super::bootstrap;
use crate::app_config::AppConfig;
use crate::domain::store::Store;

/// 圈子種類——「這批知識屬於哪個圈子」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircleKind {
    Company,
    Department,
    Project,
}

/// 寫入目標（供給完成後）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteTarget {
    pub scope_id: String,
    pub source_id: String,
    /// 該 source 的檔案目錄（repo 流寫入點；capture 流僅供溯源）。
    pub dir: PathBuf,
}

/// 圈子值正規化：小寫、非 `[a-z0-9]` 連續段→`-`、去頭尾 `-`、上限 16 字元。
pub fn sanitize_circle(value: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            let lower = ch.to_ascii_lowercase();
            out.push(lower);
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("misc");
    }
    out.truncate(16);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// scope/source id 慣例（D-C13g）：Company→`co-{level}`、Department→`dept-{circle}-{level}`、
/// Project→`proj-{circle}-{level}`；總長夾在 GBrain source id 上限 32 內。
pub fn derive_scope_id(kind: CircleKind, circle: &str, level: SecurityLevel) -> String {
    let circle = sanitize_circle(circle);
    let level = format!("{level:?}").to_lowercase();
    let prefix = match kind {
        CircleKind::Company => "co",
        CircleKind::Department => "dept",
        CircleKind::Project => "proj",
    };
    let base = match kind {
        CircleKind::Company => format!("{prefix}-{level}"),
        _ => format!("{prefix}-{circle}-{level}"),
    };
    if base.len() <= 32 {
        return base;
    }
    let keep = 32 - level.len() - prefix.len() - 2;
    format!("{prefix}-{}-{level}", sanitize_circle(&circle[..keep.min(circle.len())]))
}

/// 等級 → visibility：Secret 一律 Restricted；其餘隨圈子種類。
pub fn visibility_for(kind: CircleKind, level: SecurityLevel) -> Visibility {
    if level == SecurityLevel::Secret {
        return Visibility::Restricted;
    }
    match kind {
        CircleKind::Company => Visibility::Company,
        CircleKind::Department => Visibility::Department,
        CircleKind::Project => Visibility::Project,
    }
}

/// 既有 scope 查找（供給前先找——冪等）：kind/value/level 全符合即命中。
pub fn find_scope_for(
    store: &dyn Store,
    kind: CircleKind,
    circle: &str,
    level: SecurityLevel,
) -> Result<Option<KnowledgeScope>> {
    let circle_norm = sanitize_circle(circle);
    Ok(store
        .list_scopes()?
        .into_iter()
        .find(|s| {
            s.classification == level
                && match kind {
                    CircleKind::Company => s.visibility == Visibility::Company,
                    CircleKind::Department => {
                        s.department.as_deref() == Some(circle_norm.as_str())
                    }
                    CircleKind::Project => s.project.as_deref() == Some(circle_norm.as_str()),
                }
        }))
}

/// provisioned source 的目錄位置：notes repo 旁的 `scopes/<source_id>`。
pub fn scoped_dir(cfg: &AppConfig, source_id: &str) -> PathBuf {
    let root = std::path::Path::new(&cfg.notes_repo_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    root.join("scopes").join(source_id)
}

/// 供給（冪等）：既有 scope 直接回傳；否則建目錄（種子 README＋git）＋
/// `sources add --force`（Public→federated，其餘 no-federated）＋註冊 scope＋
/// 合成 owner 白名單規則（D-C13i）。回傳寫入目標。
pub async fn resolve_write_target(
    store: &dyn Store,
    cfg: &AppConfig,
    kind: CircleKind,
    circle: &str,
    level: SecurityLevel,
    owner_principal: &str,
) -> Result<WriteTarget> {
    if let Some(existing) = find_scope_for(store, kind, circle, level)? {
        let scope_id = existing.id.clone();
        let source_id = existing
            .source_ids
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("scope {} 未綁任何 source", scope_id))?;
        let dir = scoped_dir(cfg, &source_id);
        return Ok(WriteTarget { scope_id, source_id, dir });
    }

    let scope_id = derive_scope_id(kind, circle, level);
    let source_id = scope_id.clone();
    let dir = scoped_dir(cfg, &source_id);

    // 目錄＋種子 README＋git（sync 流的必要條件——M0 實測）。
    std::fs::create_dir_all(&dir)?;
    let readme = dir.join("README.md");
    if !readme.exists() {
        std::fs::write(
            &readme,
            format!(
                "# {source_id}\n\nAuto-provisioned by Operoid C13c（圈子×等級寫入供給）。\n"
            ),
        )?;
    }
    let git = |args: &[&str]| -> Result<()> {
        let st = std::process::Command::new("git")
            .current_dir(&dir)
            .args(args)
            .status()?;
        assert!(st.success(), "git {args:?} 失敗");
        Ok(())
    };
    if !dir.join(".git").exists() {
        git(&["init", "-q"])?;
    }
    git(&["add", "-A"])?;
    // 無變更時 commit 非零可容忍（種子已 add；首建必有變更）。
    let _ = git(&[
        "-c",
        "user.email=operoid@local",
        "-c",
        "user.name=operoid",
        "commit",
        "-qm",
        "provision",
    ]);

    // 註冊 source：Public→federated，其餘 no-federated（受限知識不進預設搜尋——D1 雙保險）。
    let federated_flag = if level == SecurityLevel::Public {
        "--federated"
    } else {
        "--no-federated"
    };
    let exe = cfg.gbrain_exe_path.clone();
    let env = crate::proc::env_for_brain(cfg.active_env_home());
    let dir_s = dir.to_string_lossy().to_string();
    let (code, _, err) = crate::gbrain_cli::run_capture(
        &exe,
        &["sources", "add", &source_id, "--path", &dir_s, "--force", federated_flag],
        &env,
    )
    .await?;
    if code != 0 {
        return Err(anyhow!("sources add {source_id} 失敗：{}", err.trim()));
    }

    // 註冊 scope（含稽核事件）。
    bootstrap::save_scope_with_event(
        store,
        &KnowledgeScope {
            id: scope_id.clone(),
            visibility: visibility_for(kind, level),
            classification: level,
            source_ids: vec![source_id.clone()],
            owner: Some(owner_principal.to_string()),
            department: match kind {
                CircleKind::Department => Some(sanitize_circle(circle)),
                _ => None,
            },
            project: match kind {
                CircleKind::Project => Some(sanitize_circle(circle)),
                _ => None,
            },
        },
    )?;

    // D-C13i：自動合成 owner 白名單 allow 規則（作者至少讀得到自己的產出）。
    let rule_id = format!("owner-{}-{}", scope_id, owner_principal);
    let (policy, _) = bootstrap::load_policy_fail_closed(store);
    let already = policy
        .rules
        .iter()
        .any(|r| r.id == rule_id && r.effect == Effect::Allow);
    if !already {
        let mut rules = policy.rules;
        rules.push(PolicyRule {
            id: rule_id,
            priority: 5,
            effect: Effect::Allow,
            principals: Some(vec![owner_principal.to_string()]),
            principal_types: None,
            scopes: Some(vec![scope_id.clone()]),
            departments: None,
            projects: None,
            classifications: None,
            department_membership: false,
            project_membership: false,
        });
        bootstrap::save_policy_new_version(store, rules)?;
    }

    Ok(WriteTarget { scope_id, source_id, dir })
}

/// repo 流：寫檔後 add＋commit（無變更可容忍）——sync 只吃已提交內容（M0 實測）。
pub fn commit_dir(dir: &std::path::Path) -> Result<()> {
    let git = |args: &[&str]| -> Result<()> {
        let st = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()?;
        assert!(st.success(), "git {args:?} 失敗");
        Ok(())
    };
    git(&["add", "-A"])?;
    let _ = git(&[
        "-c",
        "user.email=operoid@local",
        "-c",
        "user.name=operoid",
        "commit",
        "-qm",
        "content",
    ]);
    Ok(())
}

/// 找出涵蓋某 source 的 scope（寫入端天花的資源側查找）。
pub fn scope_covering_source<'a>(
    scopes: &'a [KnowledgeScope],
    source_id: &str,
) -> Option<&'a KnowledgeScope> {
    scopes.iter().find(|s| s.source_ids.iter().any(|sid| sid == source_id))
}

/// 由 repo 路徑反查作用中腦的 source id（路徑正規化：斜線統一、去尾、小寫）。
pub async fn source_id_for_repo(cfg: &AppConfig, repo_path: &str) -> Result<Option<String>> {
    let brain_id = cfg
        .active_brain_id
        .clone()
        .unwrap_or(crate::app_config::DEFAULT_BRAIN_ID.to_string());
    let sources = crate::brains::list_sources(cfg, &brain_id).await?;
    let norm = |p: &str| p.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let target = norm(repo_path);
    Ok(sources
        .into_iter()
        .find(|s| s.local_path.as_deref().map(|p| norm(p) == target).unwrap_or(false))
        .map(|s| s.id))
}

/// C13c（D-C13i）：**寫入端天花板**——repo 所屬 scope 的等級超過作者 clearance → 拒。
/// source 未被任何 scope 涵蓋（未分類）→ 放行（co-common bootstrap 涵蓋全部 sources，
/// 正常流程不會發生；存活舊資料為 Internal 語意）。回傳 AppError 供 HTTP 直接映射。
pub async fn enforce_write_ceiling(
    cfg: &AppConfig,
    store: &dyn Store,
    owner_principal: &str,
    repo_path: &str,
) -> core::result::Result<(), crate::i18n::AppError> {
    use crate::i18n::AppError;
    let Some(source_id) = source_id_for_repo(cfg, repo_path)
        .await
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?
    else {
        return Ok(());
    };
    let scopes = store
        .list_scopes()
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?;
    let Some(scope) = scope_covering_source(&scopes, &source_id) else {
        return Ok(());
    };
    let clearance = store
        .get_principal(owner_principal)
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?
        .and_then(|p| p.attrs.clearance)
        .unwrap_or(SecurityLevel::Internal);
    if scope.classification > clearance {
        return Err(AppError::new("knowledge.writeAboveClearance")
            .p("level", format!("{:?}", scope.classification).to_lowercase())
            .p("clearance", format!("{clearance:?}").to_lowercase())
            .p("scope", &scope.id));
    }
    Ok(())
}

/// capture 流寫入：`gbrain capture --file <f> --source <id>`（直寫 DB，免 repo sync）。
pub async fn capture_into_source(
    exe: &str,
    home: Option<&str>,
    file: &std::path::Path,
    source_id: &str,
) -> Result<()> {
    let file_s = file.to_string_lossy().to_string();
    let (code, _, err) = crate::gbrain_cli::run_capture(
        exe,
        &["capture", "--file", &file_s, "--source", source_id, "--type", "note", "--quiet"],
        &crate::proc::env_for_brain(Some(home.unwrap_or_default())),
    )
    .await?;
    if code != 0 {
        return Err(anyhow!("capture → {source_id} 失敗：{}", err.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SqliteStore;

    fn store() -> SqliteStore {
        let dir = std::env::temp_dir().join(format!(
            "c13c-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        SqliteStore::open(&dir.join("test.db")).unwrap()
    }

    #[test]
    fn sanitize_and_derive() {
        assert_eq!(sanitize_circle("品質部 Quality!"), "quality");
        assert_eq!(sanitize_circle("  R&D -- 部門 "), "r-d");
        assert_eq!(sanitize_circle("///"), "misc");
        assert_eq!(
            derive_scope_id(CircleKind::Company, "anything", SecurityLevel::Internal),
            "co-internal"
        );
        assert_eq!(
            derive_scope_id(CircleKind::Department, "Quality", SecurityLevel::Confidential),
            "dept-quality-confidential"
        );
        assert_eq!(
            derive_scope_id(CircleKind::Project, "CPO-800G", SecurityLevel::Internal),
            "proj-cpo-800g-internal"
        );
        // 32 字元上限：超長圈子被夾斷仍合字元集
        let long = derive_scope_id(
            CircleKind::Department,
            "a-very-long-department-name-beyond-limits",
            SecurityLevel::Secret,
        );
        assert!(long.len() <= 32);
        assert!(long.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()));
    }

    #[test]
    fn visibility_secret_always_restricted() {
        assert_eq!(visibility_for(CircleKind::Company, SecurityLevel::Secret), Visibility::Restricted);
        assert_eq!(visibility_for(CircleKind::Department, SecurityLevel::Internal), Visibility::Department);
    }

    #[test]
    fn find_scope_matches_circle_and_level() {
        let s = store();
        crate::knowledge::bootstrap::bootstrap_with_sources(&s, &["src-a".into()]).unwrap();
        use crate::domain::Store as _;
        s.put_scope(&KnowledgeScope {
            id: "dept-quality-confidential".into(),
            visibility: Visibility::Department,
            classification: SecurityLevel::Confidential,
            source_ids: vec!["src-qc".into()],
            owner: None,
            department: Some("quality".into()),
            project: None,
        })
        .unwrap();

        // 命中：品質 × Confidential
        let hit = find_scope_for(&s, CircleKind::Department, "Quality", SecurityLevel::Confidential)
            .unwrap()
            .expect("應命中既有 scope");
        assert_eq!(hit.id, "dept-quality-confidential");
        // 不命中：品質 × Secret / 行銷 × Confidential / 公司 × Confidential
        assert!(find_scope_for(&s, CircleKind::Department, "quality", SecurityLevel::Secret).unwrap().is_none());
        assert!(find_scope_for(&s, CircleKind::Department, "marketing", SecurityLevel::Confidential).unwrap().is_none());
        assert!(find_scope_for(&s, CircleKind::Company, "company", SecurityLevel::Confidential).unwrap().is_none());
    }
}
