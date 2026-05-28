use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct RoCrateMetadata {
    pub name: String,
    pub description: String,
    pub doi: Option<String>,
    pub author_orcid: Option<String>, // e.g. "https://orcid.org/0000-0001-2345-6789"
    pub author_ror: Option<String>,   // e.g. "https://ror.org/03yrm5c26"
    pub license_spdx: String,         // e.g. "CC-BY-4.0"
    pub published_at: i64,            // Unix timestamp
    pub keywords: Vec<String>,
    /// Optional executable specification (Phase B replay-runner contract).
    /// When present, the deposited RO-Crate declares a reproducible
    /// `entry_point`, the expected output paths + hashes, and a resource
    /// budget — enough for `vox-replay-runner` to re-execute the experiment
    /// in a sandbox and write a measured `artifact_replayability` value back
    /// to the worthiness signals. None means the manifest is not
    /// replay-eligible.
    pub main_entity: Option<MainEntity>,
}

/// Executable specification embedded as the RO-Crate `mainEntity` node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MainEntity {
    /// Shell-invocable entry point, relative to the RO-Crate root. The
    /// runner spawns this under `sh -c <entry_point>` (POSIX) or
    /// `cmd /C <entry_point>` (Windows).
    pub entry_point: String,
    /// Output paths (relative to the RO-Crate root) whose SHA3-256 hashes
    /// form the truth-claim. Lengths of `expected_output_paths` and
    /// `expected_output_hashes_hex` MUST match.
    pub expected_output_paths: Vec<String>,
    /// Hex-encoded SHA3-256 of each expected output, in the same order as
    /// `expected_output_paths`.
    pub expected_output_hashes_hex: Vec<String>,
    /// Reference to the locking surface used to pin dependencies
    /// (e.g. `"cargo-lock-sha:<hex>"`).
    pub env_pin: String,
    /// Hard wall-clock cap for the sandboxed run.
    pub timeout_seconds: u32,
    /// Resource budget; runner truncates beyond this size.
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
    /// Phase 5 — figure provenance. Each entry binds a figure file in the
    /// RO-Crate to the script that produced it plus a SHA3-256 hash of the
    /// rendered bytes. The worthiness rubric's "figures must be traceably
    /// generated from stored artifacts" red line is enforced by requiring
    /// every figure referenced in the manuscript to appear here with a
    /// matching hash. Empty when the manifest has no figures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub figures: Vec<FigureProvenance>,
}

/// Provenance binding for one figure in a publication's RO-Crate.
///
/// A figure is "traceable" iff `(path → sha3_256_hex)` matches a real
/// regenerable artifact AND `source_script` is itself in the RO-Crate so
/// reviewers can re-run the rendering. The `caption_hint` is a TODO for the
/// human author — the scaffolder uses it only as a placeholder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureProvenance {
    /// Figure file path relative to the RO-Crate root (e.g.
    /// `figures/fig-01-p95-latency.svg`).
    pub path: String,
    /// SHA3-256 of the rendered figure bytes, hex-encoded. Verified by the
    /// replay runner against the actual file in `stage_dir`.
    pub sha3_256_hex: String,
    /// Path to the script that produced this figure (relative to the
    /// RO-Crate root). Must be re-runnable from the `mainEntity` entry
    /// point's environment.
    pub source_script: String,
    /// Epoch-ms timestamp of when the figure was rendered. Surfaced in
    /// the manuscript so reviewers can correlate against revision history.
    pub rendered_at_ms: i64,
    /// Optional one-line caption hint the human can edit. The scaffolder
    /// renders this as a TODO placeholder; the rubric forbids
    /// auto-generating final captions for measured-outcome figures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_hint: Option<String>,
}

pub fn build_ro_crate_json(metadata: &RoCrateMetadata) -> Value {
    // Format ISO date from Unix timestamp (seconds since epoch, UTC)
    let iso_date = format_iso_date(metadata.published_at);

    let license_url = format!("https://spdx.org/licenses/{}", metadata.license_spdx);

    let author_id = metadata
        .author_orcid
        .clone()
        .unwrap_or_else(|| "#anonymous-author".to_string());

    // Build identifier array (include DOI if present)
    let mut identifiers: Vec<Value> = Vec::new();
    if let Some(doi) = &metadata.doi {
        let doi_url = if doi.starts_with("http") {
            doi.clone()
        } else {
            format!("https://doi.org/{}", doi)
        };
        identifiers.push(json!({ "@id": doi_url }));
    }

    // Build author node
    let mut author_node = json!({
        "@id": author_id,
        "@type": "Person"
    });
    if let Some(ror) = &metadata.author_ror {
        author_node["affiliation"] = json!({ "@id": ror });
    }

    // Descriptor node (ro-crate-metadata.json itself)
    let descriptor = json!({
        "@id": "ro-crate-metadata.json",
        "@type": "CreativeWork",
        "about": { "@id": "./" },
        "conformsTo": { "@id": "https://w3id.org/ro/crate/1.2" }
    });

    // Root dataset node
    let mut dataset = json!({
        "@id": "./",
        "@type": "Dataset",
        "name": metadata.name,
        "description": metadata.description,
        "license": { "@id": license_url },
        "author": [{ "@id": author_id }],
        "datePublished": iso_date,
        "keywords": metadata.keywords
    });

    if !identifiers.is_empty() {
        dataset["identifier"] = json!(identifiers);
    }

    let mut graph = vec![descriptor, dataset];
    // Only include author node if we have a real ORCID or ROR
    if metadata.author_orcid.is_some() || metadata.author_ror.is_some() {
        graph.push(author_node);
    }
    // Phase B: emit mainEntity executable-spec node when present.
    if let Some(me) = &metadata.main_entity {
        graph.push(build_main_entity_node(me));
    }

    json!({
        "@context": [
            "https://w3id.org/ro/crate/1.2/context",
            { "vox": "https://vox-lang.org/ro-crate/v1#" }
        ],
        "@graph": graph
    })
}

/// Construct the JSON-LD node for a [`MainEntity`].
///
/// Uses the `vox:` prefix declared in the top-level `@context` for
/// Vox-specific predicates (entryPoint, expectedOutputs, envPin,
/// timeoutSeconds, resourceBudget) so the node remains valid JSON-LD
/// without polluting the standard schema.org / SoftwareSourceCode terms.
fn build_main_entity_node(me: &MainEntity) -> Value {
    let outputs: Vec<Value> = me
        .expected_output_paths
        .iter()
        .zip(me.expected_output_hashes_hex.iter())
        .map(|(path, hash)| json!({ "path": path, "sha3_256_hex": hash }))
        .collect();
    let figures: Vec<Value> = me
        .figures
        .iter()
        .map(|f| {
            let mut node = json!({
                "@type": "ImageObject",
                "path": f.path,
                "sha3_256_hex": f.sha3_256_hex,
                "vox:sourceScript": f.source_script,
                "vox:renderedAtMs": f.rendered_at_ms,
            });
            if let Some(c) = &f.caption_hint {
                node["vox:captionHint"] = json!(c);
            }
            node
        })
        .collect();
    let mut me_node = json!({
        "@id": "#mainEntity",
        "@type": "SoftwareSourceCode",
        "vox:entryPoint": me.entry_point,
        "vox:expectedOutputs": outputs,
        "vox:envPin": me.env_pin,
        "vox:timeoutSeconds": me.timeout_seconds,
        "vox:resourceBudget": {
            "maxStdoutBytes": me.max_stdout_bytes,
            "maxStderrBytes": me.max_stderr_bytes,
        }
    });
    if !figures.is_empty() {
        me_node["vox:figures"] = json!(figures);
    }
    me_node
}

/// Format a Unix timestamp as an ISO 8601 date string (YYYY-MM-DD).
fn format_iso_date(unix_secs: i64) -> String {
    // Days since Unix epoch
    let days_since_epoch = unix_secs / 86_400;
    let mut year = 1970u32;
    let mut remaining = days_since_epoch as u32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let month_days: [u32; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 1u32;
    for &days in &month_days {
        if remaining < days {
            break;
        }
        remaining -= days;
        month += 1;
    }

    let day = remaining + 1; // 1-based day
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metadata() -> RoCrateMetadata {
        RoCrateMetadata {
            name: "Test Dataset".to_string(),
            description: "A test dataset".to_string(),
            doi: None,
            author_orcid: Some("https://orcid.org/0000-0001-2345-6789".to_string()),
            author_ror: None,
            license_spdx: "CC-BY-4.0".to_string(),
            published_at: 1_700_000_000,
            keywords: vec!["test".to_string()],
            main_entity: None,
        }
    }

    fn sample_main_entity() -> MainEntity {
        MainEntity {
            entry_point: "run.sh".to_string(),
            expected_output_paths: vec!["out.txt".to_string()],
            expected_output_hashes_hex: vec!["deadbeef".to_string()],
            env_pin: "cargo-lock-sha:0000".to_string(),
            timeout_seconds: 60,
            max_stdout_bytes: 1_000_000,
            max_stderr_bytes: 1_000_000,
            figures: vec![],
        }
    }

    fn sample_figure() -> FigureProvenance {
        FigureProvenance {
            path: "figures/fig-01-p95.svg".to_string(),
            sha3_256_hex: "abcd1234".to_string(),
            source_script: "scripts/plot_p95.py".to_string(),
            rendered_at_ms: 1_747_000_000_000,
            caption_hint: Some("p95 latency drop over Q1".to_string()),
        }
    }

    #[test]
    fn default_main_entity_emits_no_figures_node() {
        let mut meta = sample_metadata();
        meta.main_entity = Some(sample_main_entity());
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let me = graph
            .iter()
            .find(|n| n["@id"].as_str() == Some("#mainEntity"))
            .unwrap();
        assert!(
            me.get("vox:figures").is_none(),
            "no figures supplied → no vox:figures key"
        );
    }

    #[test]
    fn main_entity_with_figures_emits_figure_nodes_with_provenance() {
        let mut meta = sample_metadata();
        let mut me = sample_main_entity();
        me.figures = vec![sample_figure()];
        meta.main_entity = Some(me);
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let me_node = graph
            .iter()
            .find(|n| n["@id"].as_str() == Some("#mainEntity"))
            .unwrap();
        let figs = me_node["vox:figures"].as_array().unwrap();
        assert_eq!(figs.len(), 1);
        assert_eq!(figs[0]["@type"], "ImageObject");
        assert_eq!(figs[0]["path"], "figures/fig-01-p95.svg");
        assert_eq!(figs[0]["sha3_256_hex"], "abcd1234");
        assert_eq!(figs[0]["vox:sourceScript"], "scripts/plot_p95.py");
        assert_eq!(figs[0]["vox:renderedAtMs"], 1_747_000_000_000_i64);
        assert_eq!(figs[0]["vox:captionHint"], "p95 latency drop over Q1");
    }

    #[test]
    fn figure_without_caption_hint_omits_field_in_json_ld() {
        let mut meta = sample_metadata();
        let mut me = sample_main_entity();
        me.figures = vec![FigureProvenance {
            caption_hint: None,
            ..sample_figure()
        }];
        meta.main_entity = Some(me);
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let me_node = graph
            .iter()
            .find(|n| n["@id"].as_str() == Some("#mainEntity"))
            .unwrap();
        let figs = me_node["vox:figures"].as_array().unwrap();
        assert!(figs[0].get("vox:captionHint").is_none());
    }

    #[test]
    fn multiple_figures_preserve_order() {
        let mut meta = sample_metadata();
        let mut me = sample_main_entity();
        let mut f2 = sample_figure();
        f2.path = "figures/fig-02-ablation.svg".to_string();
        me.figures = vec![sample_figure(), f2];
        meta.main_entity = Some(me);
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let me_node = graph
            .iter()
            .find(|n| n["@id"].as_str() == Some("#mainEntity"))
            .unwrap();
        let figs = me_node["vox:figures"].as_array().unwrap();
        assert_eq!(figs.len(), 2);
        assert_eq!(figs[0]["path"], "figures/fig-01-p95.svg");
        assert_eq!(figs[1]["path"], "figures/fig-02-ablation.svg");
    }

    #[test]
    fn main_entity_absent_by_default_in_graph() {
        let json = build_ro_crate_json(&sample_metadata());
        let graph = json["@graph"].as_array().unwrap();
        let has_main_entity = graph
            .iter()
            .any(|n| n["@id"].as_str() == Some("#mainEntity"));
        assert!(
            !has_main_entity,
            "default metadata must not emit a mainEntity node"
        );
    }

    #[test]
    fn main_entity_emitted_when_present() {
        let mut meta = sample_metadata();
        meta.main_entity = Some(sample_main_entity());
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let me = graph
            .iter()
            .find(|n| n["@id"].as_str() == Some("#mainEntity"))
            .expect("mainEntity node missing");
        assert_eq!(me["@type"], "SoftwareSourceCode");
        assert_eq!(me["vox:entryPoint"], "run.sh");
        assert_eq!(me["vox:timeoutSeconds"], 60);
        let outputs = me["vox:expectedOutputs"].as_array().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["path"], "out.txt");
        assert_eq!(outputs[0]["sha3_256_hex"], "deadbeef");
    }

    #[test]
    fn vox_prefix_declared_in_context_when_main_entity_present() {
        let mut meta = sample_metadata();
        meta.main_entity = Some(sample_main_entity());
        let json = build_ro_crate_json(&meta);
        let ctx = &json["@context"];
        // @context is an array; one of its entries must map "vox" to the
        // Vox prefix URL so the mainEntity predicates are valid JSON-LD.
        let ctx_arr = ctx.as_array().expect("@context must be an array");
        let has_vox_prefix = ctx_arr.iter().any(|entry| {
            entry.as_object().and_then(|o| o.get("vox")).is_some()
        });
        assert!(has_vox_prefix, "expected @context to declare `vox:` prefix");
    }

    #[test]
    fn ro_crate_has_required_context() {
        // Phase B made `@context` an array so the `vox:` prefix can be
        // declared alongside the base RO-Crate context. The first entry is
        // still the canonical RO-Crate 1.2 context URL.
        let json = build_ro_crate_json(&sample_metadata());
        let ctx = json["@context"].as_array().expect("@context must be an array");
        assert_eq!(
            ctx.first().and_then(Value::as_str),
            Some("https://w3id.org/ro/crate/1.2/context")
        );
    }

    #[test]
    fn ro_crate_graph_contains_dataset() {
        let json = build_ro_crate_json(&sample_metadata());
        let graph = json["@graph"].as_array().unwrap();
        let has_dataset = graph
            .iter()
            .any(|node| node["@type"].as_str() == Some("Dataset"));
        assert!(has_dataset);
    }

    #[test]
    fn doi_included_when_present() {
        let mut meta = sample_metadata();
        meta.doi = Some("10.1234/test".to_string());
        let json = build_ro_crate_json(&meta);
        let graph = json["@graph"].as_array().unwrap();
        let dataset = graph.iter().find(|n| n["@type"] == "Dataset").unwrap();
        assert!(dataset.get("identifier").is_some());
    }

    #[test]
    fn iso_date_epoch_zero_is_1970_01_01() {
        assert_eq!(format_iso_date(0), "1970-01-01");
    }

    #[test]
    fn iso_date_one_day_is_1970_01_02() {
        assert_eq!(format_iso_date(86_400), "1970-01-02");
    }

    #[test]
    fn iso_date_known_date_2023_11_15() {
        // 1700000000 Unix = 2023-11-14 22:13:20 UTC → date = 2023-11-14
        // 1700006400 Unix = 2023-11-15 00:00:00 UTC → date = 2023-11-15
        assert_eq!(format_iso_date(1_700_006_400), "2023-11-15");
    }
}
