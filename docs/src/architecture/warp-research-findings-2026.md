---
title: "Warp Terminal Research Findings (2026)"
description: "Systematic scan of warpdotdev/warp for high-value primitives, license compatibility, and feature gaps relevant to Vox."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Competitive ecosystem analysis with license and architecture findings."
---
# Warp Terminal Research Findings (2026-04-29)

## Summary

Scanned [warpdotdev/warp](https://github.com/warpdotdev/warp) (~60 crates,
Apache-licensed **AGPL-3.0-only**) for primitives, design patterns, and
feature gaps that could advance Vox. Key finding: **all Warp source is
AGPL-3.0-only and cannot be vendored into Vox (Apache-2.0)** per `deny.toml`
and ADR-026. Warp is exclusively a design reference.

## License Determination

| Source | Finding |
|---|---|
| `Cargo.toml` workspace | `license = "AGPL-3.0-only"` |
| All crate `Cargo.toml`s | `license.workspace = true` → inherits AGPL-3.0-only |
| Per-file headers | None (workspace license governs) |
| `LICENSE-MIT` file | Covers contributor CLA history; does NOT grant MIT use of artifacts |
| Vox `deny.toml` | Already denies `AGPL-3.0-only` and `AGPL-3.0-or-later` |

**Conclusion:** Direct copy, vendor, or git-dep of any Warp crate is
**prohibited** under Vox's existing `deny.toml` policy and ADR-026.

## Warp Architecture Overview

Warp is a Rust-based terminal emulator ("agentic development environment") with:
- **WarpUI** — custom Entity-Component-Handle GPU UI framework (Flutter-inspired)
- **warp_core** — platform abstractions, feature flags
- **warp_terminal** — PTY, shell management, terminal block model
- **ai/** (in app/) — Agent Mode, codebase indexing
- **GraphQL** + **WebSocket** transport to hosted server (Warp Drive / cloud sync)
- **Diesel + SQLite** persistence layer
- Cross-platform: macOS, Windows, Linux, WASM target

## Crate Tier Map

### Tier A — Design reference (study, clean-room reimplement)

| Warp crate | Design concept to extract | Vox application |
|---|---|---|
| `command-signatures-v2` | Structured grammar of CLI invocations: flags, args, redirects, danger annotations | `vox-exec-grammar` — AST validator backing `contracts/terminal/exec-policy.v1.yaml`. This was the single biggest Vox gap confirmed by this research. |
| `input_classifier` | Distinguishes typed command vs natural language vs structured action | `vox-cli` dispatch routing (`dispatch.rs`) for `vox ask` vs `vox run` vs subcommand |
| `natural_language_detection` | Lightweight NL detector for LLM-vs-deterministic routing | Pairs with input_classifier; feeds `vox-orchestrator` routing |
| `computer_use` | Anthropic computer-use tool surface (screenshot, click, key-chord) | `vox-skills` `@mcp.tool` set for desktop-control agent capability |
| `isolation_platform` | OS-level process sandboxing abstraction | Vox has `--isolation wasm`; this covers the OS-process-isolation tier |
| `settings_value_derive` | Derive macro for compile-time config schema | `vox-config` hardening |
| `channel_versions` | Versioned async channels | `vox-actor-runtime` mailbox/`vox-orchestrator` typed message versioning |

### Tier B — Alternate Apache-2.0 source available

| Warp crate | Preferred substitute | Notes |
|---|---|---|
| `sum_tree` | `zed-industries/zed` `crates/sum_tree` (Apache-2.0) | Same lineage (xi-editor → Zed → Warp). Zed's version is vendorable. |
| `string-offset` | Zed `crates/text` + `rope` primitives (Apache-2.0) | UTF-16/grapheme offset conversion for `vox-lsp` |
| `fuzzy_match` | `nucleo-matcher` (MIT, crates.io) — **already wired** in `vox-cli/src/fuzzy.rs` | Fix: `fuzzy-search` feature + dep declaration completed 2026-04-29 |
| `markdown_parser` | `pulldown-cmark` (MIT) | Warp's block-extension design informs `vox-doc-pipeline` extensions |
| `syntax_tree` | `tree-sitter` (MIT) | Already standard; Warp's wrapper is design reference only |
| `watcher` | `notify-debouncer-full` (MIT/Apache-2.0) | `vox-cli/src/watcher.rs` already exists using `notify = "7"` |

### Tier C — Drop / not relevant

`warpui`, `warpui_core`, `warpui_extras`, `warp_terminal`, `vim`, `editor`,
`firebase`, `graphql`, `warp_graphql_schema`, `warp_server_client`,
`onboarding`, `voice_input`, `prevent_sleep`, `app-installation-detection`,
`node_runtime`, `warp_js`, `handlebars`, `simple_logger` — Warp-specific
business logic, GPU UI framework, or Vox already has equivalents.

## Modernization Signals (gaps revealed by Warp's existence)

1. **AST-level command validation** — biggest gap. `contracts/terminal/exec-policy.v1.yaml`
   has policy but no parser. `vox-exec-grammar` scaffolded 2026-04-29 (TASK-3.x).
2. **`vox shell check` semantic depth** — currently shape-only per AGENTS.md.
   Needs `vox-exec-grammar` to grow teeth.
3. **Input routing intelligence** — `vox <text>` dispatch in `dispatch.rs` is
   blunt prefix matching; should classify `InputKind` before routing.
4. **Terminal block model in dashboard** — Warp's "one block per command" UX for
   `vox run` output and agent traces is a pure frontend/CSS-tier improvement.
5. **Codebase indexing ignore surface** — `.voxindexingignore` created 2026-04-29,
   informed by Warp's `.warpindexingignore` design.

## Deliverables Produced

| Artifact | Path |
|---|---|
| Third-party provenance ADR | `docs/src/adr/026-third-party-code-provenance.md` |
| `vox-exec-grammar` scaffold | `crates/vox-exec-grammar/` |
| `fuzzy-search` feature wire-up | `crates/vox-cli/Cargo.toml`, `src/lib.rs`; `Cargo.toml` workspace dep |
| `.voxindexingignore` | `.voxindexingignore` |
| This research doc | `docs/src/architecture/warp-research-findings-2026.md` |
| Research index update | Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`. |

## Next Steps (ordered by value)

1. **TASK-3.x**: Implement `vox-exec-grammar` parser (full tokeniser for POSIX +
   PowerShell cmdlet invocations). Wire into `vox shell check`.
2. **TASK-3.y**: Input classifier in `vox-cli/src/dispatch.rs` —
   `InputKind::VoxScript | NaturalLanguage | SubCommand | FuzzyCommand`.
3. **Vendor Zed `sum_tree`** — once TASK-3.x is stable, evaluate whether
   corpus/search indexing would benefit from a persistent B-tree.
4. **Computer-use skill** — `@mcp.tool` set in `vox-skills` for desktop agent actions.
