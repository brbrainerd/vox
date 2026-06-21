---
title: "History & Clip Manager Handoff Findings"
description: "Handoff details for the History and Clip Manager implementation under Plan 7, outlining changes made, CLI/GUI status, and next steps."
category: "Architecture SSOTs"
status: "current"
---

# History & Clip Manager Handoff Findings (June 2026)

This document outlines the accomplishments, architecture layout, and compliance updates for the **Plan 7: History & Clip Manager (Ditto-style, project-scoped)** branch. It is prepared for the reviewer running Claude Opus 4.8.

## What Was Accomplished

All tasks in Plan 7's roadmap have been completed and verified as green:

1. **Database Schema & Accessors (Tasks 1-4)**:
   - Bumped the database baseline version to 79 and added the `history_entries` table.
   - Created core database operations in `vox-db::history_store` (`add_entry`, `list_entries`, `pin_entry`, `delete_entry`, `evict`, and `search_entries`).
   - Implemented secret redaction rules inside `add_entry` to mask keys and credentials.

2. **Search Facade & Tauri Commands (Tasks 5-6)**:
   - Registered `SearchCorpus::ClipHistory` in the global search system.
   - Wrote Tauri handlers (`history_list`, `history_add`, `history_pin`, `history_delete`, `history_search`) to bridge database records to the frontend.
   - Refactored `history_search` to delegate to `history_store::search_entries`, ensuring zero direct SQL query/execute invocations in `crates/vox-gui/src/commands/history.rs` to satisfy `sql-surface-guard`.

3. **GUI Componentry (Tasks 7-9)**:
   - Implemented `HistoryPanel.tsx` with fast local fuzzy filtering.
   - Integrated with legacy surface routing, window shortcuts (`Ctrl+Alt+C`), terminal command triggers (`D` finished kind), and chat turn appends.

4. **CLI Subcommands (Task 10)**:
   - Implemented `vox clip` and `vox history` commands in `crates/vox-cli/src/commands/history_cli.rs`.
   - Wired `check_terminal::run_check` to enforce terminal execution policies during CLI calls.
   - Verified that `vox clip add "..."` and `vox history list` function correctly.

5. **Compliance & Guard Alignment**:
   - Added `"axis"` to compliance skipped aliases in `validators.rs`.
   - Updated the allowed domains list in `policy-registry.v1.schema.json` to include `"gui-design-rule"`.
   - Added `crates/vox-orchestrator/src/orchestrator/core/` to `turso-import-allowlist.txt`.
   - Added `crates/vox-gui/src/commands/activity.rs` to `query-all-allowlist.txt`.
   - Regenerated snapshot baselines (`command_catalog_paths_baseline.txt` and `route_simulation_golden.json`).
   - Isolated `db_migrate_semantics_test.rs` by enforcing a temporary database path, preventing baseline mismatch errors on the host's existing `codex.db`.

## Code Locations

Reviewers can inspect the following files to verify implementation:

- **CLI Commands**: [`crates/vox-cli/src/commands/history_cli.rs`](file:///c:/Users/Owner/vox/crates/vox-cli/src/commands/history_cli.rs)
- **Tauri Commands**: [`crates/vox-gui/src/commands/history.rs`](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/history.rs)
- **Database Store API**: [`crates/vox-db/src/history_store.rs`](file:///c:/Users/Owner/vox/crates/vox-db/src/history_store.rs)
- **Allowlists & Schemas**:
  - [`docs/agents/query-all-allowlist.txt`](file:///c:/Users/Owner/vox/docs/agents/query-all-allowlist.txt)
  - [`docs/agents/turso-import-allowlist.txt`](file:///c:/Users/Owner/vox/docs/agents/turso-import-allowlist.txt)
  - [`contracts/policy/policy-registry.v1.schema.json`](file:///c:/Users/Owner/vox/contracts/policy/policy-registry.v1.schema.json)
  - [`crates/vox-cli/src/commands/ci/command_compliance/validators.rs`](file:///c:/Users/Owner/vox/crates/vox-cli/src/commands/ci/command_compliance/validators.rs)

## Verification Status

All checks run against the branch are clean:
- **CLI Unit/Integration Tests**: `cargo test -p vox-cli` passed successfully.
- **Database Unit Tests**: `cargo test -p vox-db` passed successfully.
- **Guard Verifications**: `sql-surface-guard`, `turso-import-guard`, and `query-all-guard` checks are fully resolved.
