# CLI → GUI Governance Gap Audit

**Date:** 2026-06-26
**Branch:** `claude/graphify-general-gui-ia`
**Scope:** Every `vox <group> <subcommand>` in the clap CLI tree (`crates/vox-cli/` + gated `crates/vox-ml-cli/` for mens/populi/oratio), classified by whether the GUI can reach/control it. Completes the deferred `CliOnly` coverage dimension.

## Method

- **CLI SSOT:** `vox commands --format json --include-nested` (reflects `VoxCliRoot::command()` at compile time) → 555 catalog entries / 503 invokable leaves. The default binary collapses feature-gated groups (`mens`, `populi`, `oratio`/`speech`, `train`) to single stubs; their real subcommand counts were recovered from the clap enums in `crates/vox-ml-cli/` (`PopuliAction`=22 mens actions, `PopuliCli`=18, `OratioAction`=9) and substituted. **Gated-corrected total: 549 leaf commands across 74 groups.**
- **GUI SSOT:** `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` `cliGroup` rows (which CLI groups have a surface home), plus the Tauri wrappers in `crates/vox-gui/src/commands/*.rs` and `execute_command` / `decoratorRegistry.ts` shell-outs in `crates/vox-gui/ui/src`.
- **Governance classes:**
  - `governed-wrapper` — group has typed `#[tauri::command]` wrappers and a live surface (GUI invokes directly, structured I/O).
  - `governed-exec` — reached only via `execute_command` / decorator command-cards (read-only shell-out, no typed wrapper).
  - `ungoverned` — exists in the CLI, `tier: 'none'` in the surface registry, no wrapper, no surface reaches it.

## Headline numbers

| Metric | Value |
|---|---|
| CLI groups | **74** |
| Total leaf commands (gated-corrected) | **549** |
| governed-wrapper (cmds) | 104 (8 groups) |
| governed-exec (cmds) | 56 (4 groups) |
| **ungoverned (cmds)** | **389 (62 groups)** |
| **% governed (command-weighted)** | **29.1%** |
| **% ungoverned** | **70.9%** |

**Roughly 7 in 10 CLI commands have no GUI path.** The governed surface is real but narrow: it concentrates on the AI/knowledge groups (`scientia`, `model`, `research`, `mens`, `populi`, `oratio`, `memory`, `repo`, `config`, `policy`, `llm`) and leaves the entire build/test/CI/data/admin spine CLI-only.

### Governed groups (the 12 with GUI presence)

- **wrapper:** `scientia` (61), `model` (20), `research` (10), `config` (5), `policy` (5), `commands`/catalog (1), `memory` (1), `llm` (1)
- **exec / decorator cards:** `mens` (22), `populi` (18), `oratio` (9), `repo` (7)
- (Plus GUI-only surfaces with no top-level clap group: `skill` via MCP `vox_skill_*` tools, `ludus`/gamify via typed wrappers, `search`. These govern *capabilities* that aren't CLI groups.)

Note that several "governed" groups are governed *shallowly*: `mens`/`populi`/`oratio` are three read-only command-cards each (status/probe/snapshot) over 22/18/9 real subcommands — the training, serving, corpus, federation, and lifecycle subcommands are not reachable. Counting them governed at group level overstates true coverage.

## The biggest ungoverned clusters

| Group | Cmds | Nature | Recommended home |
|---|---:|---|---|
| **`ci`** | **157** | CI gates, language rules, audits, benches | **NEW `Develop > CI`** — read-only gate dashboard + run actions |
| **`db`** | **77** | vox-db admin, query, publication tables | **NEW `Knowledge > Database`** — read-only query panel + admin actions |
| `ext` | 11 | extension management | NEW `System > Settings > Extensions` |
| `fabrica` | 9 | aliases of build/check/test/run | fold into `Develop > Workspace` |
| `pm` | 9 | project management | NEW `Operate > Project Mgmt` |
| `codex` | 8 | codex agent runner | `Develop > Harness` |
| `graphify` | 8 | knowledge-graph build/query | NEW `Knowledge > Graph` |
| `secrets` | 8 | secret store | NEW `System > Settings > Secrets` (needs wrapper) |
| `telemetry` | 8 | telemetry config/spool | NEW `System > Settings > Telemetry` |
| `auth` | 7 | account/auth | NEW `System > Settings > Account` |
| `plugin` | 7 | plugin management | fold into `Develop > Skills` (Plugins tab) |
| `audit` | 6 | audit runners | NEW `System > Settings > Audits` |

`ci` (157) and `db` (77) alone are **43% of all CLI commands** and have zero GUI presence — they are the two surfaces most worth adding.

Beyond these, the **entire build/dev spine is ungoverned**: `build`, `check`, `compile`, `dev`, `run`, `test`, `fmt`, `emit`, `new`, `init`, `generate`, `component`, `bundle`, `snippet`. The Repository surface already shells `status`/`check`/`diff`, so wiring build/test/run there is low-cost.

## Governance recommendations

### 1. Fold into existing surfaces (cheap, no new nav)
- **Build spine** (`build` `check` `compile` `dev` `run` `test` `fmt` `fabrica` `emit` `new` `init` `generate` `component` `bundle*` `snippet` `share`) → **Develop > Workspace / Console** as actions. Repository already proves the `execute_command` pattern.
- **Dependency ops** (`add` `remove` `lock` `sync`) → **Develop > Repository** deps panel.
- **`plugin` + `mcp`** → **Develop > Skills** (Plugins / MCP tabs) — Skills already reaches `vox_skill_*` via MCP.
- **`plan` + `workflow` + `dispatch` + `stop`** → **Operate > Tasks / Agents**.
- **`speech`/`train`** → already under Oratio / Mens (gated).

### 2. New read-only panels (highest value, low risk)
- **`Develop > CI`** for `ci` (157) — gate status dashboard; run actions behind confirm. Biggest single win.
- **`Knowledge > Database`** for `db` (77) — read-only query/table browser; destructive admin (`db migrate`, drops) behind confirm.
- **`Knowledge > Graph`** for `graphify` (8) — graph viewer + build action.
- **`System > Settings`** sub-panels for `doctor` `diag` `drift-check` `audit` `telemetry` (read-only health/coverage) and `secrets` `auth`/`login`/`logout` `ext` (need typed wrappers, not raw shell-out, because they touch credentials).

### 3. Needs a typed wrapper, not exec (security)
`secrets`, `auth`, `login`, `logout`, `config` writes — never shell these through the generic `execute_command`; add `#[tauri::command]` wrappers with structured args so secrets never transit a shell string.

### 4. Keep CLI-only (intentional)
| Group | Why |
|---|---|
| `completions` | shell-setup, one-time |
| `lsp` | editor/LSP integration, not interactive |
| `grammar`, `wasm`, `play`, `ars` | dev/compiler-internal tooling |
| `repl`, `shell`, `term` | interactive terminals (GUI has PTY already; the CLI forms stay) |
| `visus` | CI visual-review tooling (advisory, non-interactive) |
| `migrate`, `rollback`, `snapshot`, `repair`, `upgrade`, `update` | dangerous/admin — surface *read-only status* at most; keep the mutating action CLI-gated or behind a strong confirm. |

## Tie-in to the ratified nav

The blueprint nav groups (`operate`, `develop`, `knowledge`, `compute`, `system`) already host every governed surface. The gap is filled with **at most 3 genuinely new surfaces** — `Develop > CI`, `Knowledge > Database`, `Knowledge > Graph` — plus tabs/sub-panels under the existing `Develop > Workspace`, `Develop > Skills`, and `System > Settings`. No new top-level nav group is required. Prioritize CI and Database: they convert 234 of the 389 ungoverned commands (60%) into reachable surfaces.

## Artifacts
- `graphify-out/gui-coverage/cli-governance.json` — per-group + per-command `{group, command, governance, surface_home, recommended_home}` (74 groups, 503 per-command rows).
- This report.

## Caveats
- Governance is assessed at **group granularity** then weighted by leaf count; a group marked `governed-*` may still have individual subcommands unreached (notably `mens`/`populi`/`oratio` cards cover ~3 of 9–22 subcommands each, and `scientia`'s 61 are partially wired). True per-leaf coverage inside governed groups is *lower* than 29.1% — treat 29.1% as an upper bound on real governance.
- **DONE (vs1):** the clap CLI tree is now ingested into the surface graph as `cli:<group>:<command>` nodes via `vox_graphify_reader::registry::cli_command_nodes` (joined to the existing `cmd:`/`tool:`/surface nodes), and unified coverage classifies surface-less CLI nodes as `CliOnly`. Run `vox search coverage --corpus vox-gui-surface --kind cli-command` for the live per-leaf not-in-GUI report. (Originally this was deferred: the graph held only `cmd:`/`tool:` nodes from the Rust/TSX walk, not the clap derive enums — this audit was the manual stand-in; that enumeration is now built into the graph walk.)
