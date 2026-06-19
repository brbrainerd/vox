use serde::{Deserialize, Serialize};
use vox_config::model_pool::{
    ModelPool, PoolModelView, list_enabled_providers, resolve_with_fallback,
};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ModelPoolDto {
    pub rules: Vec<vox_config::model_pool::PoolRule>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub disabled_sources: Vec<String>,
    #[serde(default)]
    pub member_ids: Vec<String>,
    #[serde(default)]
    pub fell_open: bool,
}

fn catalog_views() -> Vec<PoolModelView> {
    // reuse the same registry the model cards come from (see commands/models.rs)
    crate::commands::models::registry_from_cache()
        .list_models()
        .iter()
        .map(|m| PoolModelView {
            id: m.id.clone(),
            provider: m.provider.clone(),
            cost_per_1k: m.cost_per_1k,
            max_tokens: m.max_tokens as u64,
            is_free: m.is_free,
            tier: format!("{:?}", m.capabilities.tier),
        })
        .collect()
}

#[tauri::command]
pub async fn get_model_pool() -> Result<ModelPoolDto, String> {
    let pool = vox_config::VoxConfig::load().model_pool;
    let (ids, fell_open) =
        resolve_with_fallback(&pool, &catalog_views(), &list_enabled_providers());
    Ok(ModelPoolDto {
        rules: pool.rules,
        includes: pool.includes,
        excludes: pool.excludes,
        disabled_sources: pool.disabled_sources,
        member_ids: ids.into_iter().collect(),
        fell_open,
    })
}

#[tauri::command]
pub async fn set_model_pool(pool: ModelPoolDto) -> Result<(), String> {
    let mut cfg = vox_config::VoxConfig::load();
    cfg.model_pool = ModelPool {
        rules: pool.rules,
        includes: pool.includes,
        excludes: pool.excludes,
        disabled_sources: pool.disabled_sources,
    };
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_enabled_providers_cmd() -> Result<Vec<String>, String> {
    Ok(list_enabled_providers().into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::model_pool::PoolRule;

    #[test]
    fn test_dto_serde_roundtrip() {
        let dto = ModelPoolDto {
            rules: vec![PoolRule::Free, PoolRule::MaxCostPer1k { value: 0.005 }],
            includes: vec!["my/model".into()],
            excludes: vec!["other/model".into()],
            disabled_sources: vec!["badprovider".into()],
            member_ids: vec!["my/model".into()],
            fell_open: false,
        };
        let serialized = serde_json::to_string(&dto).unwrap();
        let deserialized: ModelPoolDto = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.includes, dto.includes);
        assert_eq!(deserialized.excludes, dto.excludes);
        assert_eq!(deserialized.disabled_sources, dto.disabled_sources);
        assert_eq!(deserialized.member_ids, dto.member_ids);
        assert_eq!(deserialized.fell_open, dto.fell_open);
        assert_eq!(deserialized.rules.len(), 2);
    }
}
