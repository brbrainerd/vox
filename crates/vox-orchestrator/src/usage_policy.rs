use std::collections::BTreeMap;
use vox_secrets::{SecretId, resolve_secret};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderLimitOwned {
    pub provider: String,
    pub model: String,
    pub daily_limit: u32,
}

fn parse_limit_json(raw: &str) -> Vec<ProviderLimitOwned> {
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(obj) = parsed.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (k, v) in obj {
        let Some(limit) = v.as_u64() else {
            continue;
        };
        let mut parts = k.splitn(2, '/');
        let provider = parts.next().unwrap_or_default().trim();
        let model = parts.next().unwrap_or_default().trim();
        if provider.is_empty() || model.is_empty() {
            continue;
        }
        out.push(ProviderLimitOwned {
            provider: provider.to_string(),
            model: model.to_string(),
            daily_limit: limit.min(u32::MAX as u64) as u32,
        });
    }
    out
}

fn default_limits() -> Vec<ProviderLimitOwned> {
    let default_cloud_limit = resolve_secret(SecretId::VoxProviderDailyLimitDefault)
        .expose()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(100);
    let providers = resolve_secret(SecretId::VoxProviderLimitProviders)
        .expose()
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                "google".to_string(),
                "openrouter".to_string(),
                "ollama".to_string(),
                "groq".to_string(),
                "cerebras".to_string(),
                "mistral".to_string(),
                "deepseek".to_string(),
                "sambanova".to_string(),
                "custom".to_string(),
            ]
        });
    let mut out = Vec::new();
    for provider in providers {
        let (model, daily_limit) = match provider.as_str() {
            "openrouter" => (":free".to_string(), default_cloud_limit / 2),
            "ollama" => ("*".to_string(), u32::MAX),
            _ => ("*".to_string(), default_cloud_limit),
        };
        out.push(ProviderLimitOwned {
            provider,
            model,
            daily_limit,
        });
    }
    out
}

pub fn resolve_provider_limits() -> Vec<ProviderLimitOwned> {
    let mut merged: BTreeMap<(String, String), u32> = BTreeMap::new();
    for d in default_limits() {
        merged.insert((d.provider, d.model), d.daily_limit);
    }

    if let Some(path) = resolve_secret(SecretId::VoxProviderDailyLimitsFile).expose() {
        let p = std::path::PathBuf::from(path);
        if let Ok(raw) = vox_bounded_fs::read_utf8_path_capped(&p) {
            for d in parse_limit_json(&raw) {
                merged.insert((d.provider, d.model), d.daily_limit);
            }
        }
    }
    if let Some(raw) = resolve_secret(SecretId::VoxProviderDailyLimitsJson).expose() {
        for d in parse_limit_json(raw) {
            merged.insert((d.provider, d.model), d.daily_limit);
        }
    }

    merged
        .into_iter()
        .map(|((provider, model), daily_limit)| ProviderLimitOwned {
            provider,
            model,
            daily_limit,
        })
        .collect()
}

#[cfg(test)]
mod semcov_wave1_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn parse_limit_json_invalid_and_malformed_keys() {
        // Invalid JSON -> empty
        assert_eq!(parse_limit_json("not json"), Vec::new());
        // Non-object JSON -> empty
        assert_eq!(parse_limit_json("[1,2,3]"), Vec::new());
        // Key without '/' has empty model -> skipped; non-u64 value -> skipped
        let out = parse_limit_json(r#"{"google":5,"openrouter/gpt":"x"}"#);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_limit_json_valid_entry_and_u32_clamp() {
        // Valid entry with surrounding whitespace trimmed
        let out = parse_limit_json(r#"{" google / gemini ": 42}"#);
        assert_eq!(
            out,
            vec![ProviderLimitOwned {
                provider: "google".to_string(),
                model: "gemini".to_string(),
                daily_limit: 42,
            }]
        );
        // Value exceeding u32::MAX is clamped to u32::MAX
        let big = format!(r#"{{"p/m": {}}}"#, u64::MAX);
        let clamped = parse_limit_json(&big);
        assert_eq!(clamped.len(), 1);
        assert_eq!(clamped[0].daily_limit, u32::MAX);
    }
}
