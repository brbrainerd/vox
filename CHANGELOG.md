# Changelog

All notable changes to the Vox project are documented here.

## [Unreleased]

### Added
- **Parser**: Trailing comma support in function parameter lists (A-072/A-100)
- **Parser**: Duplicate parameter name detection with clear error message (A-074/A-101)
- **Parser**: Error recovery test coverage (A-099)
- **Parser**: `filter_fields` support in `VectorIndexDecl` parsing
- **Typeck**: Lambda parameter type checking test (A-092)
- **Typeck**: Lambda outer scope capture test (A-093)
- **Typeck**: Match arm variable binding test (A-094)
- **Typeck**: Match exhaustiveness error test (A-095)
- **Store**: `CodeStore::dry_run_migration()` — report pending migrations without applying (B-059)
- **Store**: `CodeStore::health_check()` — `PRAGMA integrity_check` wrapper (B-060)
- **Store**: `CodeStore::batch_insert()` for bulk artifact insertion (B-062)
- **Store**: Pagination support (`LIMIT`/`OFFSET`) in `list_components` (B-063)
- **Store**: Relevance threshold filtering in `recall_memory` (B-064)
- **VoxDb**: `DbConfig::from_env()` for environment-based configuration (B-065)
- **VoxDb**: Retry logic (3× with backoff) in `VoxDb::connect` (B-066)
- **VoxDb**: `VoxDb::transaction()` wrapper for atomic operations (B-067)
- **VoxDb**: Integration test for in-memory connection (B-068)
- **AGENTS.md**: Phase 5 VoxPM roadmap merged from `PLAN.md` (B-076)
- **Docs**: `vox-runtime/README.md` — actor model architecture (B-112)
- **Docs**: `vox-pm/README.md` — CAS store architecture (B-113)
- **Docs**: mdBook search enabled with full-text indexing (A-136)
- **Docs**: Automated API reference pipeline `vox doc` (A-142)
- **Docs**: Decorator and Keyword manifests in JSON format (B-121/B-122)
- **Docs**: OpenGraph/SEO metadata and social sharing support (B-125)
- **Docs**: RSS/Atom feed generation for release notes (B-124)
- **CI**: Documentation build check and Rustdoc integration (B-117/B-118)
- **CI**: Dashboard API `dead_code` warnings suppressed (future integration)
- **OpenCode CLI**: `vox opencode` subcommand tree (install, setup, doctor, status, dashboard, spawn, review, config, sync, logs, share)
- **OpenCode CLI**: `vox opencode install` — downloads OpenCode AI and scaffolds config
- **OpenCode CLI**: `vox opencode doctor` — preflight check (binary, MCP, LSP, config, version)
- **OpenCode CLI**: `vox opencode dashboard` — launches embedded real-time agent dashboard
- **OpenCode CLI**: `vox completions <shell>` — generate shell completion scripts
- **OpenCode CLI**: `vox mcp-docs` — auto-generate MCP tool reference markdown table
- **OpenCode Integration**: `opencode.json` with version pinning (`opencode_version: >=0.2.0`)
- **OpenCode Integration**: Plugin API compatibility shim for OpenCode < 0.2.0
- **OpenCode Integration**: GitHub Actions workflow for `vox opencode doctor` in CI
- **MCP Server**: Protocol version negotiation (server echoes client's `protocolVersion`)
- **MCP Server**: 34 new tools (102 total): A2A messaging, VCS snapshots, JJ-inspired oplog/conflicts/workspaces, OpenCode bridge tools
- **MCP Tools**: `vox_map_opencode_session`, `vox_record_cost`, `vox_heartbeat`, `vox_cost_history`
- **MCP Tools**: `vox_a2a_send`, `vox_a2a_inbox`, `vox_a2a_ack`, `vox_a2a_broadcast`, `vox_a2a_history`
- **MCP Tools**: `vox_snapshot_*`, `vox_oplog`, `vox_undo`, `vox_redo`
- **MCP Tools**: `vox_workspace_*`, `vox_conflict_*`, `vox_change_*`, `vox_vcs_status`
- **Dashboard**: Redesigned with dark theme, glassmorphism, D3.js topology, SSE event log
- **Dashboard**: VCS panel, gamification panel, cost charts
- **Docs**: `docs/opencode-integration.md` — user-facing setup guide
- **Docs**: `docs/architecture/opencode-bridge.md` — technical deep-dive
- **Docs**: `docs/mcp-tool-reference.md` — auto-generated from 102 MCP tool schemas
- **Docs**: `docs/troubleshooting-faq.md` — common issues: port conflicts, MCP timeouts, LSP crashes
- **AGENTS.md**: Updated with 102 MCP tool list, OpenCode bridge section, new documentation links
- **CLI UX**: Colored output with actionable error suggestions in `vox opencode` commands


### Fixed
- **Store**: Replaced `.unwrap()` on embedding `try_into()` with proper error handling (B-056)
- **Normalize**: All `AstNode` variants now have explicit cases (no wildcard fallthrough) (B-058)
- **LSP**: Removed unused imports in `main.rs`

### Removed
- `PLAN.md` — content merged into `AGENTS.md` §3 (B-076)
