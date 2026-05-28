use std::path::PathBuf;

use super::ingest::{research_ingest_file, research_ingest_url};
use super::list_map::research_map_add;

/// Known OpenClaw primary sources for refresh (OC126).
const OPENCLAW_REFRESH_URLS: &[(&str, &str)] = &[
    (
        "architecture",
        "https://openclawlab.com/en/docs/concepts/system-architecture/",
    ),
    (
        "gateway",
        "https://openclawlab.com/en/docs/gateway/protocol/",
    ),
    ("remote", "https://openclawlab.com/en/docs/gateway/remote/"),
    (
        "failover",
        "https://openclawlab.com/en/docs/providers/failover/",
    ),
    (
        "openresponses",
        "https://openclawlab.com/en/docs/gateway/openresponses-http-api/",
    ),
];

/// Context-engineering research docs to ingest into ecosystem/context-engineering.
const CONTEXT_ENGINEERING_FILES: &[(&str, &str)] = &[
    (
        "context_engineering",
        "docs/src/research/res-context-engineering-2025-2026.md",
    ),
    (
        "context_rot",
        "docs/src/research/res-context-rot-long-window.md",
    ),
    (
        "prompt_caching",
        "docs/src/research/res-prompt-caching-observability.md",
    ),
    (
        "mcp_a2a",
        "docs/src/research/res-mcp-a2a-context-transfer.md",
    ),
    (
        "populi_policy",
        "docs/src/research/res-mens-context-policy.md",
    ),
    (
        "capability_map",
        "docs/src/research/res-context-capability-map.md",
    ),
];

/// Planning-mode research docs to ingest into ecosystem/planning-mode.
const PLANNING_MODE_FILES: &[(&str, &str)] = &[
    (
        "agentic_coding",
        "docs/src/research/res-planning-mode-agentic-coding-2026.md",
    ),
    (
        "capability_map",
        "docs/src/research/res-planning-mode-capability-map.md",
    ),
];

/// Re-fetch OpenClaw primary sources or ingest context-engineering docs; flags drift when content changes.
pub async fn research_refresh(vendor: &str, dry_run: bool) -> anyhow::Result<()> {
    if vendor == "openclaw" {
        if dry_run {
            println!(
                "Would refresh {} OpenClaw sources:",
                OPENCLAW_REFRESH_URLS.len()
            );
            for (topic, url) in OPENCLAW_REFRESH_URLS {
                println!("  {}: {}", topic, url);
            }
            return Ok(());
        }
        for (topic, url) in OPENCLAW_REFRESH_URLS {
            eprintln!("Fetching {}...", topic);
            research_ingest_url(
                vendor,
                topic,
                url,
                None,
                None,
                "official",
                Some(topic),
                Some("ecosystem/openclaw"),
                Some("refresh"),
                0.9,
            )
            .await?;
        }
        println!(
            "Refreshed {} OpenClaw sources. Run quarterly per codex-baas.md.",
            OPENCLAW_REFRESH_URLS.len()
        );
        return Ok(());
    }
    if vendor == "context_engineering" {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if dry_run {
            println!(
                "Would ingest {} context-engineering docs:",
                CONTEXT_ENGINEERING_FILES.len()
            );
            for (topic, rel) in CONTEXT_ENGINEERING_FILES {
                let p = base.join(rel);
                println!(
                    "  {}: {} ({})",
                    topic,
                    rel,
                    if p.exists() { "exists" } else { "missing" }
                );
            }
            return Ok(());
        }
        for (topic, rel) in CONTEXT_ENGINEERING_FILES {
            let path = base.join(rel);
            if !path.exists() {
                eprintln!("Skipping {} (not found at {})", topic, path.display());
                continue;
            }
            eprintln!("Ingesting {}...", topic);
            research_ingest_file(
                vendor,
                topic,
                &path,
                Some(topic),
                Some("ecosystem/context-engineering"),
                Some("context,memory,retrieval"),
                0.95,
            )
            .await?;
        }
        // Persist capability-map rows for context-engineering gaps
        research_refresh_context_capability_map().await?;
        println!(
            "Ingested context-engineering docs and capability map into ecosystem/context-engineering."
        );
        return Ok(());
    }
    if vendor == "planning_mode" {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if dry_run {
            println!(
                "Would ingest {} planning-mode docs:",
                PLANNING_MODE_FILES.len()
            );
            for (topic, rel) in PLANNING_MODE_FILES {
                let p = base.join(rel);
                println!(
                    "  {}: {} ({})",
                    topic,
                    rel,
                    if p.exists() { "exists" } else { "missing" }
                );
            }
            return Ok(());
        }
        for (topic, rel) in PLANNING_MODE_FILES {
            let path = base.join(rel);
            if !path.exists() {
                eprintln!("Skipping {} (not found at {})", topic, path.display());
                continue;
            }
            eprintln!("Ingesting {}...", topic);
            research_ingest_file(
                vendor,
                topic,
                &path,
                Some(topic),
                Some("ecosystem/planning-mode"),
                Some("planning,agentic,evidence,verification,schema,persistence"),
                0.95,
            )
            .await?;
        }
        println!("Ingested planning-mode docs into ecosystem/planning-mode.");
        return Ok(());
    }
    anyhow::bail!(
        "research-refresh supports vendor=openclaw, vendor=context_engineering, or vendor=planning_mode"
    );
}

/// Persist context-engineering capability-map rows (external guidance vs Vox state).
async fn research_refresh_context_capability_map() -> anyhow::Result<()> {
    let rows: Vec<(&str, &str, &str, &str, &str, &str, &str)> = vec![
        (
            "working_memory",
            "Active prompt, current files, recent messages, tool outputs",
            "MCP chat_tools, orchestrator memory window",
            "partial",
            "vox_ahead",
            "Reuse in Mens generate",
            "crates/vox-mcp/src/tools/chat_tools.rs, crates/vox-orchestrator/src/memory/manager.rs",
        ),
        (
            "episodic_memory",
            "Snapshots, conversation edges, version linkage",
            "Arca/Codex accessors; generate graph-aware mode",
            "partial",
            "parity",
            "Mens generate --context-mode graph-aware --conversation-id N",
            "crates/vox-arca/src/store_ext.rs, crates/vox-cli/src/commands/ai/generate.rs",
        ),
        (
            "semantic_memory",
            "Schema digest, KB results, module signatures",
            "SchemaDigest, vox-codex retrieval",
            "partial",
            "parity",
            "Augment Mens retrieval",
            "crates/vox-codex/src/schema_digest.rs",
        ),
        (
            "procedural_memory",
            "Playbooks, prompt templates, routing policy",
            "Prompt templates in training artifacts",
            "partial",
            "parity",
            "First-class runtime field",
            "crates/vox-cli/src/commands/ai/",
        ),
        (
            "context_editing",
            "Stale tool-result clearing before summarization",
            "MCP has some; Mens none",
            "partial",
            "vox_ahead",
            "Extract shared, wire Mens",
            "crates/vox-mcp/src/tools/chat_tools.rs",
        ),
        (
            "prompt_caching",
            "Stable prefixes, versioned blocks",
            "None",
            "missing",
            "external_ahead",
            "Defer; prep layout",
            "crates/vox-arca/src/store_ext.rs",
        ),
        (
            "a2a_handoff_durability",
            "Structured briefing, snapshot linkage",
            "store_a2a_snapshot in handoff; vox_a2a_snapshots",
            "partial",
            "parity",
            "Wire vox_a2a_edges for handoff chains",
            "crates/vox-orchestrator/src/handoff.rs",
        ),
        (
            "observability",
            "Token, latency, truncation, retrieval telemetry",
            "Minimal",
            "partial",
            "external_ahead",
            "Instrument LLM spans",
            "crates/vox-orchestrator/, crates/vox-mcp/",
        ),
        (
            "evals",
            "Targeted context evals (compaction, handoff, goal retention)",
            "Eval harness exists; no context evals",
            "missing",
            "external_ahead",
            "Add context evals",
            "crates/vox-eval/",
        ),
    ];
    for (area, ext_guidance, vox_evidence, status, direction, action, paths) in rows {
        research_map_add(
            "context_engineering",
            "capability_gap",
            area,
            ext_guidance,
            vox_evidence,
            status,
            direction,
            action,
            Some(paths),
        )
        .await?;
    }
    Ok(())
}
