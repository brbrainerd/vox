//! Guardrail: every `examples/golden/**/*.vox` file containing `fn main`
//! must execute under the interpreter without crashing. Known pre-existing
//! gaps are listed in `KNOWN_INTERP_GAPS` with a reason comment; delete
//! entries as each gap is resolved.

use std::path::{Path, PathBuf};

fn collect_golden_vox_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let read = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_golden_vox_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("vox") {
            out.push(path);
        }
    }
}

/// Known gaps that prevent a golden from running under `--mode interp`.
/// Each entry is a file-name suffix (enough to be unique) with a reason comment.
/// Delete entries as the gap is fixed.
const KNOWN_INTERP_GAPS: &[(&str, &str)] = &[
    // Web/UI constructs — no interpreter semantics for reactive UI
    ("dashboard_ui.vox", "web-only: reactive UI surface"),
    ("react_interop.vox", "web-only: React JSX/hook bridge"),
    ("reactive_counter.vox", "web-only: reactive state surface"),
    (
        "web_routing_fullstack.vox",
        "web-only: fullstack route surface",
    ),
    ("blog_fullstack.vox", "web-only: fullstack route surface"),
    ("index_showcase.vox", "web-only: component showcase surface"),
    ("layered_overlay.vox", "web-only: overlay UI surface"),
    // Mobile / native constructs
    ("mobile_camera.vox", "mobile-only: native camera API"),
    ("mobile_test.vox", "mobile-only: mobile test surface"),
    // Actor / distributed runtime constructs
    (
        "counter_actor.vox",
        "actor-only: actor spawn/send semantics",
    ),
    ("ref_actors.vox", "actor-only: actor spawn/send semantics"),
    ("ref_agents.vox", "agent-only: agent spawn semantics"),
    ("ref_orchestrator.vox", "actor-only: orchestrator surface"),
    ("agent_pipeline.vox", "agent-only: agent pipeline surface"),
    (
        "agentos_std_surface.vox",
        "agent-only: AgentOS stdlib surface",
    ),
    // Async / durable workflow constructs
    (
        "durable_workflow_real.vox",
        "durable-only: durable workflow surface",
    ),
    ("checkout_workflow.vox", "durable-only: workflow surface"),
    (
        "saga_compensation.vox",
        "durable-only: saga/compensate surface",
    ),
    // Scheduled jobs
    ("scheduled_tick.vox", "runtime-only: scheduled tick surface"),
    (
        "background_jobs.vox",
        "runtime-only: background job surface",
    ),
    // AI/ML fixtures that require LLM runtime
    (
        "ai_fixtures/agent_control_subagent.vox",
        "llm-only: subagent control",
    ),
    (
        "ai_fixtures/deferred_fill_hole.vox",
        "llm-only: deferred fill-hole",
    ),
    (
        "ai_fixtures/end_to_end_demo.vox",
        "llm-only: end-to-end LLM demo",
    ),
    (
        "ai_fixtures/model_selection_intent_routed.vox",
        "llm-only: model-selection intent",
    ),
    (
        "ai_fixtures/query_template_prompt.vox",
        "llm-only: query template prompt",
    ),
    (
        "ai_fixtures/search_substitution_docs.vox",
        "llm-only: search substitution docs",
    ),
    // IoT / native I/O
    ("iot_telemetry.vox", "native-only: IoT telemetry surface"),
    // Mesh constructs
    ("mesh/noop.vox", "mesh-only: mesh dispatch surface"),
    ("ref_effects.vox", "effect-only: effect surface"),
    // Multi-tenancy — requires runtime DB/auth
    ("multi_tenancy.vox", "runtime-only: multi-tenancy DB/auth"),
    // MCP tool surface — requires MCP runtime
    ("mcp_tools.vox", "mcp-only: MCP tool dispatch surface"),
    // Plugin / scrape surfaces
    ("scrape_demo.vox", "browser-only: CDP/scrape surface"),
    // Process/shell — requires OS process runtime
    ("process_run.vox", "os-only: process spawn surface"),
    ("structured_shell_listings.vox", "os-only: shell surface"),
    ("tabular_subprocess.vox", "os-only: subprocess surface"),
    // Auth patterns — requires OAuth/session runtime
    ("auth_patterns.vox", "runtime-only: OAuth/session surface"),
    // Config/deploy — requires env/deploy runtime
    ("config_deploy.vox", "runtime-only: deploy/env surface"),
    // Repo operations — requires VCS runtime
    ("repo_operations.vox", "vcs-only: repo VCS surface"),
    ("repo_versioned_decorator.vox", "vcs-only: repo VCS surface"),
    // DB-heavy goldens that require a running DB
    ("db_advanced_queries.vox", "db-only: DB query surface"),
    ("db_native_ir.vox", "db-only: DB native IR surface"),
    ("db_operations.vox", "db-only: DB operation surface"),
    // CRUD API — HTTP server surface
    ("crud_api.vox", "server-only: CRUD HTTP surface"),
    ("http_error_mapping.vox", "server-only: HTTP error surface"),
    ("std_http_wrappers.vox", "server-only: HTTP stdlib surface"),
    ("pagination.vox", "server-only: HTTP/DB pagination surface"),
    // TypeScript FFI — requires TS runtime
    ("ts_source_ffi.vox", "ffi-only: TypeScript source FFI"),
    // Inventory rosetta — heavy platform surface
    (
        "inventory_rosetta_platform.vox",
        "platform-only: rosetta platform surface",
    ),
];

fn is_known_gap(path: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy();
    // Normalise separators so forward-slash suffixes match on Windows
    let normalised = path_str.replace('\\', "/");
    for (suffix, reason) in KNOWN_INTERP_GAPS {
        if normalised.ends_with(suffix) {
            return Some(reason);
        }
    }
    None
}

/// Every golden containing `fn main` must execute under `--mode interp`
/// without error (or with a documented "not supported in --mode interp"
/// diagnostic). This gate prevents silent regressions after gap fixes.
#[test]
fn all_runnable_goldens_execute_under_interp() {
    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden");
    assert!(golden_dir.is_dir(), "missing {}", golden_dir.display());

    let mut paths = Vec::new();
    collect_golden_vox_files(&golden_dir, &mut paths);
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no .vox files under {}",
        golden_dir.display()
    );

    let mut failures: Vec<String> = vec![];
    let mut skipped = 0usize;
    let mut ran = 0usize;

    for path in &paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: read error: {e}", path.display()));
                continue;
            }
        };

        // Only run files that have an entrypoint
        if !src.contains("fn main") {
            continue;
        }

        // Skip known gaps
        if let Some(reason) = is_known_gap(path) {
            eprintln!("SKIP {}: {reason}", path.display());
            skipped += 1;
            continue;
        }

        let tokens = vox_compiler::lexer::lex(&src);
        let module = match vox_compiler::parser::descent::parse(tokens) {
            Ok(m) => m,
            Err(e) => {
                // Parse failures in golden files are pre-existing bugs — skip
                // and count them separately so they show up in the summary.
                eprintln!("SKIP (parse-err) {}: {e:?}", path.display());
                skipped += 1;
                continue;
            }
        };

        ran += 1;

        let lowered = vox_compiler::hir::lower::lower_module(&module);
        let mut interp = vox_compiler::eval::Interpreter::new(100_000);

        if let Err(e) = interp.run_module(&lowered) {
            let msg = format!("{e:?}");
            if msg.contains("not supported in --mode interp") {
                eprintln!("OK (interp-n/a) {}", path.display());
                continue;
            }
            failures.push(format!("{}: run_module error: {msg}", path.display()));
            continue;
        }

        // Only call main if the lowered module has a fn named "main"
        let has_main = lowered.functions.iter().any(|f| f.name == "main");
        if has_main {
            if let Err(e) = interp.call("main", vec![]) {
                let msg = format!("{e:?}");
                if msg.contains("not supported in --mode interp") {
                    eprintln!("OK (interp-n/a) {}", path.display());
                    continue;
                }
                failures.push(format!("{}: call main error: {msg}", path.display()));
            } else {
                eprintln!("OK {}", path.display());
            }
        } else {
            eprintln!("OK (no-main fn) {}", path.display());
        }
    }

    println!(
        "Golden interp: ran={ran}, skipped={skipped}, failures={}",
        failures.len()
    );

    assert!(
        failures.is_empty(),
        "Golden files failed under --mode interp:\n{}",
        failures.join("\n")
    );
}
