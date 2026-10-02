//! 知識政策評估（M1-WP-C3＋C7 條件擴充）——確定性純函式，**LLM 永不參與**（I3／提示詞 Rule 6）。
//!
//! 評估序：規則按 `priority` 升冪掃描，先到先得——DENY 命中即拒、ALLOW 命中即准、
//! 無匹配＝DENY（I2 fail closed）。呼叫端不得自稱身份：`AccessContext` 只由伺服器端
//! 構造（I4），本函式因此可以信任 `ctx` 的內容。
//!
//! 條件面（D-C7a，全 `None`/`false`＝不限）：
//! - principal 側：`principals`／`principal_types`／`departments`（交集）／`projects`（交集）
//! - 資源側：`scopes`（id 白名單）／`classifications`（分類白名單）
//! - 成員制：`department_membership`（`scope.department ∈ ctx.departments`）、
//!   `project_membership`（`scope.project ∈ ctx.projects`）——一條規則把各部門 scope
//!   授給該部門成員（Test 2 完整版的地基）。
//!
//! 兩個進入點：
//! - [`evaluate`]：單一 scope 的准／拒判定。
//! - [`authorized_sources`]／[`authorized_scope_ids`]：授權鏈（scope 鏈→source 集合），
//!   C6 KnowledgeService 的候選搜尋空間構造。

use super::types::{AccessContext, Decision, KnowledgePolicy, KnowledgeScope, PrincipalType, PolicyRule};

/// 單條件白名單比對：`None`＝不限；`Some(list)`＝須命中其中之一。
fn matches<T: PartialEq>(cond: &Option<Vec<T>>, value: Option<&T>) -> bool {
    match cond {
        None => true,
        Some(list) => match value {
            Some(v) => list.contains(v),
            None => false,
        },
    }
}

/// principal 側交集條件：`None`＝不限；`Some(list)`＝`ctx 屬性 ∩ list ≠ ∅`。
fn intersects(cond: &Option<Vec<String>>, values: &[String]) -> bool {
    match cond {
        None => true,
        Some(list) => values.iter().any(|v| list.contains(v)),
    }
}

fn rule_matches(rule: &PolicyRule, ctx: &AccessContext, scope: &KnowledgeScope) -> bool {
    let principals = matches(&rule.principals, Some(&ctx.principal_id));
    let principal_types = matches(&rule.principal_types, Some(&ctx.principal_type));
    let scopes = match &rule.scopes {
        None => true,
        Some(list) => list.iter().any(|s| s == &scope.id),
    };
    // C7：principal 側屬性交集
    let departments = intersects(&rule.departments, &ctx.departments);
    let projects = intersects(&rule.projects, &ctx.projects);
    // C7：資源側分類白名單
    let classifications = matches(&rule.classifications, Some(&scope.classification));
    // C7：成員制（scope 無歸屬欄位時不命中——從嚴）
    let dep_member = !rule.department_membership
        || match (&scope.department, ctx.departments.is_empty()) {
            (Some(d), false) => ctx.departments.contains(d),
            _ => false,
        };
    let proj_member = !rule.project_membership
        || match (&scope.project, ctx.projects.is_empty()) {
            (Some(p), false) => ctx.projects.contains(p),
            _ => false,
        };
    principals && principal_types && scopes && departments && projects && classifications
        && dep_member && proj_member
}

/// 評估單一 scope 的存取決策（I2：無匹配＝DENY）。
pub fn evaluate(policy: &KnowledgePolicy, ctx: &AccessContext, scope: &KnowledgeScope) -> Decision {
    let mut rules: Vec<&PolicyRule> = policy.rules.iter().collect();
    rules.sort_by_key(|r| r.priority);
    for rule in rules {
        if rule_matches(rule, ctx, scope) {
            return match rule.effect {
                super::types::Effect::Allow => Decision::Allow,
                super::types::Effect::Deny => Decision::Deny {
                    reason: format!("denied_by_rule:{}", rule.id),
                },
            };
        }
    }
    Decision::Deny {
        reason: "default_deny".to_string(),
    }
}

/// 授權 scope id 集合（receipt 的 Scope 鏈；id 排序）。
pub fn authorized_scope_ids(
    policy: &KnowledgePolicy,
    ctx: &AccessContext,
    scopes: &[KnowledgeScope],
) -> Vec<String> {
    let mut out: Vec<String> = scopes
        .iter()
        .filter(|s| matches!(evaluate(policy, ctx, s), Decision::Allow))
        .map(|s| s.id.clone())
        .collect();
    out.sort();
    out
}

/// 候選搜尋空間構造（I1）：policy＋全部 scopes→該 AccessContext 可檢索的 source 集合。
///
/// 回傳去重排序後的 source id 清單（空集合＝呼叫端必須 DENY，不得以任何形式檢索）。
pub fn authorized_sources(
    policy: &KnowledgePolicy,
    ctx: &AccessContext,
    scopes: &[KnowledgeScope],
) -> Vec<String> {
    let allowed: Vec<&KnowledgeScope> = scopes
        .iter()
        .filter(|s| matches!(evaluate(policy, ctx, s), Decision::Allow))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for scope in allowed {
        for sid in &scope.source_ids {
            if !out.contains(sid) {
                out.push(sid.clone());
            }
        }
    }
    out.sort();
    out
}

/// 檢查某 principal_type 是否被任一 `principal_types` 條件接受（輔助測試與 C4）。
#[allow(dead_code)] // C4/C5 接線時消費；C3 先固化語意與測試。
pub fn type_in(cond: &Option<Vec<PrincipalType>>, t: PrincipalType) -> bool {
    matches(cond, Some(&t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{Effect, PrincipalAttrs};

    fn ctx(pid: &str, t: PrincipalType) -> AccessContext {
        AccessContext {
            principal_id: pid.to_string(),
            principal_type: t,
            employee_id: None,
            workspace_id: "ws".into(),
            roles: vec![],
            departments: vec![],
            projects: vec![],
            task_id: None,
            purpose: None,
        }
    }

    fn ctx_dep(pid: &str, t: PrincipalType, departments: &[&str]) -> AccessContext {
        AccessContext {
            departments: departments.iter().map(|s| s.to_string()).collect(),
            ..ctx(pid, t)
        }
    }

    fn policy(rules: Vec<PolicyRule>) -> KnowledgePolicy {
        KnowledgePolicy { version: 1, rules }
    }

    fn scope(id: &str) -> KnowledgeScope {
        KnowledgeScope {
            id: id.into(),
            visibility: super::super::types::Visibility::Company,
            classification: "internal".into(),
            source_ids: vec![format!("src-{id}")],
            owner: None,
            department: None,
            project: None,
        }
    }

    fn scope_dep(id: &str, department: &str) -> KnowledgeScope {
        KnowledgeScope {
            department: Some(department.into()),
            ..scope(id)
        }
    }

    fn rule(id: &str, priority: i64, effect: Effect) -> PolicyRule {
        PolicyRule {
            id: id.into(),
            priority,
            effect,
            principals: None,
            principal_types: None,
            scopes: None,
            departments: None,
            projects: None,
            classifications: None,
            department_membership: false,
            project_membership: false,
        }
    }

    #[test]
    fn no_match_is_default_deny() {
        let p = policy(vec![]);
        assert_eq!(
            evaluate(&p, &ctx("p1", PrincipalType::Human), &scope("scope-a")),
            Decision::Deny { reason: "default_deny".into() }
        );
    }

    #[test]
    fn allow_rule_grants() {
        let mut r = rule("r1", 1, Effect::Allow);
        r.principals = Some(vec!["p1".into()]);
        r.scopes = Some(vec!["scope-a".into()]);
        let p = policy(vec![r]);
        assert_eq!(evaluate(&p, &ctx("p1", PrincipalType::Human), &scope("scope-a")), Decision::Allow);
        // scope 不在白名單 → default deny
        assert!(matches!(evaluate(&p, &ctx("p1", PrincipalType::Human), &scope("scope-b")),
            Decision::Deny { .. }));
    }

    #[test]
    fn deny_precedes_allow_by_priority() {
        // DENY priority 較大（後評估）也擋不住先命中的 ALLOW；反過來 DENY 先命中即拒。
        let mut d = rule("d", 1, Effect::Deny);
        d.principals = Some(vec!["p1".into()]);
        let deny_first = policy(vec![d, rule("a", 2, Effect::Allow)]);
        assert!(matches!(evaluate(&deny_first, &ctx("p1", PrincipalType::Human), &scope("s")),
            Decision::Deny { .. }));
        // 同一條 DENY 對別人無效 → 落到 ALLOW
        assert_eq!(evaluate(&deny_first, &ctx("p2", PrincipalType::Human), &scope("s")), Decision::Allow);
    }

    #[test]
    fn principal_type_condition() {
        let mut r = rule("r", 1, Effect::Allow);
        r.principal_types = Some(vec![PrincipalType::Human]);
        let p = policy(vec![r]);
        assert_eq!(evaluate(&p, &ctx("h", PrincipalType::Human), &scope("s")), Decision::Allow);
        assert!(matches!(evaluate(&p, &ctx("a", PrincipalType::AiEmployee), &scope("s")),
            Decision::Deny { .. }));
    }

    #[test]
    fn c7_department_membership_grants_members_only() {
        // 一條成員制 allow：各部門 scope 授給該部門成員（Test 2 完整版的核心語意）。
        let mut r = rule("member", 1, Effect::Allow);
        r.department_membership = true;
        let p = policy(vec![r]);
        let quality = scope_dep("dept-quality", "quality");
        let mfg = scope_dep("dept-mfg", "manufacturing");
        // quality 成員 → 命中 dept-quality、不命中 dept-mfg
        let q_member = ctx_dep("u1", PrincipalType::AiEmployee, &["quality"]);
        assert_eq!(evaluate(&p, &q_member, &quality), Decision::Allow);
        assert!(matches!(evaluate(&p, &q_member, &mfg), Decision::Deny { .. }));
        // 無部門屬性者 → 成員制不命中（ctx.departments 空＝從嚴）
        assert!(matches!(evaluate(&p, &ctx("u2", PrincipalType::AiEmployee), &quality),
            Decision::Deny { .. }));
        // scope 無 department 欄位 → 成員制不命中（從嚴）
        assert!(matches!(evaluate(&p, &q_member, &scope("co-common")), Decision::Deny { .. }));
    }

    #[test]
    fn c7_principal_side_intersection_condition() {
        let mut r = rule("rd-only", 1, Effect::Allow);
        r.departments = Some(vec!["rd".into(), "research".into()]);
        let p = policy(vec![r]);
        assert_eq!(evaluate(&p, &ctx_dep("u1", PrincipalType::AiEmployee, &["rd", "backend"]), &scope("s")), Decision::Allow);
        assert!(matches!(evaluate(&p, &ctx_dep("u2", PrincipalType::AiEmployee, &["sales"]), &scope("s")),
            Decision::Deny { .. }));
        // 屬性全空 → 交集條件不命中
        assert!(matches!(evaluate(&p, &ctx("u3", PrincipalType::AiEmployee), &scope("s")),
            Decision::Deny { .. }));
    }

    #[test]
    fn c7_classification_resource_condition() {
        let mut confidential = scope("restricted-hr");
        confidential.classification = "confidential".into();
        let mut r = rule("conf", 1, Effect::Allow);
        r.classifications = Some(vec!["confidential".into()]);
        let p = policy(vec![r]);
        assert_eq!(evaluate(&p, &ctx("u", PrincipalType::Human), &confidential), Decision::Allow);
        assert!(matches!(evaluate(&p, &ctx("u", PrincipalType::Human), &scope("s")),
            Decision::Deny { .. }));
    }

    #[test]
    fn authorized_sources_dedup_sorted_and_fail_closed() {
        let scopes = vec![
            KnowledgeScope { id: "scope-a".into(), visibility: super::super::types::Visibility::Company,
                classification: "internal".into(), source_ids: vec!["src-b".into(), "src-a".into()],
                owner: None, department: None, project: None },
            KnowledgeScope { id: "scope-b".into(), visibility: super::super::types::Visibility::Project,
                classification: "internal".into(), source_ids: vec!["src-c".into()],
                owner: None, department: None, project: None },
        ];
        let mut r = rule("r", 1, Effect::Allow);
        r.scopes = Some(vec!["scope-a".into()]);
        let allow_a = policy(vec![r]);
        // 空 policy＝全 DENY＝空集合（fail closed）
        assert!(authorized_sources(&policy(vec![]), &ctx("p", PrincipalType::Human), &scopes).is_empty());
        assert_eq!(
            authorized_sources(&allow_a, &ctx("p", PrincipalType::Human), &scopes),
            vec!["src-a".to_string(), "src-b".to_string()]
        );
        // scope 鏈（C7）：receipt 的 Scope 面可稽核
        assert_eq!(authorized_scope_ids(&allow_a, &ctx("p", PrincipalType::Human), &scopes), vec!["scope-a".to_string()]);
    }

    #[test]
    fn attrs_default_roundtrip() {
        // PrincipalAttrs serde default：舊 JSON（無 attrs 欄）解碼不炸（零遷移）。
        let old = r#"{"id":"p","principal_type":"human","display_name":""}"#;
        let p: super::super::types::Principal = serde_json::from_str(old).unwrap();
        assert!(p.attrs.departments.is_empty());
        let with_attrs = super::super::types::Principal {
            attrs: PrincipalAttrs { roles: vec![], departments: vec!["quality".into()], projects: vec![] },
            ..p
        };
        assert!(serde_json::to_string(&with_attrs).unwrap().contains("quality"));
    }
}
