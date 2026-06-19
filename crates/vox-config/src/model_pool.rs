//! Operator-curated allowed-model pool. Pure data + predicate; persistence is via the
//! `model_pool` field on `VoxConfig` (single writer to ~/.vox/config.toml).
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PoolRule {
    Free,
    Provider {
        value: String,
    },
    MaxCostPer1k {
        value: f64,
    },
    Tier {
        value: String,
    },
    MinContext {
        value: u64,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelPool {
    pub rules: Vec<PoolRule>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub disabled_sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PoolModelView {
    pub id: String,
    pub provider: String,
    pub cost_per_1k: f64,
    pub max_tokens: u64,
    pub is_free: bool,
    pub tier: String,
}

pub fn rule_matches(rule: &PoolRule, m: &PoolModelView) -> bool {
    match rule {
        PoolRule::Free => m.is_free || m.cost_per_1k == 0.0,
        PoolRule::Provider { value } => m.provider.eq_ignore_ascii_case(value),
        PoolRule::MaxCostPer1k { value } => m.cost_per_1k <= *value,
        PoolRule::Tier { value } => m.tier.eq_ignore_ascii_case(value),
        PoolRule::MinContext { value } => m.max_tokens >= *value,
        PoolRule::Unknown => false,
    }
}

pub fn resolve(
    pool: &ModelPool,
    catalog: &[PoolModelView],
    enabled: &BTreeSet<String>,
) -> BTreeSet<String> {
    let open = pool.rules.is_empty() && pool.includes.is_empty();
    catalog
        .iter()
        .filter(|m| enabled.contains(&m.provider))
        .filter(|m| {
            !pool
                .disabled_sources
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&m.provider))
        })
        .filter(|m| !pool.excludes.contains(&m.id))
        .filter(|m| {
            open || pool.includes.contains(&m.id) || pool.rules.iter().any(|r| rule_matches(r, m))
        })
        .map(|m| m.id.clone())
        .collect()
}

pub fn resolve_with_fallback(
    pool: &ModelPool,
    catalog: &[PoolModelView],
    enabled: &BTreeSet<String>,
) -> (BTreeSet<String>, bool) {
    let ids = resolve(pool, catalog, enabled);
    if ids.is_empty() {
        (resolve(&ModelPool::default(), catalog, enabled), true)
    } else {
        (ids, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    fn mv(id: &str, p: &str, c: f64, f: bool, t: &str, ctx: u64) -> PoolModelView {
        PoolModelView {
            id: id.into(),
            provider: p.into(),
            cost_per_1k: c,
            max_tokens: ctx,
            is_free: f,
            tier: t.into(),
        }
    }
    fn cat() -> Vec<PoolModelView> {
        vec![
            mv("or/free", "openrouter", 0.0, true, "Free", 8000),
            mv("an/opus", "anthropic", 0.015, false, "Elite", 200000),
            mv("oa/mini", "openai", 0.002, false, "Fast", 128000),
            mv("x/dep", "openai", 0.001, false, "Fast", 4000),
        ]
    }
    fn enabled() -> BTreeSet<String> {
        ["openrouter", "anthropic", "openai"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
    #[test]
    fn empty_pool_all_enabled() {
        assert_eq!(resolve(&ModelPool::default(), &cat(), &enabled()).len(), 4);
    }
    #[test]
    fn rules_union_includes_minus_excludes() {
        let pool = ModelPool {
            rules: vec![PoolRule::Free, PoolRule::MaxCostPer1k { value: 0.005 }],
            includes: vec!["an/opus".into()],
            excludes: vec!["x/dep".into()],
            disabled_sources: vec![],
        };
        let g = resolve(&pool, &cat(), &enabled());
        assert!(
            g.contains("or/free")
                && g.contains("oa/mini")
                && g.contains("an/opus")
                && !g.contains("x/dep")
        );
    }
    #[test]
    fn disabled_source_and_unenabled_drop() {
        let pool = ModelPool {
            disabled_sources: vec!["openrouter".into()],
            ..Default::default()
        };
        let en: BTreeSet<String> = ["openrouter", "anthropic"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let g = resolve(&pool, &cat(), &en);
        assert!(!g.contains("or/free") && !g.contains("oa/mini"));
    }
    #[test]
    fn empty_result_fails_open() {
        let pool = ModelPool {
            excludes: cat().iter().map(|m| m.id.clone()).collect(),
            ..Default::default()
        };
        let (g, fo) = resolve_with_fallback(&pool, &cat(), &enabled());
        assert!(fo && g.len() == 4);
    }
    #[test]
    fn parses_toml() {
        let p: ModelPool = toml::from_str(
            r#"rules=[{kind="free"},{kind="provider",value="anthropic"}]
includes=["a/b"]"#,
        )
        .unwrap();
        assert_eq!(p.rules.len(), 2);
        assert_eq!(p.includes, vec!["a/b"]);
    }
}
