# Crate API: vox-cli

## Overview

The command-line interface for the Vox programming language. Entry point for all `vox` commands.

## Commands

| Command | Description |
|---------|-------------|
| `vox build <file>` | Compile a `.vox` file to Rust + TypeScript |
| `vox run <file>` | Build and run a Vox application |
| `vox bundle <file>` | Bundle into a standalone web application |
| `vox test <file>` | Run `@test` decorated functions |
| `vox fmt <file>` | Format Vox source files |
| `vox check <file>` | Type-check without producing output |
| `vox compact <file>` | Compact source for LLM context |
| `vox lsp` | Launch the Language Server |
| `vox doc` | Generate documentation via vox-doc-pipeline |
| `vox init [name]` | Scaffold a new Vox project |
| `vox install <pkg>` | Install a package |
| `vox add/remove` | Add or remove dependencies |
| `vox update` | Update dependencies |
| `vox vendor` | Bundle dependencies for offline builds |
| `vox publish` | Publish a package to the registry |
| `vox search <query>` | Search the package registry |
| `vox audit` | Check dependencies for security advisories |
| `vox clean` | Remove build artifacts |
| `vox stub-check` | Run TOESTUB anti-pattern detection |
| `vox review` | AI-assisted code review |
| `vox share <action>` | Publish/browse shared artifacts |
| `vox snippet <action>` | Save/search code snippets |
| `vox agent <action>` | Register/manage AI agents |
| `vox gamify <action>` | Code companions, quests, battles |
| `vox orchestrator <action>` | Multi-agent task queues |
| `vox dashboard` | Orchestrator HUD web UI |
| `vox learn <action>` | Learning and skill progress |
| `vox agent <action>` | AI Agent integration (spawn, status, review) |

## Key Files

| File | Purpose |
|------|---------|
| `main.rs` | CLI argument parsing (clap) and command dispatch |
| `commands/` | One module per subcommand |
| `templates.rs` | Project scaffolding templates for `vox init` |
| `v0.rs` | v0.dev AI component generation integration |

## Usage

```bash
# Install from source
cargo install --path crates/vox-cli

# Or build for development
cargo build -p vox-cli
```

---

### `fn run`

`vox add <dep> [--version <ver>] [--path <path>]` — add a dependency to Vox.toml.


## Module: `vox-cli\src\commands\agent.rs`

`vox agent` — register, list, and inspect AI agent definitions.


### `fn create`

Register an agent definition.


### `fn list`

List all registered agents.


### `fn info`

Get details of a specific agent.


### `fn generate`

Dynamically generate native AI agents based on workspace crates.


### `fn run`

`vox audit` — audit dependencies for known issues.


### `fn run`

Bundle a Vox source file into a complete, runnable web application.

1. Runs the build pipeline (lex → parse → typecheck → codegen)
2. Scaffolds a Vite + React project around the generated TS components
3. Runs `npm install && npm run build` to produce static assets
4. Copies built assets into the Rust backend's public/ directory
5. Runs `cargo build --release` to produce a single binary


### `fn run`

`vox clean` — clean build artifacts and caches.


## Module: `vox-cli\src\commands\db.rs`

`vox db` subcommand — inspect and manage the local VoxDB database.


### `fn status`

Print current VoxDB schema version and connection path.


### `fn reset`

Reset the database by dropping all tables and re-applying migrations.


### `fn schema`

Print the current schema digest for LLM context.


### `fn sample`

Print sample data from a table or collection.


### `fn migrate`

Apply any pending schema migrations.


### `fn export`

Export memory, patterns, and preferences for a user to JSON.


### `fn import`

Import preferences and memory from a JSON file previously exported with `vox db export`.


### `fn vacuum`

Run SQLite VACUUM to reclaim space and defragment the database.


### `fn prune`

Delete memory entries older than `days` days for a given agent/user.


### `fn pref_get`

Get a user preference by key.


### `fn pref_set`

Set a user preference key/value.


### `fn pref_list`

List all preferences for a user.


### `fn run`

Stub for the `vox deploy` command.

In the future, this will read deploy configurations from `Vox.toml`
and handle orchestration to cloud providers or local runners.


## Module: `vox-cli\src\commands\dev.rs`

`vox dev` — build, run, and watch for changes in one command.

Equivalent to running `vox build` and `vox run` simultaneously, with
automatic rebuild and restart when source files change.


## Module: `vox-cli\src\commands\doctor.rs`

`vox doctor` — check the development environment is ready.


## Module: `vox-cli\src\commands\gamify.rs`

`vox gamify` subcommands — profile, companions, quests, battles.


### `fn record_activity`

Record a daily activity action and display a subtle message if a streak/level changes.


### `fn status`

Display gamification status (profile overview).


### `fn companion_list`

List all companions.


### `fn companion_create`

Create a new companion from a source file.


### `fn quest_list`

List daily quests.


### `fn battle_start`

Start a bug battle.


### `fn companion_interact`

Interact with a companion.


### `fn battle_submit`

Submit code to win a bug battle.


## Module: `vox-cli\src\commands\generate.rs`

`vox generate` — generate validated Vox code using the QWEN fine-tuned model.

Calls the inference server at localhost:7863 (started by `python scripts/vox_inference.py --serve`)
or starts it automatically if not running.

Usage:
vox generate "Create a counter actor with increment and decrement"
vox generate "Write a todo app" --output todo.vox
vox generate "Write unit tests for the factorial function" --no-validate


### `fn run`

Run the generate command.


### `fn run`

`vox info <package>` — display package information.


### `fn run`

`vox init` — scaffold a new Vox project with a `Vox.toml` manifest.


### `fn run`

`vox install [package_name]` — install dependencies from Vox.toml or a specific package.


### `fn run`

`vox login` — authenticate with the VoxPM registry.


## Module: `vox-cli\src\commands\agent.rs`

`vox agent` — interact with AI agents natively via CLI and SDK.

### `fn create`

Register an agent definition.

### `fn list`

List all registered agents.

### `fn info`

Get details of a specific agent.

### `fn generate`

Dynamically generate native AI agents based on workspace crates.


### `fn status`

`vox orchestrator status` — show all agents, queues, and file assignments.


### `fn submit`

`vox orchestrator submit` — manually submit a task.


### `fn queue`

`vox orchestrator queue` — show a specific agent's queue.


### `fn rebalance`

`vox orchestrator rebalance` — trigger manual rebalancing.


### `fn config`

`vox orchestrator config` — show current orchestrator configuration.


### `fn pause`

`vox orchestrator pause` — pause an agent.


### `fn resume`

`vox orchestrator resume` — resume an agent.


### `fn save`

`vox orchestrator save` — manually save orchestrator state.


### `fn load`

`vox orchestrator load` — manually load orchestrator state.


### `fn run`

`vox publish` — publish the current package to the VoxPM registry.


### `fn run`

`vox remove <dep>` — remove a dependency from Vox.toml.


## Module: `vox-cli\src\commands\review.rs`

`vox review` — AI-powered code review command.

Performs multi-layer code review:
1. Static analysis (TOESTUB detectors)
2. Context gathering (all files, or only git-diff changed files in --diff mode)
3. LLM review via provider cascade (OpenRouter → OpenAI → Gemini → Ollama → Pollinations)
4. Post-LLM verification and deduplication
5. Optional PR comment posting (GitHub REST API)


### `fn run`

Run the `vox review` command.


### `fn run`

`vox search <query>` — search the VoxPM registry for packages.


### `struct DashboardState`

Dashboard API — provides REST endpoints for the Vox Dashboard UI.
Each endpoint reads from the CodeStore so the dashboard shows real data
instead of hardcoded mocks.

Endpoints:
GET  /api/workflows       → list workflow definitions
GET  /api/skills          → list skill-type artifacts
GET  /api/agents          → list agent definitions
GET  /api/snippets        → list/search code snippets
GET  /api/marketplace     → list published shared artifacts
GET  /api/feedback        → list LLM interactions + feedback
POST /api/agents          → create/update agent definition
POST /api/snippets        → save a code snippet
POST /api/feedback        → submit feedback for an interaction
Shared state for the dashboard API.


### `fn list_workflows`

GET /api/workflows


### `fn list_skills`

GET /api/skills


### `fn list_agents`

GET /api/agents


### `fn list_snippets`

GET /api/snippets?q=&lt;query&gt;


### `fn list_marketplace`

GET /api/marketplace?q=&lt;query&gt;


### `fn list_feedback`

GET /api/feedback


### `fn create_agent`

POST /api/agents


### `fn save_snippet`

POST /api/snippets


### `fn submit_feedback`

POST /api/feedback


## Module: `vox-cli\src\commands\serve_dashboard\mod.rs`

Vox Dashboard server module — serves the built-in dashboard UI.


## Module: `vox-cli\src\commands\share.rs`

`vox share` — share artifacts (workflows, skills, code) via the Vox marketplace.


### `fn publish`

Run the `vox share publish` subcommand.


### `fn search`

Run the `vox share search` subcommand.


### `fn list`

Run the `vox share list` subcommand.


### `fn review`

Run the `vox share review` subcommand.


## Module: `vox-cli\src\commands\skill.rs`

`vox skill` — manage Vox skills from the CLI.


### `fn discover`

9.4 — Skill auto-discovery: scan the crate graph for `.skill.md` files
and suggest installable skills not yet in the registry.


## Module: `vox-cli\src\commands\snippet.rs`

`vox snippet` — save, search, and manage code snippets.


### `fn save`

Save a code snippet from a file.


### `fn search`

Search code snippets.


### `fn export`

Export snippets as JSON (for RLHF/RAG pipelines).


### `fn run`

Run the TOESTUB analysis.


## Module: `vox-cli\src\commands\train.rs`

Fine-tune orchestration: same dataset artifact as `vox learn --export-dataset`.
Local provider runs uv-driven vox-train; remote supports Together AI (env TOGETHER_API_KEY).


### `fn run`

Run fine-tune orchestration (local or remote via Together AI).


### `fn run`

`vox tree` — display the dependency tree for the current project.


### `fn run`

`vox update` — update dependencies to latest compatible versions.


## Module: `vox-cli\src\commands\workflow.rs`

`vox workflow` — inspect and validate Vox workflows and activities.


### `fn list`

List all workflows and activities defined in a .vox file.


### `fn inspect`

Show type-checked info about a specific workflow.


### `fn check`

Type-check a workflow file through the full Vox compiler pipeline.


### `fn run`

Execute a workflow (currently a stub before durable execution runtime is implemented).


## Module: `vox-cli\src\main.rs`

# vox-cli

Command-line interface for the Vox AI-native programming language.

Provides `vox build`, `vox test`, `vox fmt`, `vox lsp`, `vox install`,
and many more subcommands for the full development lifecycle.


## Module: `vox-cli\src\templates.rs`

Embedded templates for scaffolding a complete web application.
These are baked into the compiler binary so no external files are needed.


### `fn index_html`

Returns the default `index.html` template for Vox web applications.


### `fn main_tsx`

Returns the default `main.tsx` template, wiring up React and the specified root component.


### `fn index_css`

Returns the default `index.css` global stylesheet.


### `fn package_json`

Returns the default `package.json` for frontend dependencies.


### `fn vite_config`

Returns the `vite.config.ts` template configured to proxy API requests to the Rust backend.


### `fn tsconfig_json`

Returns the default `tsconfig.json` for the frontend build.


### `fn generate_component`

Generate a UI component using v0.dev based on a prompt.

This function calls the v0 Platform API to generate React code.
It expects the `V0_API_KEY` environment variable to be set.
