---
title: "External repositories & workspace SSOT"
description: "Official documentation for External repositories & workspace SSOT for the Vox language. Detailed technical reference, architecture guides"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# External repositories & workspace SSOT

Single source of truth for **repository identity**, **layout-derived affinity**, and **tenant-scoped** on-disk paths. Applies to the Vox monorepo and arbitrary Git checkouts.

## Invariants

1. **Repository root** — Prefer the Git work tree root (ancestor with `.git`). If there is no Git checkout, fall back to the canonicalized starting path (typically process CWD or a client override).
2. **`repository_id`** — Stable 16-hex string: `blake3(origin_url + NUL + canonical_root_path)` when `remote.origin.url` is readable from `.git/config`; otherwise `blake3(canonical_root_path)` only.
3. **Tool CWD** — Git MCP tools use `current_dir` = Git work tree (or repository root). Cargo MCP tools use `current_dir` = repository root and return a structured error when the root is not a Cargo package/workspace.
4. **Affinity groups** — If `repo_root/Vox.toml` contains a non-empty `affinity_groups` array, `load_from_config` builds the registry from explicit `name` + `patterns` (glob strings). Otherwise `AffinityGroupRegistry::detect_from_repository_layout` (in `vox-orchestrator`) prefers, in order:
   - Cargo `[workspace].members` (including simple `crates/*` expansion),
   - Node `package.json` `workspaces` (incl. Yarn object form) and `pnpm-workspace.yaml` `packages` (glob expansion to dirs with `package.json`),
   - Python root (`pyproject.toml` / `setup.py`),
   - Go root (`go.mod`),
   - `crates/` directory scan,
   - single catch-all `**/*`.
5. **Orchestrator memory** — `vox-mcp` shards file-backed memory under `repo_root/.vox/cache/repos/<repository_id>/memory/` (and `MEMORY.md` beside it) so concurrent opens of different repos do not share the same relative `./memory` tree.
6. **CLI benchmark telemetry vs MCP** — Opt-in Codex rows use `bench:<repository_id>` (see `VoxDb::record_benchmark_event`). Subprocesses spawned with a different CWD than the IDE/MCP server should set **`VOX_REPOSITORY_ROOT`** to the same logical repo root MCP discovered so `repository_id` (and thus session keys) stay aligned.
7. **Sessions** — JSONL sessions default to `.sessions/<repository_id>/` when using MCP `ServerState::new`; `SessionConfig.repository_id` is set so dual-written Codex `agent_sessions.task_snapshot` JSON includes the same tenant id.
8. **Codex / Turso rows** — Repo-scoped *filesystem* paths use `repository_id`; optional future migrations may add a `repository_id` column (or composite keys) on Codex tables per ADR 004 — not required for MCP memory/session sharding above.
9. **Agent scopes** — `.vox/agents/{name}.md` `scope:` lists are parsed by `vox_repository::load_agent_scopes`; task paths are checked with `normalize_task_path`.
10. **Cross-repo working set** — Explicit polyrepo manifests live at `repo_root/.vox/repositories.yaml`; Vox does not ambient-scan the whole machine for unrelated clones.
11. **Cross-repo refresh cache** — Re-resolved catalog snapshots and related metadata live under `repo_root/.vox/cache/repos/<repository_id>/`.

## MCP tools

| Tool | Behavior |
| :--- | :--- |
| `vox_git_*` | `current_dir` = Git root (see `git_tools::git_cwd`); subprocesses use `tokio::process` from the async tool dispatcher. |
| `vox_validate_file`, `vox_run_tests`, `vox_check_workspace`, `vox_test_all`, `vox_build_crate`, `vox_lint_crate`, `vox_coverage_report` | `current_dir` = repository root when invoking `cargo`; `tokio::process` + `tokio::fs` for validate. `vox_lint_crate` runs TOESTUB via `tokio::task::spawn_blocking` after clippy. |
| `vox_repo_index_status` / `vox_repo_index_refresh` | Bounded walk of `repository.root`; optional JSON cache under `.vox/cache/repos/<repository_id>/repo_index.json`. |

## Config

- **`VoxConfig::load_from_repo_root`** (`vox-config`) — Applies `repo_root/Vox.toml` before CWD `Vox.toml`, then env. Use when loading settings from a discovered repository root.
- **Cross-repo catalog manifest** — `.vox/repositories.yaml` is the local-first workspace manifest for cataloged repositories. It may include local roots plus remote adapter descriptors (`remote_mcp`, `remote_git_host`, `remote_search_service`) without weakening single-repo path safety.

## Crates

**Policy:** New code that needs Git root, `repository_id`, workspace layout, or agent scope parsing must depend on **`vox-repository`** (and `vox-config` for `Vox.toml`), not ad-hoc `std::env::current_dir` + manual walks in `vox-cli` or other crates.

| Crate | Role |
| :--- | :--- |
| `vox-repository` | `discover_repository`, `RepositoryContext` (`has_vox_agents_dir`, `vox_toml`), `RepoCapabilities`, layout helpers (`cargo_workspace_member_dirs`, `node_workspace_packages`, `python_roots`, `go_roots`), `load_agent_scopes`, `normalize_task_path`. |
| `vox-orchestrator` | `load_from_config` / `AffinityGroupRegistry::detect_from_repository_layout`, sessions, memory config consumed by MCP. |
| `vox-mcp` | `ServerState::repository`, git/compiler/task/repo_index wiring. Included in the root workspace (`cargo check --workspace` / CI). |

## Cross-repo catalog

Use the repo catalog when you want one operator workflow to query several repositories without rebinding the MCP server root.

Current policy:

- catalog membership is explicit
- each local entry resolves into its own `RepositoryContext`
- remote entries are adapter metadata first, query backends later
- cross-repo paths stay per-repository; there is no shared global path namespace

See also: [Cross-repo querying and observability](../archive/research-2026-q1/cross-repo-query-observability.md).

## Related

- [`orchestration-unified.md`](orchestration-unified.md) — MCP/DeI plan alignment, migration flags, benchmark telemetry env.
- [`mens.md`](populi.md) — `VOX_MESH_*` contract, local registry, HTTP control plane.
- ADR 004 (`docs/src/adr/004-codex-arca-turso-ssot.md`) — Codex env and Turso.
- `AGENTS.md` §2.2.2 — short agent-oriented summary.


