//! Guards for data / telemetry SSOT drift (`vox ci data-ssot-guards`).

use anyhow::{Context, Result, anyhow};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

fn metric_type_constants_from_research_contract(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim_start();
        if !line.starts_with("pub const METRIC_TYPE_") {
            continue;
        }
        let Some(rest) = line.strip_prefix("pub const METRIC_TYPE_") else {
            continue;
        };
        // `BENCHMARK_EVENT: &str = "benchmark_event";`
        let Some(eq) = rest.find("= \"") else {
            continue;
        };
        let after = &rest[eq + 3..];
        let Some(end) = after.find('"') else {
            continue;
        };
        out.push(after[..end].to_string());
    }
    out
}

fn run_scientia_consumption_registry_guard(root: &Path, contracts_index_raw: &str) -> Result<()> {
    let registry = root.join("contracts/scientia/consumption-registry.v1.yaml");
    let raw =
        read_utf8_path_capped(&registry).with_context(|| format!("read {}", registry.display()))?;
    let y: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", registry.display()))?;
    let entries = y
        .get("entries")
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| anyhow!("{} must define `entries: []`", registry.display()))?;
    if entries.is_empty() {
        return Err(anyhow!(
            "{} must include at least one consumption entry",
            registry.display()
        ));
    }
    for entry in entries {
        let schema_id = entry
            .get("schema_id")
            .and_then(serde_yaml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} contains an entry with missing/empty schema_id",
                    registry.display()
                )
            })?;
        let contract_path = entry
            .get("contract_path")
            .and_then(serde_yaml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} entry {schema_id} missing/empty contract_path",
                    registry.display()
                )
            })?;
        let consumption_required = entry
            .get("consumption_required")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);

        if !contracts_index_raw.contains(schema_id) {
            return Err(anyhow!(
                "{} schema_id `{schema_id}` must be registered in contracts/index.yaml",
                registry.display()
            ));
        }
        if !root.join(contract_path).is_file() {
            return Err(anyhow!(
                "{} schema_id `{schema_id}` references missing contract path {}",
                registry.display(),
                contract_path
            ));
        }
        if !consumption_required {
            continue;
        }

        let consumer_paths = entry
            .get("consumer_paths")
            .and_then(serde_yaml::Value::as_sequence)
            .ok_or_else(|| {
                anyhow!(
                    "{} entry {schema_id} must define consumer_paths when consumption_required=true",
                    registry.display()
                )
            })?;
        if consumer_paths.is_empty() {
            return Err(anyhow!(
                "{} entry {schema_id} requires non-empty consumer_paths",
                registry.display()
            ));
        }
        for p in consumer_paths {
            let rel = p
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "{} entry {schema_id} has invalid consumer path",
                        registry.display()
                    )
                })?;
            if !root.join(rel).is_file() {
                return Err(anyhow!(
                    "{} entry {schema_id} references missing consumer path {}",
                    registry.display(),
                    rel
                ));
            }
        }

        let test_targets = entry
            .get("test_targets")
            .and_then(serde_yaml::Value::as_sequence)
            .ok_or_else(|| {
                anyhow!(
                    "{} entry {schema_id} must define test_targets when consumption_required=true",
                    registry.display()
                )
            })?;
        if test_targets.is_empty() {
            return Err(anyhow!(
                "{} entry {schema_id} requires at least one declared test target",
                registry.display()
            ));
        }
        for t in test_targets {
            let cmd = t
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "{} entry {schema_id} has invalid test target",
                        registry.display()
                    )
                })?;
            if !cmd.starts_with("cargo test -p ") {
                return Err(anyhow!(
                    "{} entry {schema_id} test target must start with `cargo test -p ` (got `{}`)",
                    registry.display(),
                    cmd
                ));
            }
        }
    }
    Ok(())
}

fn run_routing_ssot_guard(root: &Path) -> Result<()> {
    let yaml_path = root.join("contracts/orchestration/model-routing.v1.yaml");
    let schema_path = root.join("contracts/orchestration/model-routing.v1.schema.json");
    if !yaml_path.is_file() {
        return Err(anyhow!("missing {}", yaml_path.display()));
    }
    if !schema_path.is_file() {
        return Err(anyhow!("missing {}", schema_path.display()));
    }

    let yaml_txt = read_utf8_path_capped(&yaml_path)
        .with_context(|| format!("read {}", yaml_path.display()))?;
    let _yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_txt)
        .with_context(|| format!("parse {}", yaml_path.display()))?;
    if !yaml_txt.contains("capability_policy:") {
        return Err(anyhow!(
            "{} must define `capability_policy`",
            yaml_path.display()
        ));
    }
    if !yaml_txt.contains("lifecycle:") {
        return Err(anyhow!("{} must define `lifecycle`", yaml_path.display()));
    }

    let schema_txt = read_utf8_path_capped(&schema_path)
        .with_context(|| format!("read {}", schema_path.display()))?;
    let _schema: serde_json::Value = serde_json::from_str(&schema_txt)
        .with_context(|| format!("parse {}", schema_path.display()))?;

    Ok(())
}

fn run_telemetry_events_catalog_guard(root: &Path) -> Result<()> {
    let catalog_path = root.join("contracts/telemetry/events.v1.yaml");
    let raw = read_utf8_path_capped(&catalog_path)
        .with_context(|| format!("read {}", catalog_path.display()))?;
    let y: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", catalog_path.display()))?;
    let events = y
        .get("events")
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| anyhow!("{} must define `events: []`", catalog_path.display()))?;
    if events.is_empty() {
        return Err(anyhow!(
            "{} must include at least one telemetry event entry",
            catalog_path.display()
        ));
    }
    for ev in events {
        let id = ev
            .get("id")
            .and_then(serde_yaml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("{} contains event with missing id", catalog_path.display()))?;
        let schema_path = ev
            .get("schema_path")
            .and_then(serde_yaml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} event `{id}` missing schema_path",
                    catalog_path.display()
                )
            })?;
        let abs = root.join(schema_path);
        if !abs.is_file() {
            return Err(anyhow!(
                "{} event `{id}` references missing schema {}",
                catalog_path.display(),
                schema_path
            ));
        }
    }
    Ok(())
}

fn run_provider_secret_parity_guard(root: &Path) -> Result<()> {
    let providers_path = root.join("contracts/orchestration/providers.v1.yaml");
    let providers_txt = read_utf8_path_capped(&providers_path)
        .with_context(|| format!("read {}", providers_path.display()))?;
    let providers_yaml: serde_yaml::Value = serde_yaml::from_str(&providers_txt)
        .with_context(|| format!("parse {}", providers_path.display()))?;
    let providers = providers_yaml
        .get("providers")
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| anyhow!("{} must define providers[]", providers_path.display()))?;

    let known_ids: std::collections::BTreeSet<String> = vox_secrets::all_specs()
        .iter()
        .map(|s| format!("{:?}", s.id))
        .collect();
    for provider in providers {
        let provider_name = provider
            .get("name")
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or("<unknown>");
        if provider.get("lifecycle").is_none() {
            return Err(anyhow!(
                "{} provider `{}` must define lifecycle metadata",
                providers_path.display(),
                provider_name
            ));
        }
        if let Some(secret_id) = provider
            .get("secret_id")
            .and_then(serde_yaml::Value::as_str)
            && !known_ids.contains(secret_id)
        {
            return Err(anyhow!(
                "{} provider `{}` uses unknown secret_id `{}` (not present in vox_secrets::all_specs)",
                providers_path.display(),
                provider_name,
                secret_id
            ));
        }
    }
    Ok(())
}

/// Run file-based checks that do not require a full workspace `cargo test`.
pub fn run_data_ssot_guards(root: &Path) -> Result<()> {
    let watch = root.join("crates/vox-ml-cli/src/commands/mens/watch_telemetry.rs");
    let watch_txt =
        read_utf8_path_capped(&watch).with_context(|| format!("read {}", watch.display()))?;
    if !watch_txt.contains("eta_seconds_remaining") {
        return Err(anyhow!(
            "{} must parse telemetry payload key `eta_seconds_remaining` (Populi telemetry_schema)",
            watch.display()
        ));
    }
    if !watch_txt.contains("steps_per_sec_ema") {
        return Err(anyhow!(
            "{} must parse `steps_per_sec_ema`",
            watch.display()
        ));
    }

    let policy_doc = root.join("docs/src/architecture/voxdb-connect-policy.md");
    let fallback_policy_doc = root.join("docs/src/architecture/data-storage-ssot-2026.md");
    if !policy_doc.is_file() && !fallback_policy_doc.is_file() {
        return Err(anyhow!(
            "missing {} and {}",
            policy_doc.display(),
            fallback_policy_doc.display()
        ));
    }

    let metric_doc = root.join("docs/src/reference/telemetry-metric-contract.md");
    if !metric_doc.is_file() {
        return Err(anyhow!("missing {}", metric_doc.display()));
    }
    let metric_txt = read_utf8_path_capped(&metric_doc)
        .with_context(|| format!("read {}", metric_doc.display()))?;

    let taxonomy_doc = root.join("docs/src/architecture/telemetry-taxonomy-contracts-ssot.md");
    let fallback_taxonomy_doc = root.join("docs/src/architecture/telemetry-trust-ssot.md");
    if !taxonomy_doc.is_file() && !fallback_taxonomy_doc.is_file() {
        return Err(anyhow!(
            "missing {} and {}",
            taxonomy_doc.display(),
            fallback_taxonomy_doc.display()
        ));
    }
    let taxonomy_txt = if taxonomy_doc.is_file() {
        read_utf8_path_capped(&taxonomy_doc)
            .with_context(|| format!("read {}", taxonomy_doc.display()))?
    } else {
        read_utf8_path_capped(&fallback_taxonomy_doc)
            .with_context(|| format!("read {}", fallback_taxonomy_doc.display()))?
    };

    let research_contract = root.join("crates/vox-telemetry/src/types.rs");
    let rc = read_utf8_path_capped(&research_contract)
        .with_context(|| format!("read {}", research_contract.display()))?;
    let metric_types = metric_type_constants_from_research_contract(&rc);
    if metric_types.is_empty() {
        return Err(anyhow!(
            "{}: expected pub const METRIC_TYPE_* string literals",
            research_contract.display()
        ));
    }
    for mt in &metric_types {
        if !metric_txt.contains(mt.as_str()) && !taxonomy_txt.contains(mt.as_str()) {
            return Err(anyhow!(
                "METRIC_TYPE value `{mt}` must appear in {} or {}",
                metric_doc.display(),
                taxonomy_doc.display()
            ));
        }
    }

    let contracts_index = root.join("contracts/index.yaml");
    let ix = read_utf8_path_capped(&contracts_index)
        .with_context(|| format!("read {}", contracts_index.display()))?;
    for id in [
        "telemetry-completion-run-v1-schema",
        "telemetry-completion-finding-v1-schema",
        "telemetry-completion-detector-snapshot-v1-schema",
        "orchestration-model-routing-v1",
        "orchestration-model-routing-v1-schema",
        "orchestration-providers-v1",
    ] {
        if !ix.contains(id) {
            return Err(anyhow!(
                "{} must register `{id}` (completion telemetry JSON Schemas)",
                contracts_index.display()
            ));
        }
    }
    run_scientia_consumption_registry_guard(root, &ix)?;
    run_telemetry_events_catalog_guard(root)?;
    run_provider_secret_parity_guard(root)?;

    let env_vars = root.join("contracts/config/env-vars.v1.yaml");
    let env_txt =
        read_utf8_path_capped(&env_vars).with_context(|| format!("read {}", env_vars.display()))?;
    for name in [
        "VOX_ROUTE_POLICY_PROFILE",
        "VOX_ROUTE_ALLOW_NET",
        "VOX_ROUTE_ALLOW_PROVIDER_NETWORK",
        "VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP",
    ] {
        if !env_txt.contains(name) {
            return Err(anyhow!(
                "{} must include `{}` for routing capability policy overrides",
                env_vars.display(),
                name
            ));
        }
    }

    let codex_metrics = root.join("crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs");
    let cm = read_utf8_path_capped(&codex_metrics)
        .with_context(|| format!("read {}", codex_metrics.display()))?;
    if cm.contains("COALESCE(metric_value") {
        return Err(anyhow!(
            "{} must not use COALESCE(metric_value — NULL must stay distinct from 0.0",
            codex_metrics.display()
        ));
    }

    let retention_policy = root.join("contracts/db/retention-policy.yaml");
    let retention_txt = read_utf8_path_capped(&retention_policy)
        .with_context(|| format!("read {}", retention_policy.display()))?;
    if !retention_txt.contains("agent_exec_history") {
        return Err(anyhow!(
            "{} must define retention policy for `agent_exec_history`",
            retention_policy.display()
        ));
    }

    run_routing_ssot_guard(root)?;

    println!("data-ssot-guards: OK");
    Ok(())
}
