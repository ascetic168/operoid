//! 知識政策評估（M1-WP-C3）——確定性純函式，**LLM 永不參與**（I3／提示詞 Rule 6）。
//!
//! 評估序：規則按 `priority` 升冪掃描，先到先得——DENY 命中即拒、ALLOW 命中即准、
//! 無匹配＝DENY（I2 fail closed）。呼叫端不得自稱身份：`AccessContext` 只由伺服器端
//! 構造（I4），本函式因此可以信任 `ctx` 的內容。
//!
//! 兩個進入點：
//! - [`evaluate`]：單一 scope 的准／拒判定。
//! - [`authorized_sources`]：policy＋scopes→授權 source 集合（C6 KnowledgeService 的
//!   候選搜尋空間構造——I1 檢索前授權的核心步驟）。

use super::types::{AccessContext, Decision, KnowledgePolicy, KnowledgeScope, PrincipalType};

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

fn rule_matches(
    rule: &super::types::PolicyRule,
    ctx: &AccessContext,
    scope_id: &str,
) -> bool {
    let principals = matches(&rule.principals, Some(&ctx.principal_id));
    let principal_types = matches(
        &rule.principal_types,
        Some(&ctx.principal_type),
    );
    let scopes = match &rule.scopes {
        None => true,
        Some(list) => list.iter().any(|s| s == scope_id),
    };
    principals && principal_types && scopes
}

/// 評估單一 scope 的存取決策（I2：無匹配＝DENY）。
pub fn evaluate(policy: &KnowledgePolicy, ctx: &AccessContext, scope_id: &str) -> Decision {
    let mut rules: Vec<&super::types::PolicyRule> = policy.rules.iter().collect();
    rules.sort_by_key(|r| r.priority);
    for rule in rules {
        if rule_matches(rule, ctx, scope_id) {
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

/// 候選搜尋空間構造（I1）：policy＋全部 scopes→該 AccessContext 可檢索的 source 集合。
///
/// 回傳去重排序後的 source id 清單（空集合＝呼叫端必須 DENY，不得以任何形式檢索）。
pub fn authorized_sources(
    policy: &KnowledgePolicy,
    ctx: &AccessContext,
    scopes: &[KnowledgeScope],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for scope in scopes {
        if matches!(
            evaluate(policy, ctx, &scope.id),
            Decision::Allow
        ) {
            for sid in &scope.source_ids {
                if !out.contains(sid) {
                    out.push(sid.clone());
                }
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
    use super::super::types::{Effect, PolicyRule};

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

    fn policy(rules: Vec<PolicyRule>) -> KnowledgePolicy {
        KnowledgePolicy { version: 1, rules }
    }

    #[test]
    fn no_match_is_default_deny() {
        let p = policy(vec![]);
        assert_eq!(
            evaluate(&p, &ctx("p1", PrincipalType::Human), "scope-a"),
            Decision::Deny { reason: "default_deny".into() }
        );
    }

    #[test]
    fn allow_rule_grants() {
        let p = policy(vec![PolicyRule {
            id: "r1".into(),
            priority: 1,
            effect: Effect::Allow,
            principals: Some(vec!["p1".into()]),
            principal_types: None,
            scopes: Some(vec!["scope-a".into()]),
        }]);
        assert_eq!(evaluate(&p, &ctx("p1", PrincipalType::Human), "scope-a"), Decision::Allow);
        // scope 不在白名單 → default deny
        assert!(matches!(evaluate(&p, &ctx("p1", PrincipalType::Human), "scope-b"),
            Decision::Deny { .. }));
    }

    #[test]
    fn deny_precedes_allow_by_priority() {
        // DENY priority 較大（後評估）也擋不住先命中的 ALLOW；反過來 DENY 先命中即拒。
        let deny_first = policy(vec![
            PolicyRule { id: "d".into(), priority: 1, effect: Effect::Deny,
                principals: Some(vec!["p1".into()]), principal_types: None, scopes: None },
            PolicyRule { id: "a".into(), priority: 2, effect: Effect::Allow,
                principals: None, principal_types: None, scopes: None },
        ]);
        assert!(matches!(evaluate(&deny_first, &ctx("p1", PrincipalType::Human), "s"),
            Decision::Deny { .. }));
        // 同一條 DENY 對別人無效 → 落到 ALLOW
        assert_eq!(evaluate(&deny_first, &ctx("p2", PrincipalType::Human), "s"), Decision::Allow);
    }

    #[test]
    fn principal_type_condition() {
        let p = policy(vec![PolicyRule {
            id: "r".into(), priority: 1, effect: Effect::Allow,
            principals: None,
            principal_types: Some(vec![PrincipalType::Human]),
            scopes: None,
        }]);
        assert_eq!(evaluate(&p, &ctx("h", PrincipalType::Human), "s"), Decision::Allow);
        assert!(matches!(evaluate(&p, &ctx("a", PrincipalType::AiEmployee), "s"),
            Decision::Deny { .. }));
    }

    #[test]
    fn authorized_sources_dedup_sorted_and_fail_closed() {
        let scopes = vec![
            KnowledgeScope { id: "scope-a".into(), visibility: super::super::types::Visibility::Company,
                classification: "internal".into(), source_ids: vec!["src-b".into(), "src-a".into()], owner: None },
            KnowledgeScope { id: "scope-b".into(), visibility: super::super::types::Visibility::Project,
                classification: "internal".into(), source_ids: vec!["src-c".into()], owner: None },
        ];
        let allow_a = policy(vec![PolicyRule {
            id: "r".into(), priority: 1, effect: Effect::Allow,
            principals: None, principal_types: None, scopes: Some(vec!["scope-a".into()]),
        }]);
        // 空 policy＝全 DENY＝空集合（fail closed）
        assert!(authorized_sources(&policy(vec![]), &ctx("p", PrincipalType::Human), &scopes).is_empty());
        assert_eq!(
            authorized_sources(&allow_a, &ctx("p", PrincipalType::Human), &scopes),
            vec!["src-a".to_string(), "src-b".to_string()]
        );
    }
}
