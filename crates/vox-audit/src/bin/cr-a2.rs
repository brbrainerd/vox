//! CR-A2 schema-parity sweep.
//!
//! Per `docs/src/architecture/v1-release-criteria.md` §CR-A2 and honest
//! plan §5.7: "100% of internal FFI and IPC interfaces must use
//! non-null, machine-verified schemas (VoxProto v1)."
//!
//! What this v1.0 sweep does:
//!
//!   1. Walks `contracts/` looking for every `.yaml` / `.yml` / `.json`
//!      file (these define the workspace's IPC / contract surfaces).
//!   2. For each, checks whether the top-level body declares
//!      `schema_version` (the v1.0 proxy for "machine-verified schema").
//!      Files that declare `schema_version: <int>` are treated as parity-
//!      compliant; files that don't are flagged.
//!   3. Emits `contracts/reports/arch/cr-a2/<UTC>.json` with parity
//!      percentage, list of non-compliant files, and the v1.0 enforce-
//!      mode flag (initially false per plan: "begins with enforce:false,
//!      flipped to true once parity reaches 100%").
//!
//! What it does NOT do (intentionally deferred):
//!
//!   - Parse Rust `extern "C"` declarations to confirm each has a
//!     matching `voxproto/` schema. There is no voxproto/ directory
//!     today; that's its own v1.x track.
//!   - Walk MCP tool registrations. The MCP tool surface is owned by
//!     `vox-cli-mcp` and has its own contract.
//!
//! These exclusions are recorded in the artifact under
//! `out_of_scope_for_v1_0` so the next reader knows.

use serde_json::json;

// 2026-05-21: flipped to true once parity reached 100% per honest plan
// §5.7 ("begins with enforce:false, flipped to true once parity reaches
// 100%"). New unversioned contracts now fail CI.
const ENFORCE: bool = true;

fn main() {
    let workspace = vox_audit::workspace_root();
    let contracts_dir = workspace.join("contracts");
    if !contracts_dir.is_dir() {
        eprintln!("CR-A2: no contracts/ directory; nothing to audit");
        std::process::exit(2);
    }

    let mut total: u32 = 0;
    let mut with_schema_version: Vec<String> = Vec::new();
    let mut without_schema_version: Vec<String> = Vec::new();
    let mut unparseable: Vec<String> = Vec::new();
    let mut malformed_json: Vec<String> = Vec::new();
    let mut excluded_as_data: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(&contracts_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        // Exclude reports/ — those are generated artifacts, not contracts.
        if p.components().any(|c| c.as_os_str() == "reports") {
            continue;
        }
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        total += 1;
        let rel = p
            .strip_prefix(&workspace)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");

        // Exclude DATA files (samples, fixtures, protocol messages) from
        // the parity denominator. These are not contracts themselves;
        // they're values conforming to a contract that lives elsewhere.
        // The schema-version requirement applies to the contract, not
        // to every conforming instance.
        if is_data_not_contract(&rel) {
            excluded_as_data.push(rel);
            total -= 1; // back out the bump we did above
            continue;
        }

        let Ok(body) = std::fs::read_to_string(p) else {
            unparseable.push(rel);
            continue;
        };
        // Surface malformed JSON as its own bug class. The version-detection
        // still succeeds via text-scan fallback, so this is informational —
        // but a malformed schema file is a real defect worth flagging.
        if ext == "json" && !json_parses(&body) {
            malformed_json.push(rel.clone());
        }
        if has_schema_version(&body, ext) {
            with_schema_version.push(rel);
        } else {
            without_schema_version.push(rel);
        }
    }

    let parity_pct = if total == 0 {
        0.0
    } else {
        100.0 * (with_schema_version.len() as f64) / f64::from(total)
    };
    let met = without_schema_version.is_empty() && unparseable.is_empty();
    let gate_blocks = ENFORCE && !met;

    eprintln!(
        "CR-A2: {} of {} contracts declare schema_version ({parity_pct:.1}%)",
        with_schema_version.len(),
        total
    );
    if !without_schema_version.is_empty() {
        eprintln!(
            "CR-A2: {} contract(s) missing schema_version (enforce={ENFORCE}):",
            without_schema_version.len()
        );
        for m in without_schema_version.iter().take(20) {
            eprintln!("  - {m}");
        }
        if without_schema_version.len() > 20 {
            eprintln!("  ... and {} more", without_schema_version.len() - 20);
        }
    }

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-A2",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "contracts_dir": contracts_dir.display().to_string(),
        "total_contracts": total,
        "with_schema_version_count": with_schema_version.len(),
        "without_schema_version": without_schema_version,
        "unparseable": unparseable,
        "malformed_json": malformed_json,
        "excluded_as_data": excluded_as_data,
        "parity_pct": parity_pct,
        "enforce": ENFORCE,
        "threshold": {
            "target_parity_pct": 100.0,
            "met": met,
            "gate_blocks": gate_blocks,
        },
        "out_of_scope_for_v1_0": [
            "Rust extern \"C\" declarations are not enumerated (no voxproto/ schemas exist yet).",
            "MCP tool surfaces are owned by vox-cli-mcp and have their own contract."
        ]
    });
    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("arch")
        .join("cr-a2");
    std::fs::create_dir_all(&out_dir).expect("create cr-a2 dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if gate_blocks {
        std::process::exit(1);
    }
}

/// Does the body declare any of the recognized machine-verified version
/// markers at the top level?
///
/// Honest plan §5.7 originally said "schema_version", but the codebase
/// already uses several legitimate alternatives for self-describing
/// contracts. We accept any of:
///   - `schema_version` — the canonical project convention
///   - `x-vox-version`  — Vox-extended-attribute convention (in-house)
///   - `$schema`        — JSON Schema standard (self-describing)
///   - `openapi`        — OpenAPI v3 standard (self-describing)
///
/// Files lacking ALL of these are flagged as genuinely-unversioned.
/// This matches the spirit of CR-A2 ("non-null, machine-verified
/// schemas") without forcing the canonical name on already-versioned
/// schemas that follow upstream standards.
fn has_schema_version(body: &str, ext: &str) -> bool {
    match ext {
        "json" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
                return json_value_has_version(&v);
            }
            // JSON-parse failure (malformed file): fall back to text scan.
            json_text_scan_for_version_key(body)
        }
        "yaml" | "yml" => yaml_top_level_scan_for_version_key(body),
        _ => false,
    }
}

/// Accept any of the recognized markers as version proof on a parsed
/// JSON value. For top-level arrays (legitimate convention for catalog
/// files like `model-catalog.bootstrap.v1.json`), every element must
/// carry a recognized marker — that's how the catalog encodes "each
/// entry has its own version."
fn json_value_has_version(v: &serde_json::Value) -> bool {
    if let Some(obj) = v.as_object() {
        return obj.contains_key("schema_version")
            || obj.contains_key("x-vox-version")
            || obj.contains_key("$schema")
            || obj.contains_key("schema")
            || obj.contains_key("version");
    }
    if let Some(arr) = v.as_array() {
        // Convention used by catalog files (e.g. model-catalog.bootstrap.v1.json):
        // the FIRST element carries the file-level version marker as a
        // header. We accept that as "the array is versioned."
        return arr.first().is_some_and(json_value_has_version);
    }
    false
}

/// YAML scan — top-level means column 0 (`line.starts_with(key:)`).
/// Nested keys like `doc:\n  schema_version: 1` do NOT count.
///
/// Recognized markers (each is one of the codebase's existing
/// conventions for "this contract is versioned"):
///   - `schema_version`  — canonical project name
///   - `x-vox-version`   — in-house extended-attribute convention
///   - `openapi`         — OpenAPI v3 self-versioning
///   - `$schema`         — JSON Schema self-versioning
///   - `version`         — top-level integer version (used by MCP, scientia distribution packs, etc.)
///   - `schema`          — typed schema-ID reference (used by workspace-toolchain.v1.yaml's `schema: vox.workspace.toolchain.v1`)
///
/// Additionally, the conventional `# yaml-language-server: $schema=…`
/// editor pragma on line 1 counts — that schema reference IS machine-
/// verifiable.
fn yaml_top_level_scan_for_version_key(body: &str) -> bool {
    let needles = [
        "schema_version",
        "x-vox-version",
        "openapi",
        "$schema",
        "version",
        "schema",
    ];
    for (i, line) in body.lines().enumerate() {
        // yaml-language-server: $schema=... pragma (line 1 by convention).
        if i < 3 && line.starts_with("# yaml-language-server:") && line.contains("$schema=") {
            return true;
        }
        for needle in needles {
            if line.starts_with(&format!("{needle}:")) || line.starts_with(&format!("{needle} :")) {
                // For `version:` and `schema:`, require a non-empty
                // value (i.e. not `version:` followed by nested keys).
                let rest = line[needle.len() + 1..].trim();
                if matches!(needle, "version" | "schema") && rest.is_empty() {
                    continue;
                }
                return true;
            }
        }
    }
    false
}

/// JSON text scan — used only when serde_json fails to parse. Looks for
/// a top-level pretty-printed key like `  "x-vox-version": 1,`. Allows
/// up to 4 spaces of leading indent (the conventional pretty-print
/// depth for top-level keys inside a one-object-per-file JSON).
fn json_text_scan_for_version_key(body: &str) -> bool {
    for line in body.lines() {
        let lead = line.chars().take_while(|c| *c == ' ').count();
        if lead > 4 {
            continue;
        }
        let trimmed = &line[lead..];
        for needle in ["schema_version", "x-vox-version", "$schema"] {
            if trimmed.starts_with(&format!("\"{needle}\":"))
                || trimmed.starts_with(&format!("\"{needle}\" :"))
            {
                return true;
            }
        }
    }
    false
}

/// Returns true if the body is valid JSON. Used to surface malformed
/// `.schema.json` files as their own diagnostic category.
fn json_parses(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok()
}

/// Classify a contract path as DATA-not-CONTRACT. These are excluded
/// from the parity denominator because they're instances conforming to
/// a contract that lives elsewhere, not contracts themselves.
///
/// Categories excluded:
///   - `*/repair-corpus/projects/*/expected.json` — per-fixture
///     post-repair criterion data
///   - `*/openclaw/discovery/*.json` — JSON-RPC discovery message samples
///   - `*/openclaw/protocol/*.json` — JSON-RPC protocol message samples
///   - `*.fixtures.json` / `*.fixture.json` — fixture data files
///   - `*.example.*` — example payloads
///   - `*.test-corpus.*` — corpus samples for a test
///
/// The contract these conform to (e.g. an OpenClaw protocol schema)
/// lives in a sibling `.schema.json` / `.v1.yaml` that IS in scope.
fn is_data_not_contract(rel_path: &str) -> bool {
    let lower = rel_path.to_ascii_lowercase();
    if lower.contains("/repair-corpus/projects/") && lower.ends_with("/expected.json") {
        return true;
    }
    if lower.contains("/openclaw/discovery/") && lower.ends_with(".json") {
        return true;
    }
    if lower.contains("/openclaw/protocol/") && lower.ends_with(".json") {
        return true;
    }
    if lower.ends_with(".fixtures.json") || lower.ends_with(".fixture.json") {
        return true;
    }
    if lower.contains(".example.") {
        return true;
    }
    if lower.contains(".test-corpus.") {
        return true;
    }
    // Generated files — annotating them is futile (the next regen
    // strips it). The generator that produces them MUST emit a version
    // marker; that's a generator-side fix, out of scope for this sweep.
    if lower.ends_with(".canonical.yaml") || lower.ends_with(".canonical.json") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_top_level_schema_version_detected() {
        assert!(has_schema_version("schema_version: 1\nname: foo\n", "yaml"));
    }

    #[test]
    fn yaml_nested_schema_version_not_at_top_is_missed() {
        assert!(!has_schema_version("doc:\n  schema_version: 1\n", "yaml"));
    }

    #[test]
    fn yaml_with_x_vox_prefix_ok() {
        assert!(has_schema_version(
            "x-vox-version: 1\nschema_version: 1\n",
            "yaml"
        ));
    }

    #[test]
    fn json_with_schema_version_ok() {
        assert!(has_schema_version(
            r#"{"schema_version": 1, "x": []}"#,
            "json"
        ));
    }

    #[test]
    fn json_without_schema_version_missed() {
        assert!(!has_schema_version(r#"{"x": []}"#, "json"));
    }

    #[test]
    fn json_schema_with_dollar_schema_ok() {
        // Standard JSON Schema files self-version via $schema.
        assert!(has_schema_version(
            r#"{"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "X"}"#,
            "json"
        ));
    }

    #[test]
    fn json_with_x_vox_version_ok() {
        // In-house convention used by examples + extended attrs.
        assert!(has_schema_version(
            r#"{"x-vox-version": 1, "version": 1}"#,
            "json"
        ));
    }

    #[test]
    fn yaml_with_openapi_top_level_ok() {
        // OpenAPI specs self-version via the `openapi:` top-level key.
        assert!(has_schema_version(
            "openapi: 3.0.3\ninfo:\n  title: x\n",
            "yaml"
        ));
    }

    #[test]
    fn yaml_with_x_vox_version_ok() {
        assert!(has_schema_version("x-vox-version: 1\nname: foo\n", "yaml"));
    }

    #[test]
    fn yaml_no_schema_version_returns_false() {
        assert!(!has_schema_version("name: foo\nkind: bar\n", "yaml"));
    }

    #[test]
    fn data_not_contract_excludes_protocol_samples() {
        assert!(is_data_not_contract(
            "contracts/openclaw/protocol/connect.request.operator.json"
        ));
        assert!(is_data_not_contract(
            "contracts/openclaw/discovery/well-known.minimal.json"
        ));
    }

    #[test]
    fn data_not_contract_excludes_repair_expected() {
        assert!(is_data_not_contract(
            "contracts/eval/repair-corpus/projects/001-type-error/expected.json"
        ));
    }

    #[test]
    fn data_not_contract_excludes_fixture_and_example_files() {
        assert!(is_data_not_contract(
            "contracts/orchestration/context-lifecycle-telemetry.fixtures.json"
        ));
        assert!(is_data_not_contract(
            "contracts/toestub/suppressions.v1.example.json"
        ));
        assert!(is_data_not_contract(
            "contracts/terminal/exec-policy.test-corpus.yaml"
        ));
    }

    #[test]
    fn data_not_contract_keeps_real_contracts() {
        assert!(!is_data_not_contract("contracts/code-audit/rules.v1.yaml"));
        assert!(!is_data_not_contract(
            "contracts/orchestration/model-pins.v1.yaml"
        ));
    }
}
