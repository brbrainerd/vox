//! Phase 5 — emit a generated `main()` for binaries compiled from `.vox` files.
//!
//! The generated binary must:
//! 1. Initialize the DB.
//! 2. Register the process-global `HirModule` so workflow bodies can look up
//!    activity bodies via `current_hir_module()`. The HirModule is serialized
//!    to JSON at codegen time and embedded as a `const &str` in the generated
//!    binary (ADR-041 §6(b); HirModule already derives `Serialize` +
//!    `Deserialize`).
//! 3. Register every `@scheduled` function with the runtime scheduler and start it.
//! 4. (Phase-5 follow-up) Boot an HTTP server for `@query` / `@mutation` /
//!    `@server` endpoints. The workspace does not currently ship a
//!    `vox-http-runtime` crate exposing a `serve(db) -> Handle` entry point
//!    (only `vox-http-client` exists), so we emit a TODO comment instead of
//!    fabricating a call to a non-existent symbol.
//! 5. Wait for `ctrl_c` and shut down gracefully.
//!
//! The duration-literal parser (`"1s"`, `"500ms"`, `"1m"`, `"1h"`, `"1d"`) is
//! emitted inline so the generated binary has no extra runtime dependency on
//! a parser helper. Bare integer strings parse as seconds for ergonomics.
//!
//! See `docs/superpowers/plans/2026-05-23-durable-functions-completion.md`
//! (Task 5.1) for the parent plan and `mod.rs` for the `emit_main_boot`
//! re-export consumers should use.

use vox_compiler::hir::{HirEndpointKind, HirModule};

/// Emit `main.rs`-eligible Rust source containing a `#[tokio::main] async fn main()`
/// plus a small `parse_duration_literal` helper.
///
/// The returned string is intentionally pre-formatted (not run through
/// `prettyplease`) — the snapshot test locks the exact shape, so reviewers
/// see emit-level changes directly rather than through a pretty-print pass.
pub fn emit_main_boot(module: &HirModule) -> String {
    let scheduled_fns: Vec<(&str, &str)> = module
        .functions
        .iter()
        .filter_map(|f| {
            f.schedule_interval
                .as_deref()
                .map(|interval| (f.name.as_str(), interval))
        })
        .collect();

    let has_http_endpoints = module.endpoint_fns.iter().any(|e| {
        matches!(
            e.kind,
            HirEndpointKind::Query | HirEndpointKind::Mutation | HirEndpointKind::Server
        )
    });

    let mut out = String::new();
    out.push_str(MAIN_BOOT_HEADER);

    out.push_str(
        "#[tokio::main]\nasync fn main() -> anyhow::Result<()> {\n",
    );

    // 1. DB init.
    out.push_str("    // 1. Initialize the durable database.\n");
    out.push_str(
        "    let db = std::sync::Arc::new(::vox_db::VoxDb::connect(::vox_db::DbConfig::default()).await?);\n\n",
    );

    // 2. HIR module registration (ADR-041 §6(b)).
    out.push_str("    // 2. Register the process-global HirModule for workflow body lookup.\n");
    out.push_str("    //    The HirModule is serialized to JSON at codegen time and embedded\n");
    out.push_str("    //    as a `const &str` (see `load_hir_module_from_embedded` below).\n");
    out.push_str("    ::vox_workflow_runtime::workflow::set_current_hir_module(\n");
    out.push_str("        load_hir_module_from_embedded()\n");
    out.push_str("    );\n\n");

    // 3. Scheduler.
    if scheduled_fns.is_empty() {
        out.push_str("    // 3. No `@scheduled` functions in this module — scheduler not started.\n\n");
    } else {
        out.push_str("    // 3. Register every `@scheduled` function and start the runner.\n");
        for (name, interval) in &scheduled_fns {
            out.push_str(&format!(
                "    ::vox_workflow_runtime::scheduled::register(\n\
                 \x20       {name_lit},\n\
                 \x20       parse_duration_literal({interval_lit}),\n\
                 \x20       std::sync::Arc::new(|| Box::pin(async {{\n\
                 \x20           let _ = {fn_call}().await;\n\
                 \x20           Ok(())\n\
                 \x20       }})),\n\
                 \x20       db.clone(),\n\
                 \x20   ).await?;\n",
                name_lit = rust_string_literal(name),
                interval_lit = rust_string_literal(interval),
                fn_call = name,
            ));
        }
        out.push_str("    let scheduled_handle = ::vox_workflow_runtime::scheduled::start(db.clone()).await?;\n\n");
    }

    // 4. HTTP boot (TODO when endpoints exist).
    if has_http_endpoints {
        out.push_str("    // 4. HTTP server boot.\n");
        out.push_str("    // TODO(phase5-followup): the workspace does not yet ship a vox-http-runtime\n");
        out.push_str("    // crate with a `serve(db) -> Handle` entry point. The generated lib.rs / Axum\n");
        out.push_str("    // emit handles HTTP today (see emit_main in http.rs); once a reusable runtime\n");
        out.push_str("    // crate exists, this is where to spawn it.\n\n");
    } else {
        out.push_str("    // 4. No `@query` / `@mutation` / `@server` endpoints — HTTP server not booted.\n\n");
    }

    // 5. Shutdown.
    out.push_str("    // 5. Wait for shutdown signal, then gracefully stop background tasks.\n");
    out.push_str("    tokio::signal::ctrl_c().await?;\n");
    if !scheduled_fns.is_empty() {
        out.push_str("    scheduled_handle.shutdown().await;\n");
    }
    out.push_str("    Ok(())\n");
    out.push_str("}\n\n");

    out.push_str(PARSE_DURATION_LITERAL);
    out.push('\n');
    out.push_str(&emit_hir_embed_helper(module));

    out
}

/// Emit the `load_hir_module_from_embedded()` helper.
///
/// The HirModule is serialized to JSON at codegen time and embedded into the
/// generated binary as a raw-string `const &str`. At boot, `main()` decodes
/// the JSON via `::serde_json::from_str(...)` and hands the resulting
/// `HirModule` to `::vox_workflow_runtime::workflow::set_current_hir_module`.
///
/// The raw-string delimiter is chosen dynamically: we start with `r#"…"#`
/// and bump the hash count whenever the serialized JSON contains a matching
/// closing delimiter that would prematurely terminate the literal. JSON
/// escapes `"` as `\"`, so in practice `"#` cannot appear and a single-hash
/// delimiter suffices — but the loop guarantees correctness for any input.
fn emit_hir_embed_helper(module: &HirModule) -> String {
    let serialized = serde_json::to_string(module)
        .expect("HirModule serializes to JSON (Serialize derive guaranteed at decl.rs:29)");

    // Pick the smallest `#`-count whose closing delimiter (`"` + N hashes)
    // does not appear inside the serialized JSON. Start at 1 and grow.
    let mut hash_count: usize = 1;
    loop {
        let mut closer = String::with_capacity(1 + hash_count);
        closer.push('"');
        for _ in 0..hash_count {
            closer.push('#');
        }
        if !serialized.contains(&closer) {
            break;
        }
        hash_count += 1;
        // Sanity: bound the search so a pathological input cannot infinite-loop.
        // 16 hashes is absurdly more than any real input would need.
        debug_assert!(hash_count <= 16, "raw-string delimiter escalation exceeded 16 hashes");
        if hash_count > 16 {
            break;
        }
    }
    let hashes: String = std::iter::repeat('#').take(hash_count).collect();

    let mut out = String::new();
    out.push_str(
        "/// Decode the codegen-time-embedded HirModule JSON back into a runtime\n\
         /// `HirModule`. Called once from `main()` to register the process-global\n\
         /// module for `current_hir_module()` lookups (ADR-041 §6(b)).\n",
    );
    out.push_str("fn load_hir_module_from_embedded() -> ::vox_compiler::hir::HirModule {\n");
    out.push_str(&format!(
        "    const EMBEDDED_HIR: &str = r{hashes}\"{serialized}\"{hashes};\n"
    ));
    out.push_str(
        "    ::serde_json::from_str(EMBEDDED_HIR)\n\
         \x20       .expect(\"embedded HirModule deserializes (codegen wrote it; should round-trip)\")\n",
    );
    out.push_str("}\n");
    out
}

/// File-level doc comment + uses for the generated `main.rs`.
const MAIN_BOOT_HEADER: &str = "\
// Generated by Vox Compiler — durable-binary main() boot routine.
// See docs/superpowers/plans/2026-05-23-durable-functions-completion.md (Task 5.1)
// and docs/src/adr/041-durable-functions-completion-2026.md §6(b).
//
// The contract here is:
//   DB init  →  HIR register  →  scheduler register+start  →  ctrl-C  →  graceful shutdown.
// HTTP serving still lives in the per-route emit (http.rs); factoring that
// into a reusable vox-http-runtime crate is tracked under §6(c).

";

/// Inline helper emitted alongside `main()` so the binary has no extra runtime
/// dependency on a duration-parsing crate.
const PARSE_DURATION_LITERAL: &str = "\
/// Parse a `@scheduled(\"…\")` interval literal into a `std::time::Duration`.
///
/// Accepts: `\"500ms\"`, `\"5s\"`, `\"1m\"`, `\"2h\"`, `\"1d\"`. A bare integer is
/// interpreted as seconds (`\"30\"` → 30s) for ergonomic parity with cron-ish
/// schedulers. Falls back to `Duration::from_secs(60)` on parse failure to
/// avoid a panic at boot — the runtime will log the bad literal once the
/// scheduler tracing is wired up.
fn parse_duration_literal(s: &str) -> std::time::Duration {
    let s = s.trim();
    let (digits, unit_secs): (&str, u64) = if let Some(rest) = s.strip_suffix(\"ms\") {
        return std::time::Duration::from_millis(rest.trim().parse().unwrap_or(60_000));
    } else if let Some(rest) = s.strip_suffix('s') {
        (rest.trim(), 1)
    } else if let Some(rest) = s.strip_suffix('m') {
        (rest.trim(), 60)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest.trim(), 60 * 60)
    } else if let Some(rest) = s.strip_suffix('d') {
        (rest.trim(), 60 * 60 * 24)
    } else {
        (s, 1)
    };
    let n: u64 = digits.parse().unwrap_or(60);
    std::time::Duration::from_secs(n * unit_secs)
}
";

/// Emit `s` as a Rust string literal with escaping for `\\`, `\"`, and control
/// characters. Used for `@scheduled` function names + interval literals — both
/// untrusted-ish (they come from source).
fn rust_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
