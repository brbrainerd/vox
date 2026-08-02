# Vox Axis / MENS Tool Mapping

Skills use Claude Code tool names. When you encounter these in a skill while running under
Vox Axis (MENS or any other model in the Vox orchestrator), use your platform equivalent:

| Skill references | Vox Axis / MENS equivalent |
|-----------------|----------------------|
| `Read` (file reading) | `vox_read_file` |
| `Write` (file creation) | `vox_write_file` |
| `Edit` (file editing) | No dedicated tool found — read via `vox_read_file`, then write the full updated content back via `vox_write_file` |
| `Bash` (run commands) | `vox_run_shell` |
| `Grep` (search file content) | No direct equivalent found as of 2026-08-01 — for structural/code search prefer the `vox-graph` skill's tools (`vox_search_structural`/`vox_search_neighbors`) once Phase 2 of the improvement roadmap lands it natively for Vox Axis; today it is Claude-Code-only |
| `Glob` (search files by name) | No direct equivalent found as of 2026-08-01 |
| `TodoWrite` (task tracking) | No direct equivalent found as of 2026-08-01 — track progress in prose or via `vox_memory_log` |
| `Skill` tool (invoke a skill) | `vox_skill_use` |
| `Task` tool (dispatch subagent) | `vox_submit_task` (submit) + `vox_task_status` (poll) — see `crates/vox-plugin-skill-orchestrator/orchestrator.skill.md` |

## No native subagent-review loop

Vox Axis's `vox_submit_task`/`vox_task_status` is fire-and-poll, not a blocking dispatch-and-return
like Claude Code's `Task` tool. Skills that assume synchronous subagent dispatch
(`subagent-driven-development`, `dispatching-parallel-agents`) need to poll `vox_task_status`
until completion instead of awaiting a return value directly.

## Additional Vox Axis tools with no Claude Code equivalent

| Tool | Purpose |
|------|---------|
| `vox_memory_store` / `vox_memory_recall` | Persist and retrieve facts across sessions (`crates/vox-plugin-skill-memory/memory.skill.md`) |
| `vox_populi_local_status` | Inspect mens worker mesh labels/registry visibility |
| `vox_search_status` / `vox_search_structural` / `vox_search_neighbors` | Graph-first structural search (see the `graphify` skill once Phase 2 lands it natively; today this is the Claude-Code-only `vox-graph` skill's tool set — see `assets/skills/vox-graph/SKILL.md`) |
