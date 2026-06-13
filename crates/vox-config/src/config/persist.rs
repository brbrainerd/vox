//! Merge-write for `~/.vox/config.toml`.

use std::path::Path;

use toml::Value;
use toml::map::Map;

use super::vox_config::VoxConfig;

pub(super) fn take_toml_subtable(root: &mut Map<String, Value>, key: &str) -> Map<String, Value> {
    match root.remove(key) {
        Some(Value::Table(t)) => t,
        _ => Map::new(),
    }
}

/// Merge `cfg` into `path` (global user `config.toml` layout). See [`VoxConfig::save`](super::VoxConfig::save).
pub(super) fn save_merged_global_config(path: &Path, cfg: &VoxConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root: Map<String, Value> = if path.exists() {
        let text = std::fs::read_to_string(path)?;
        match toml::from_str::<Value>(&text) {
            Ok(Value::Table(t)) => t,
            Ok(_) => Map::new(),
            Err(_) => Map::new(),
        }
    } else {
        Map::new()
    };

    let mut vox = take_toml_subtable(&mut root, "vox");
    vox.insert("model".to_string(), Value::String(cfg.model.clone()));
    vox.insert(
        "daily_budget_usd".to_string(),
        Value::Float(cfg.daily_budget_usd),
    );
    vox.insert(
        "per_session_budget_usd".to_string(),
        Value::Float(cfg.per_session_budget_usd),
    );
    vox.insert(
        "gamify_enabled".to_string(),
        Value::Boolean(cfg.gamify_enabled),
    );
    vox.insert(
        "gamify_mode".to_string(),
        Value::String(cfg.gamify_mode.as_config_str().to_string()),
    );
    if let Some(ref p) = cfg.mcp_binary {
        vox.insert(
            "mcp_binary".to_string(),
            Value::String(p.to_string_lossy().into_owned()),
        );
    }
    root.insert("vox".to_string(), Value::Table(vox));

    let mut train = take_toml_subtable(&mut root, "train");
    train.insert(
        "data_dir".to_string(),
        Value::String(cfg.data_dir.to_string_lossy().into_owned()),
    );
    train.insert(
        "model_dir".to_string(),
        Value::String(cfg.model_dir.to_string_lossy().into_owned()),
    );
    train.insert(
        "epochs".to_string(),
        Value::Integer(i64::try_from(cfg.train_epochs).unwrap_or(i64::MAX)),
    );
    train.insert(
        "batch_size".to_string(),
        Value::Integer(i64::try_from(cfg.train_batch_size).unwrap_or(i64::MAX)),
    );
    root.insert("train".to_string(), Value::Table(train));

    if let Some(ref url) = cfg.db_url {
        let mut db_tab = take_toml_subtable(&mut root, "db");
        db_tab.insert("url".to_string(), Value::String(url.clone()));
        root.insert("db".to_string(), Value::Table(db_tab));
    }

    let mut llm = take_toml_subtable(&mut root, "llm");
    llm.insert(
        "max_concurrent_requests".to_string(),
        Value::Integer(i64::try_from(cfg.llm_max_concurrent_requests).unwrap_or(i64::MAX)),
    );
    if let Some(v) = cfg.llm_openrouter_max_concurrent {
        llm.insert(
            "openrouter_max_concurrent".to_string(),
            Value::Integer(i64::try_from(v).unwrap_or(i64::MAX)),
        );
    }
    if let Some(v) = cfg.llm_openai_max_concurrent {
        llm.insert(
            "openai_max_concurrent".to_string(),
            Value::Integer(i64::try_from(v).unwrap_or(i64::MAX)),
        );
    }
    llm.insert(
        "retry_max_attempts".to_string(),
        Value::Integer(i64::from(cfg.llm_retry_max_attempts)),
    );
    root.insert("llm".to_string(), Value::Table(llm));

    let out = toml::to_string_pretty(&Value::Table(root))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(path, out)?;
    Ok(())
}

pub(super) fn global_config_path() -> Option<std::path::PathBuf> {
    crate::paths::data_dir().map(|d| d.join("config.toml"))
}
