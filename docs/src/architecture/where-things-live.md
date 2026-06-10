---
title: "Where Things Live"
description: "Flat lookup table — concept to crate. Consult before adding code. Referenced by AGENTS.md and CLAUDE.md."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Canonical concept-to-crate lookup; high-value for LLM navigation of the Vox workspace."
---

# Where Things Live

Flat lookup table for "I need to add/find X — where does it go?". Optimized
for LLM tool-call efficiency: skim by left column, jump to the right column.

If your concept doesn't appear here, **add the row in the same PR** — that
keeps the table accurate and prevents the next assistant from having to
guess. The full crate roster with layer assignments lives in
[`layers.toml`](./layers.toml).

For **directory sprawl, sparse folders, and non-Rust artifact provenance**, see [Repository layout sprawl audit (2026)](./repo-layout-sprawl-audit-2026.md).

## Repository roots (navigation)

Grouped map of **top-level trees** — use this before inventing a new parallel folder (for example another `docker/` or `tools/`).

| Group | Paths | Notes |
|-------|-------|-------|
| Rust workspace | [`Cargo.toml`](../../../Cargo.toml), [`crates/`](../../../crates/) | Implementation + `vox-arch-check` / [`layers.toml`](./layers.toml). `vox-arch-check` does not recurse into `target/`, `.git/`, `node_modules/`, etc. when scanning (see `[arch_check.walk_prune]`). Reclaim disk from stale artifacts: `cargo clean` or [`scripts/clean-build-artifacts.vox`](../../../scripts/clean-build-artifacts.vox) (syntax-checked in CI via `cargo run -p vox-cli -- check scripts/clean-build-artifacts.vox`). For crates under `workspace.exclude` in root `Cargo.toml`, set `CARGO_TARGET_DIR` to the repo `target/` when running `cargo --manifest-path …` so Cargo does not grow a second `crates/.../target/`. |
| Contracts | [`contracts/`](../../../contracts/), [`contracts/index.yaml`](../../../contracts/index.yaml) | SSOT bundles; path changes require index + consumer updates. |
| Unified policy catalog | [`contracts/policy/policy-registry.v1.yaml`](../../../contracts/policy/policy-registry.v1.yaml) | SSOT for the cross-domain policy registry. Model + loader live in [`vox-config`](../../../crates/vox-config/) (`policy::registry`); the generator, drift gate (`vox ci policy-registry-parity`), and `vox policy` CLI live in [`vox-cli`](../../../crates/vox-cli/) (`commands/ci/policy_registry.rs`, `commands/policy/`, feature-gated behind `completion-toestub`). |
| Per-branch policy run status (overlay) | `.vox/policy-status/<branch>.json` store (gitignored) | Model + reader in [`vox-config`](../../../crates/vox-config/) (`policy::status`: `PolicyRunReport`, `load_status`, `load_status_for_branches`); writer + capture seams in [`vox-cli`](../../../crates/vox-cli/) (`commands/policy/status_writer.rs`, per-gate dispatch wrapper in `commands/ci/run_body.rs`, code-audit emit in `commands/diagnostics/stub_check/`); `--json` per-rule emitter in [`vox-arch-check`](../../../crates/vox-arch-check/) (`json_report.rs`). Read via `vox policy status [--branch b]…`; unrun rules show `unknown` (honest grey). |
| Docs (source) | [`docs/src/`](../../) under repo `docs/` | Narrative SSOT; `.md` doctests. |
| Docs (site) | [`docs-astro/`](../../../docs-astro/) | Astro build; sidebar from frontmatter. |
| Apps & editors | [`apps/`](../../../apps/) | GUI surfaces — registry in [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml). |
| Examples / fixtures | [`examples/`](../../../examples/), [`tests/fixtures/`](../../../tests/fixtures/) | Non-production sandboxes and bundles. Goldens under [`examples/golden/`](../../../examples/golden/) include [`clean_build_stdlib_reference.vox`](../../../examples/golden/clean_build_stdlib_reference.vox) (pointer to [`scripts/clean-build-artifacts.vox`](../../../scripts/clean-build-artifacts.vox)). |
| Automation | [`scripts/*.vox`](../../../scripts/) | Prefer `vox run` over new shell/Python glue. |
| CI / hooks | [`.github/workflows/`](../../../.github/workflows/), [`lefthook.yml`](../../../lefthook.yml) | Runner wiring; see CI docs under `docs/src/ci/`. |
| Deploy / compose | [`infra/`](../../../infra/), [`docker/`](../../../docker/), root [`docker-compose.yml`](../../../docker-compose.yml) | Overlap is intentional (different compose working dirs); eval SSOT called out in deploy docs. |
| Build-time Node helper | [`apps/build-tools/render-durable-animation/`](../../../apps/build-tools/render-durable-animation/) | Invoked from [`scripts/render-durable-animation.vox`](../../../scripts/render-durable-animation.vox). |

## Quick reference: subsystem → crate (by layer)

### L0 — pure types

| Crate | One-line scope |
|---|---|
| [`vox-arch-check`](../../../crates/vox-arch-check/) | CI guard binary; enforces layers.toml. Walk-based rules skip artifact trees (built-in + optional `[arch_check.walk_prune]`); Rule 8 staleness uses one batched `git log` when available. |
| [`vox-ast`](../../../crates/vox-ast/) | Pure-data Vox AST (decl/expr/stmt/pattern/types/scalar_mapping/span); serde-only L0 leaf extracted from `vox-compiler` and re-exported there as `vox_compiler::ast`. Depend on this (not `vox-compiler`) if you only need the declaration AST — e.g. `vox-db`'s DDL emitter. |
| [`vox-build-meta`](../../../crates/vox-build-meta/) | Build-time helper emitting `VOX_BUILD_NUMBER` / `VOX_GIT_HASH`; use as `[build-dependencies]` only. |
| [`vox-db-types`](../../../crates/vox-db-types/) | Pure-data L0 leaf for vox-db: row types, IDs, schema descriptors. |
| [`vox-mesh-types`](../../../crates/vox-mesh-types/) | Pure-data mesh transport types. |
| [`vox-orchestrator-types`](../../../crates/vox-orchestrator-types/) | Pure-data L0 leaf for vox-orchestrator: agent/task IDs, file affinity, switch actions, provider catalogs, VCS capability tokens (WorkingTreeWrite, BranchCreate, etc.). |
| [`workspace-hack`](../../../crates/workspace-hack/) | Cargo-hakari unification crate; do not edit by hand. |

### L1 — primitives & utilities

| Crate | One-line scope |
|---|---|
| [`vox-foundation`](../../../crates/vox-foundation/) | Utility umbrella: cheap trace ids, exponential backoff, AgentOS mutation kinds (was `vox-primitives`), daemon wire-protocol types (was `vox-protocol`), `tracing_subscriber` bootstrap presets (was `vox-tracing-init`). Excluded from workspace-hack for fast-leaf compile. |
| [`vox-bounded-fs`](../../../crates/vox-bounded-fs/) | UTF-8 file reads capped by vox-scaling-policy max_file_bytes_hint. |
| [`vox-tauri-codegen`](../../../crates/vox-tauri-codegen/) | Hint-only Tauri 2 packaging under `target/generated/tauri-packaging/` (`tauri.conf.json` + `runtime-capabilities.projection.json` from [`contracts/capability/runtime-capabilities.v1.yaml`](../../../contracts/capability/runtime-capabilities.v1.yaml); not a full `src-tauri` crate yet). |
| [`vox-crypto`](../../../crates/vox-crypto/) | Pure-Rust crypto primitives (chacha20poly1305 AEAD, ed25519, x25519); sole crypto SSOT per AGENTS.md §Cryptography Policy. |
| [`vox-grammar-export`](../../../crates/vox-grammar-export/) | Exports the Vox grammar artifact for downstream tooling. |
| [`vox-hf-layout`](../../../crates/vox-hf-layout/) | SSOT: Hugging Face `config.json` layout parsing for MENS (`vox-populi::mens::tensor::hf_load`) and `vox-plugin-mens-candle-cuda`. |
| [`vox-identity`](../../../crates/vox-identity/) | Identity primitives: signing keys, trust ledger entries. |
| [`vox-jsonschema-util`](../../../crates/vox-jsonschema-util/) | Shared JSON Schema compile + validate helpers for CLI, contracts, and tooling. |
| [`vox-openai`](../../../crates/vox-openai/) | OpenAI integrations: wire-format types (`chat_completion.rs`) + SSE streaming (`sse.rs`) in one L1 crate. |
| [`vox-package-types`](../../../crates/vox-package-types/) | Pure-data L1 leaf for vox-package: manifest, lockfile, package_kind, resolver types. |
| [`vox-plugin-api`](../../../crates/vox-plugin-api/) | Shared API surface for Vox plugins: ABI version, traits, manifest types, error types. |
| [`vox-plugin-sdk`](../../../crates/vox-plugin-sdk/) | Authoring SDK for code plugins: re-exports the stable ABI + the `declare_plugin!` glue macro (ABI-neutral, byte-identical exports). |
| [`vox-plugin-types`](../../../crates/vox-plugin-types/) | Pure-types surface for the vox plugin system: manifests, skill types, state-backend trait. |
| [`vox-telemetry`](../../../crates/vox-telemetry/) | L1 telemetry facade: `METRIC_TYPE_*` constants, `TelemetryRecorder` trait, `record_event!` macro, `TelemetryConfig` (Phase D: org-policy hard-off + `VOX_TELEMETRY=on/off/debug`), per-task `TaskAggregate`, `record_task_started`. Zero domain dependencies. |
| [`vox-runtime`](../../../crates/vox-runtime/) | Umbrella runtime foundation: `RuntimeProfile` (Desktop vs Mobile), lifecycle traits, `VoxConfig`. Consumed by downstream runtime crates and the uniffi mobile bridge. Zero internal deps. |
| [`vox-http-client`](../../../crates/vox-http-client/) | Shared HTTP client presets (user-agent, timeouts) for CLI, runtime, and AI transports. |
| [`vox-rename-registry`](../../../crates/vox-rename-registry/) | Rename registry (`RenameKind`, `RenameRegistry`, `RegistryError`) and primitive-tag lookup (`primitive_tags::all_primitives`, `is_primitive`). L0 — zero workspace deps. Re-exported via `vox_compiler::parser::renames` and `vox_compiler::lowering_shared::primitive_tags`. |
| [`vox-shell-stdlib-types`](../../../crates/vox-shell-stdlib-types/) | Shared data types for the Vox shell stdlib surface (`std.fs.*`). `VoxFileRecord` — the canonical file-metadata type used by both the compiler interpreter path and `vox-actor-runtime` codegen path. L0 — only `serde` dep. |
| [`vox-research-events`](../../../crates/vox-research-events/) | Typed SCIENTIA research event types and `PreregistrationV1`. |
| [`vox-rule-pack`](../../../crates/vox-rule-pack/) | Declarative YAML rule-pack loader for code-audit detector patterns and Scientia heuristics. Zero heavy deps. |
| [`vox-scaling-policy`](../../../crates/vox-scaling-policy/) | Compile-time and runtime accessors for scaling SSOT (contracts/scaling/policy.yaml). |
| [`vox-secrets`](../../../crates/vox-secrets/) | Central secret resolution and compatibility adapters for Vox. |

### L2 — domain libraries

| Crate | One-line scope |
|---|---|
| [`vox-capability-registry`](../../../crates/vox-capability-registry/) | Transport-independent capability registry (YAML SSOT) + Mens chat tool descriptors. |
| [`vox-config`](../../../crates/vox-config/) | Centralized configuration and env/default resolution for Vox tooling; `project_manifest` parses `[workspace]` / `[bundle]` slices from `Vox.toml` for `vox compile`. |
| [`vox-constrained-gen`](../../../crates/vox-constrained-gen/) | Grammar-constrained inference engine — Earley/PDA backends, deadlock watchdog, stream-of-revision. |
| [`vox-doc-inventory`](../../../crates/vox-doc-inventory/) | Generate and verify docs/agents/doc-inventory.json (schema v3) without Python. |
| [`vox-journal`](../../../crates/vox-journal/) | Generic append-only JSON Lines file journal; crash-safe via per-record sync_data, replays on open. Durable substrate for workflow/actor runtimes and the mobile vox-runtime-rn (deps vox-runtime). |
| [`vox-effort-audit`](../../../crates/vox-effort-audit/) | AI-judged audit of git commit history; walks commits, calls model-agnostic judge facade, emits ranked findings JSONL + report. CLI: `vox audit effort`. |
| [`vox-effort-route`](../../../crates/vox-effort-route/) | Routes effort-audit findings to verified, drafted enforcement artifacts (AGENTS.md rule / lint detector spec / arch rule / CI gate / corpus example / Vox script). CLI: `vox audit effort-route`. |
| [`vox-eval`](../../../crates/vox-eval/) | Evaluation **metrics** — deterministic scoring of model outputs / Vox samples (format/safety/quality/parse) + MENS `CompileVerdict`. **Not** the interpreter; `--interp` lives in [`vox-compiler/src/eval/`](../../../crates/vox-compiler/src/eval/). |
| [`vox-nanopub`](../../../crates/vox-nanopub/) | SCIENTIA nanopublication leaf: TriG emission, Ed25519 signing, Trusty-URI derivation. Depends only on `vox-crypto` + `sha2` + `hex` — consumable independently of `vox-scientia` (which re-exports it via `nanopub::{trig,signing}` and adds the network layer). |
| [`vox-mcp-registry`](../../../crates/vox-mcp-registry/) | Compile-time MCP tool name/description registry from contracts YAML (SSOT). |
| [`vox-project-scaffold`](../../../crates/vox-project-scaffold/) | Shared Vox.toml + src/main.vox + skill scaffolding for vox init and MCP. |
| [`vox-repository`](../../../crates/vox-repository/) | Repository discovery, stable identity, layout probes, and agent scope helpers for external and internal Vox workspaces. |
| [`vox-skill-runtime`](../../../crates/vox-skill-runtime/) | Abstract sandbox runtime trait for skill execution. Implementations ship as plugins (wasm, container). |

### L3 — heavy domain crates

| Crate | One-line scope |
|---|---|
| [`vox-actor-runtime`](../../../crates/vox-actor-runtime/) | Process-oriented runtime: actors, mailboxes, supervision, scheduling, LLM/Mens activity primitives. |
| [`vox-cli-core`](../../../crates/vox-cli-core/) | Shared internals for the vox CLI binary (argv parsing helpers, exit-code policy). |
| [`vox-code-audit`](../../../crates/vox-code-audit/) | AI code quality stub detector — finds stubs, magic values, empty bodies, missing references, and DRY violations. |
| [`vox-drift-check`](../../../crates/vox-drift-check/) | Workspace drift and pattern-repetition linter (multi-language: Rust, TypeScript, Vox). |
| [`vox-codegen`](../../../crates/vox-codegen/) | Codegen + WebIR + vox_ir extracted from vox-compiler. Consumes analysis types from vox-compiler. |
| [`vox-codegen/src/projection_bundle.rs`](../../../crates/vox-codegen/src/projection_bundle.rs) | **`project_bundle_from_hir`** — SSOT assembly of WebIR, AppContract, RuntimeProjection, ShellProjection, and RequiredRuntimeCapabilities for emitters. |
| [`vox-compiler/src/shell_projection.rs`](../../../crates/vox-compiler/src/shell_projection.rs) | Typed shell/mobile primitive projection from HIR (`@back_button`, `@deep_link`, `@push`). |
| [`vox-compiler/src/required_capabilities.rs`](../../../crates/vox-compiler/src/required_capabilities.rs) | HIR-derived sorted capability id list for packaging / filtered Tauri `runtime-capabilities.projection.json`. |
| [`vox-scientia-jsonschema-codegen`](../../../crates/vox-scientia-jsonschema-codegen/) | Offline `cargo run` tool: `contracts/scientia/*.schema.json` → `vox-research-events` typify bundle (`schema_types.generated.rs`). |
| [`vox-compiler`](../../../crates/vox-compiler/) | Unified Vox compiler: lexer, parser, AST, HIR, typechecker, and codegen. MENS decorators `@inference`, `@training_step`, `@distributed_train` parse in `parser/descent`, effects + CUDA gate in `typeck/`. Orientation: [`vox-compiler-architecture-research-2026.md`](./vox-compiler-architecture-research-2026.md). |
| [`vox-compiler/src/eval/shell_stdlib.rs`](../../../crates/vox-compiler/src/eval/shell_stdlib.rs) | Interpreter (`--interp`) mirror of shell‑tier `std.*` builtins — **must stay aligned** with `vox-actor-runtime` (Cargo cycle prevents a direct dep; shared types extracted to `vox-shell-stdlib-types`). See [`vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md). |
| [`vox-actor-runtime/src/builtins/mod.rs`](../../../crates/vox-actor-runtime/src/builtins/mod.rs) | SSOT Rust lowering targets for `std.fs` / `std.process` / structured formats (`std.csv`, `std.toml`, `std.yaml`, `std.io`) used by native codegen. |
| Static web scraping (`Scrape.*` builtins) | `vox_scrape_*` in [`vox-actor-runtime/src/builtins/mod.rs`](../../../crates/vox-actor-runtime/src/builtins/mod.rs) (`fetch`/`fetch_html`/`select`/`select_attr` — reqwest + `scraper` CSS, no browser); registered in [`builtin_registry.rs`](../../../crates/vox-compiler/src/builtin_registry.rs). Native-codegen only (no interp/WASI); effect `Net`. For JS-rendered pages use `Browser.*` ([`vox-plugin-browser`](../../../crates/vox-plugin-browser/), chromiumoxide CDP). See [scoping doc](./vox-native-scraping-scoping-2026-06-03.md). |
| [`vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md) | Architecture SSOT: argv‑first shell‑tier stdlib vs host shells / `vox_run_shell`. |
| [`vox-container-types`](../../../crates/vox-container-types/) | Pure OCI types: `ContainerRuntime` trait, `BuildOpts`/`RunOpts`, `RuntimePreference`, `exec_grammar` parser. L0; no I/O. |
| [`vox-container`](../../../crates/vox-container/) | OCI container runtime backends (Docker + Podman CLI) and `detect_runtime`. L3; re-exports all types from `vox-container-types`. |
| [`vox-corpus`](../../../crates/vox-corpus/) | Training data contracts, preflight, corpus SSOT, and Mens dataset metadata. |
| [`vox-db`](../../../crates/vox-db/) | Codex / VoxDb facade: schema migrations, store ops, Turso/libSQL access for the Vox workspace. |
| [`vox-sql`](../../../crates/vox-sql/) | Engine-agnostic SQL surface for app data-plane backends: `SqlDialect`/`SqlBackend`, `AnySqlBackend::connect_from_app_env` (`VOX_APP_DB_URL` -> `VOX_DB_URL` fallback), `migrate::AppAutoMigrator`, and normalized `SqlValue`/row contracts (with P5 DDL + migrate smoke tests under `crates/vox-sql/tests/`). |
| [`vox-deploy-codegen`](../../../crates/vox-deploy-codegen/) | Deployment artifact codegen: Dockerfile, Compose, K8s, Fly, Coolify, systemd. Pure text generation. |
| [`vox-doc-pipeline`](../../../crates/vox-doc-pipeline/) | Docs lint + doctest helpers for `docs/src/`; Starlight sidebar and RSS are built at **docs-astro** publish time (see root `AGENTS.md`). |
| [`vox-package`](../../../crates/vox-package/) | Vox package manager runtime: content-addressed artifact cache, registry HTTP client, workspace discovery. |
| [`vox-quantize`](../../../crates/vox-quantize/) | Data-free k-quant PTQ engine (SafeTensors → Candle GGML quantized SafeTensors; GPU-first, CPU fallback). |
| [`vox-inference`](../../../crates/vox-inference/) | MENS Mn-T2: `InferenceBackend` trait + multi-backend dispatcher (Candle CPU/CUDA/Metal, llama.cpp RPC, Ollama). Backends implemented; no in-tree consumers yet. |
| [`vox-distributed-training`](../../../crates/vox-distributed-training/) | MENS Mn-T1/Mn-T6: `TrainingSession`, signed `GradientShard` / `CheckpointBundle`, `OperationKind::TrainingCheckpoint` mapping. |
| [`vox-ml-cli`](../../../crates/vox-ml-cli/) | ML / Oratio / Populi / telemetry CLI binary (`vox-ml-cli`); Mens training, GPU features, optional workflow glue. |
| [`vox-forge`](../../../crates/vox-forge/) | Platform-agnostic Git forge API — GitHub, Gitea, Forgejo (GitLab deprecated/unsupported as of 2026-06-03). |
| [`vox-gamify`](../../../crates/vox-gamify/) | Gamification layer — companions, quests, battles, and free AI integration. |
| [`vox-git`](../../../crates/vox-git/) | Pure-Rust Git bridge using gix (no C, no libgit2). |
| [`vox-vcs`](../../../crates/vox-vcs/) | VCS backend abstraction: `VcsBackend` trait + in-memory `CasFallback`; the single home for all `jj_lib::` calls (the jj-lib 0.42 `JjBackend` lands in a later phase). Injected as a trait object into `vox-compiler` to avoid L3 coupling. |
| [`vox-lsp`](../../../crates/vox-lsp/) | Vox Language Server (stdio JSON-RPC). Capability matrix: [`vox-lsp-capabilities-ssot-2026.md`](./vox-lsp-capabilities-ssot-2026.md). |
| [`vox-openclaw-runtime`](../../../crates/vox-openclaw-runtime/) | OpenClaw client + ARS runtime adapter, executor, context bundles, hooks. |
| [`vox-research-shim`](../../../crates/vox-research-shim/) | DEI research pipeline and model-selection sub-systems (A-12 wedge). SCIENTIA orchestrator, claim/verify/persist, BroadcastEmitter, ScientiaMeshSubscriber, and `selection::` (FreeTierRouter, ModelScorer, task_routing). Uses `vox_orchestrator::types::RoutingProfile`. |
| [`vox-orchestrator`](../../../crates/vox-orchestrator/) | Multi-agent file-affinity router. `types::RoutingProfile` (7 routing intent variants), `ModelTier` (Unknown→Local→Free→Fast→Light→Pro→Elite, generated from model-routing.v1.yaml), `CostPreference::Economy` (default — free-by-default policy). Extraction plan: see [2026-05-15-orchestrator-tier-d-plan.md](./2026-05-15-orchestrator-tier-d-plan.md). |
| [`vox-orchestrator-mcp`](../../../crates/vox-orchestrator-mcp/) | MCP (Model Context Protocol) tool layer for vox-orchestrator. Extracted in 2026-05-08 reorg Phase 4. |
| [`vox-orchestrator-queue`](../../../crates/vox-orchestrator-queue/) | Locks, oplog, and affinity tracking for vox-orchestrator. Extracted in 2026-05-08 reorg Phase 5. |
| [`vox-orchestrator-test-helpers`](../../../crates/vox-orchestrator-test-helpers/) | Test-only fixtures and mocks for vox-orchestrator: MockBulletinBoard, load_golden_fixture. |
| [`vox-speech`](../../../crates/vox-speech/) | Speech-to-text (Oratio) — Candle Whisper (Rust) STT and transcript refinement. |
| [`vox-plugin-catalog`](../../../crates/vox-plugin-catalog/) | SSOT catalog of all first-party Vox plugins and distribution bundles. |
| [`vox-plugin-host`](../../../crates/vox-plugin-host/) | Host-side plugin discovery, loading, and registry. |
| [`vox-plugin-test-harness`](../../../crates/vox-plugin-test-harness/) | Shared test utilities for plugin authors: fluent `Plugin.toml` manifest builders (`CodeManifestBuilder`, `SkillManifestBuilder`) and `PluginDir` temp-directory helper. |
| [`vox-populi`](../../../crates/vox-populi/) | Vox Populi: multi-node worker registry, HTTP control plane, and Mens native ML (Burn / Candle QLoRA). |
| [`vox-populi-types`](../../../crates/vox-populi-types/) | Pure-data types for the Populi mesh layer: `NodeRecord`, `PopuliRegistryFile`, `PopuliRegistryError`, and stateless helpers. L2 — no async runtime, no database. ([ADR-042](adr-042-vox-populi-types.md)) |
| [`vox-publisher`](../../../crates/vox-publisher/) | Unified news syndication and publishing for Vox. SCIENTIA mesh intake from orchestrator research events: `research_mesh.rs` (+ contract `contracts/scientia/research-mesh-intake.v1.schema.json`). |
| [`vox-scientia`](../../../crates/vox-scientia/) | SCIENTIA cluster umbrella — all Phases A–H as sub-modules: `producers` (Phase A signal emitters), `replay` (Phase B re-executor), `manuscript` (Phases C+3+4 IMRaD/LaTeX scaffolders), `critic_gate` (Phase D gate), `class_routing` (Phase E venue routing), `findings_site` (Phase G HTML builder), `dashboard` (Phase H JSON builders). |
| [`vox-search`](../../../crates/vox-search/) | Local-first retrieval execution: memory hybrid, repo inventory, Codex chunks, policy, and optional lexical/vector backends. |
| [`vox-skills`](../../../crates/vox-skills/) | Skill marketplace and plugin architecture for the Vox agent system. |
| [`vox-tensor`](../../../crates/vox-tensor/) | Pure-CPU JSONL data loaders / training-pair types (Burn extracted 2026-05-08). |
| [`vox-test-harness`](../../../crates/vox-test-harness/) | Shared compiler/tooling test fixtures plus [`workspace_paths`](../../../crates/vox-test-harness/src/workspace_paths.rs) (`repo_root_for_tests`), [`env_scratch`](../../../crates/vox-test-harness/src/env_scratch.rs) (scoped `set_var`/`remove_var`), [`temp_root`](../../../crates/vox-test-harness/src/temp_root.rs) (`tempfile::TempDir`). |
| [`vox-wasm-engine`](../../../crates/vox-wasm-engine/) | Single-source-of-truth Wasmtime engine + WASI execution for Vox programs and skill plugins. |
| [`vox-wire-format-validator`](../../../crates/vox-wire-format-validator/) | CI guard: enforces Wire Format v1 SSOT and Contract IR implementation parity. |
| [`vox-workflow-runtime`](../../../crates/vox-workflow-runtime/) | Interpreted workflow execution MVP (local + mens activity hooks). |
| [`vox-runtime-rn`](../../../crates/vox-runtime-rn/) | uniffi-bridged Rust runtime for Vox mobile (React Native + Expo); cross-compiles to iOS/Android, generates TS TurboModule bindings. Bridges vox-runtime + vox-journal to JS; consumed out-of-tree by clients/runtime-rn. |
| [`vox-mesh-policy`](../../../crates/vox-mesh-policy/) | Parses and pretty-prints `donations.vox` mesh policy files; defines `WorkerDonationPolicy`. |
| [`vox-tauri-stt`](../../../crates/vox-tauri-stt/) | Tauri 2 on-device speech-to-text plugin — wire format, guest JS facade, Android/iOS native sources. |

### L5 — surfaces

| Crate | One-line scope |
|---|---|
| [`vox-cli`](../../../crates/vox-cli/) | Vox command-line interface: compile, run, bundle, and workspace diagnostics. |
| [`vox-cli-tests`](../../../crates/vox-cli-tests/) | End-to-end `vox build`/`vox new` CLI integration harness (test-only L5; assert_cmd subprocess + tsc/cargo check). |
| [`vox-integration-tests`](../../../crates/vox-integration-tests/) | Cross-crate integration test harness (test-only L5). |
| [`vox-orchestrator-d`](../../../crates/vox-orchestrator-d/) | Vox orchestrator daemon binary. Extracted from vox-orchestrator in 2026-05-08 reorg Phase 4. |
| [`vox-gui`](../../../crates/vox-gui/) | Tauri desktop shell; depends on vox-cli / vox-orchestrator. |
| [`vox-audit`](../../../crates/vox-audit/) | CR-L gate runner binary (`vox audit <thing>`); implements the JSON report shape from `contracts/ci/vox-audit-contract.v1.yaml`. |
| [`voxup`](../../../crates/voxup/) | Desktop & CLI environment installer. |

## Common tasks → exact path

| I want to... | The right place |
|---|---|
| Add an MCP tool | `crates/vox-orchestrator-mcp/src/<group>_tools.rs` (e.g. `git_tools.rs`); register dispatch in [`mcp/dispatch.rs`](../../../crates/vox-orchestrator-mcp/src/dispatch.rs) |
| Add an HTTP route (orchestrator) | `crates/vox-orchestrator-mcp/src/services/routes/` |
| Add a CLI subcommand | `crates/vox-cli/src/commands/<group>.rs` + register in [`commands/mod.rs`](../../../crates/vox-cli/src/commands/mod.rs) |
| Add a CI subcommand under `vox ci` | `crates/vox-cli/src/commands/ci/` |
| Add a new CI/db guard | `crates/vox-cli/src/commands/ci/<name>.rs` + register in `cmd_enums.rs` and `run_body.rs`. Mirror `db_schema_coverage.rs`. |
| Add a speech-to-code audit artifact | Contracts under `contracts/speech-to-code/`; narrative findings under `docs/src/architecture/vox-speech-*-2026.md`; cross-crate tests under `crates/vox-integration-tests/tests/speech_*`. |
| AI-first language fixtures (taxonomy + seed catalog + lifecycle) | [`contracts/agentos/ai-first-fixtures.v1.yaml`](../../../contracts/agentos/ai-first-fixtures.v1.yaml) (JSON Schema: `contracts/agentos/ai-first-fixtures.v1.schema.json`) + narrative SSOT [`ai-first-fixtures-research-2026.md`](./ai-first-fixtures-research-2026.md). |
| Local `act` configuration (catalog image pin, platform map) | `.actrc` (repo root) |
| Self-hosted CI runner image | `Dockerfile.ci-runner` (repo root); published via `.github/workflows/publish-ci-runner.yml` to GHCR |
| Extend `vox ci pre-push` modes / timing JSON | `crates/vox-cli/src/commands/ci/pre_push.rs` — add `Step` to `build_steps` or extend `PrePushOpts` |
| `vox ci dev-loop-audit` (AI/local compile-loop diagnostics) | `crates/vox-cli/src/commands/ci/dev_loop_audit.rs` |
| `vox ci docs-reality-audit` (doc/code audit artifacts + metrics) | `crates/vox-cli/src/commands/ci/docs_reality_audit.rs` + `contracts/reports/docs-reality-audit/` |
| `vox ci parse-status` (golden parse matrix → `examples/PARSE_STATUS.md`) | `crates/vox-cli/src/commands/ci/parse_status.rs` |
| Find the canonical path for GUI surfaces (interop app, experimental visualizer, fixtures, VS Code host) | [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml) — `apps/interop/marquee_app`, `apps/experimental/visualizer`, `tests/fixtures/frontend/test_app_bundle`, `apps/editor/vox-vscode` |
| Add a `Db<Entity>Id` newtype | `crates/vox-db-types/src/ids.rs` (use the `string_id!` macro). |
| Add a DB store operation | `crates/vox-db/src/<concept>.rs` (impl block on `VoxDb`) |
| Add a pure-data DB row type | `crates/vox-db-types/src/store_types/` (NOT `vox-db`) |
| Add a pure-data DB type | `crates/vox-db-types/src/` |
| Add an orchestrator type (Agent/Task/etc.) | `crates/vox-orchestrator-types/src/agent_types/` |
| Add an orchestrator policy module (D1–D10) | `crates/vox-orchestrator/src/<module>.rs` + register in `lib.rs` + add row to this table |
| Add a research-pipeline stage (claims/gate/planner/provider/types/verifier) | `crates/vox-orchestrator/src/dei_shim/research/<module>.rs`. **`web_gather`** delegates to `vox-search` (`WebSearchDispatcher`, `CragRouter`); planner/claims/verifier remain Phase 0a stubs until `vox-claim-extractor`. CLI: `vox research run`; MCP: `vox_research_run`. **Telemetry bridge / policy feedback / mesh tap:** `research_event_metrics_bridge.rs`, `search_policy_feedback.rs`, `mesh_subscriber.rs`; narrative [`research-scientia-telemetry-channels.md`](./research-scientia-telemetry-channels.md). |
| Orchestrator policy façade (all D1–D10) | `crates/vox-orchestrator/src/orchestrator_policy.rs` |
| Circuit breaker — doom-loop detection (D6) | `crates/vox-orchestrator/src/circuit_breaker.rs` |
| Confidence fusion — Socrates trigger (D3) | `crates/vox-orchestrator/src/confidence_fusion.rs` |
| Tier cascade — model routing (D1) | `crates/vox-orchestrator/src/tier_cascade.rs` |
| Registry-backed model resolution (Thompson arms, privacy-local filter) | `crates/vox-orchestrator/src/registry_model_resolve.rs` |
| Inference tier/modality config for routing (`InferenceConfig`, `TierProfile`) | `crates/vox-orchestrator/src/mode.rs` |
| Embedded orchestration feature flags (YAML → runtime gates) | `crates/vox-orchestrator/src/orchestration_feature_flags.rs` |
| Plan-mode trigger — React vs. plan (D2) | `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs` |
| Risk matrix — HITL escalation (D5+D9) | `crates/vox-orchestrator/src/risk_matrix.rs` |
| Privacy classifier — sensitivity detection (D8) | `crates/vox-orchestrator/src/privacy_classifier.rs` |
| Cache predictor — prefix cache routing (D7) | `crates/vox-orchestrator/src/cache_predictor.rs` |
| Budget gate — token/cost limits (D7) | `crates/vox-orchestrator/src/budget_gate.rs` |
| Compaction trigger — strategy selection (D7) | `crates/vox-orchestrator/src/compaction_trigger.rs` |
| Calibration — drift detection + bandit (D10) | `crates/vox-orchestrator/src/calibration.rs` |
| Sub-agent dispatch — spawn vs. inline (D4) | `crates/vox-orchestrator/src/subagent_dispatch.rs` |
| Orchestrator policy metric_type constants | `crates/vox-telemetry/src/types.rs` — `METRIC_TYPE_*` constants |
| Telemetry master switch + org-policy hard-off | `crates/vox-telemetry/src/config.rs` — `TelemetryConfig::from_env()`, `org_policy_disabled()` (reads `/etc/vox/telemetry-policy.toml`), `is_master_enabled()`. Resolution order: org policy → `VOX_TELEMETRY` env var → per-category vars → default. |
| Telemetry debug sink (stderr JSON dump) | `crates/vox-cli/src/lib.rs` — `StderrDebugSink`; registered when `VOX_TELEMETRY=debug`. |
| Telemetry sink backed by VoxDb | `crates/vox-db/src/telemetry_sink.rs` — `ResearchMetricsSink`; handles `ModelCall`, `TaskRootSummary`, `BuildSummary`, `Error`, `AiFixture`. |
| LLM retry-loop error events | `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs` — `emit_llm_error_event()` helper + `retry_attempt` counter; emitted at each fallback site and terminal failure. |
| Orchestrator feature flags | `contracts/orchestration/feature-flags.v1.yaml` |
| AgentOS MCP → `mutation_kind` SSOT (orchestrator + `std.agentos` in Vox) | `crates/vox-agentos-mutation/` |
| AgentOS ACI envelope + mutation classification (MCP) | `crates/vox-orchestrator-mcp/src/aci/` |
| AgentOS guardrail kernel / checkpoint / intent planner | `crates/vox-orchestrator/src/agentos/` |
| AgentOS policy ledger (MCP mutation_kind → orchestrator risk overlay) | `crates/vox-orchestrator/src/agentos/policy_runtime.rs` |
| AgentOS shell backend adapters (contract-first) | `crates/vox-cli/src/commands/runtime/shell/backends/` |
| Semantic filesystem / intent retrieval bridge | `crates/vox-search/src/semantic_fs/` |
| AgentOS ACI contracts | `contracts/aci/agent-computer-interface.v1.yaml` + `agent-computer-interface.v1.schema.json` |
| Add a code-audit detection rule | `crates/vox-code-audit/src/detectors/<rule>.rs` |
| Add a skill manifest field | `crates/vox-plugin-types/src/skill_manifest.rs` |
| Add a plugin manifest field | `crates/vox-plugin-types/src/plugin_manifest.rs` |
| Add a queue / lock / oplog method | `crates/vox-orchestrator-queue/src/{locks,oplog,affinity}/` |
| Add an LLM provider adapter | `crates/vox-orchestrator-mcp/src/llm_bridge/providers/<name>.rs` |
| Interpreter runtime value representation & memory model (copy-on-write) | `crates/vox-compiler/src/eval/value.rs` (`VoxValue`; `List/Object/Tuple` hold `Rc<Vec<…>>`, `Fn.body` is `Rc`) + `crates/vox-compiler/src/eval/env.rs` (`Scope` frames are `Rc<HashMap>`). No GC — pure value semantics via `Rc` + `Rc::make_mut` CoW; `VoxValue` is intentionally `!Send` (single-threaded interp). See [`vox-memory-model-audit-and-value-optimization-2026-06-05.md`](./vox-memory-model-audit-and-value-optimization-2026-06-05.md). |
| `@versioned` / `@tracked` decorator + interpreter `repo.*` VCS store (auto-snapshot-on-success) | Decorator spine in `crates/vox-compiler/`: lexer token (`src/lexer/token.rs` `AtVersioned`/`AtTracked`) → parser (`src/parser/descent/decl/head.rs` `is_versioned`) → `FnDecl.is_versioned` (`crates/vox-ast/src/decl/fundecl.rs`) → `HirFn.is_versioned` + `uses vcs` injection (`src/hir/lower/decl.rs`) → `VoxValue::Fn { name, is_versioned }` (`src/eval/value.rs`); the auto-`repo.snapshot()` hook fires on successful return in both call paths (`src/eval/mod.rs` `Interpreter::call` + `src/eval/expr.rs` Call arm) against the in-memory `RepoStore` (`src/eval/repo.rs`). Interpreter-only (`--mode interp`); inert in the compiled arms. See [`vcs-as-vox-language-feature-jujutsu-2026.md`](./vcs-as-vox-language-feature-jujutsu-2026.md) §4.3. |
| Add a code generator (Rust target) | `crates/vox-codegen/src/codegen_rust/` |
| Add a code generator (TypeScript target) | `crates/vox-codegen/src/codegen_ts/` |
| Add a layer rule / arch-check rule | `crates/vox-arch-check/src/main.rs` + extend `layers.toml` schema |
| Add an architectural exception (allowed inversion) | Append `[[known_inversions]]` block in [`layers.toml`](./layers.toml) with a `reason` |
| Add a new workspace crate | Update [`Cargo.toml`](../../../Cargo.toml) `[workspace.dependencies]` AND add a row to [`layers.toml`](./layers.toml) — `vox-arch-check` will fail otherwise |
| BundleRef / Bundle / BundleStore (P2-T1) | `crates/vox-package/src/bundle.rs` |
| WorkflowDrainStarted / WorkflowDrainState (P2-T3) | `crates/vox-orchestrator/src/drain_oplog/workflow_drain.rs` |
| activity_result_cache — table DDL + SQL constants (P2-T5) | `crates/vox-db/src/ddl/activity_result_cache.rs` |
| SCIENTIA claim extraction pipeline (VeriScore, MiniCheck, T1→T2) | `crates/vox-claim-extractor/` |
| SCIENTIA drift linter — workspace pattern-repetition checks | `crates/vox-drift-check/` |
| SCIENTIA UK AISI Inspect adapter, atomic-NEI novelty, ChronoFact, EvidenceConflict | `crates/vox-inspect-bridge/` |
| SCIENTIA nanopublication builder — TriG, Ed25519 signing, Trusty URI | `crates/vox-nanopub/` |
| SCIENTIA pre-registration — signing, deviation detection, Bayesian stopping | `crates/vox-prereg/` |
| SCIENTIA research event types and ResearchEventEmitter trait (L1) | `crates/vox-research-events/` |
| SCIENTIA RO-Crate 1.2 JSON-LD builder — CFF, CodeMeta, TOP-Level-2, ACM badges | `crates/vox-ro-crate/` |

> **L0/L1 split:** if your consumer only needs row/param TYPES (no async, no
> connection), depend on `vox-db-types` directly — not on `vox-db`. The full
> `vox-db` crate transitively pulls in `turso` and tokio.

## Plugins (L4 — cdylib only; never compile-time deps for L0..L3)

If you're writing a plugin (concrete sandbox, ML backend, GPU probe, etc.),
it goes in a new `crates/vox-plugin-<name>/` and depends on `vox-plugin-api`.
Don't depend on `vox-orchestrator` or `vox-cli` from a plugin.

| Plugin crate | Provides |
|---|---|
| [`vox-plugin-browser`](../../../crates/vox-plugin-browser/) | Browser automation plugin (chromiumoxide CDP). |
| [`vox-plugin-cloud`](../../../crates/vox-plugin-cloud/) | CloudSync plugin stub: HF Hub / S3 model artifact sync. |
| [`vox-plugin-mens-candle-cuda`](../../../crates/vox-plugin-mens-candle-cuda/) | ML training backend plugin: Candle + CUDA. Implements MlBackend. |
| [`vox-plugin-mens-candle-metal`](../../../crates/vox-plugin-mens-candle-metal/) | MENS Apple Silicon Metal execution plugin. |
| [`vox-plugin-nvml-probe`](../../../crates/vox-plugin-nvml-probe/) | Hardware probe plugin: NVML for NVIDIA GPU introspection. |
| [`vox-plugin-speech`](../../../crates/vox-plugin-speech/) | Speech-to-text + AudioCapture plugin: Candle Whisper backend + mic capture surface (both extensions in one plugin). |
| [`vox-plugin-populi-mesh`](../../../crates/vox-plugin-populi-mesh/) | Populi mesh transport plugin (composite: code + skill). |
| [`vox-plugin-publication`](../../../crates/vox-plugin-publication/) | Publication plugin: RSS/Atom ingest with dedup, Reddit/YouTube publish, scholarly job feeds. |
| [`vox-plugin-runtime-container`](../../../crates/vox-plugin-runtime-container/) | Skill-runtime plugin: Docker + Podman backends for vox-skill-runtime. |
| [`vox-plugin-runtime-wasm`](../../../crates/vox-plugin-runtime-wasm/) | Skill-runtime plugin: wasmtime-based WASI sandbox (default for pure-compute skills). |
| [`vox-plugin-webhook`](../../../crates/vox-plugin-webhook/) | Webhook plugin: HTTP listener with HMAC signature verification (GitHub, Discord, Slack; GitLab deprecated). Inlines the full webhook gateway (inbound handler, outbound delivery, channel adapters, bridge) in the `webhook` submodule — no separate library crate. |

## When to NOT add a new crate

The default answer to "should this be a new crate?" is **no**. Add to an
existing crate unless one of these is true:

### Binary-only tools

Crates with `kind = "binary"` in `layers.toml` (e.g., `vox-arch-check`, `vox-populi`, `vox-orchestrator-d`) don't need a `[workspace.dependencies]` entry in the root `Cargo.toml` — they're consumed via `cargo run -p <name>`, not as library dependencies. The "Add a new workspace crate" instruction below applies to libraries only.

- The new code has zero callers in any existing crate (likely a plugin)
- The new code is **pure types** (no async, no DB) AND will have ≥3 consumers (consider an L0 or L1 crate)
- A subsystem in an existing crate has grown past its `max_loc` budget and is asking to be split (see Phase 4–5 of the [reorg outcome](./2026-05-08-workspace-reorg-outcome.md))

`vox-arch-check`'s orphan detector flags new crates with no consumers. If you
add one, expect that warning to land on your PR until you wire it up — that's
working as intended.

## Skill-only plugins (no `Cargo.toml` — exempt from `layers.toml`)

These live under `crates/` but are **not Cargo crates**. Each has a `Plugin.toml`
and one or more `.skill.md` files. They are loaded at runtime via the plugin
host, not compiled as Rust library crates.

| Dir | Purpose |
|---|---|
| `crates/vox-plugin-host/tests/fixtures/noop-skill/` | No-op stub skill fixture for plugin-host integration tests (not a workspace crate). |
| `crates/vox-plugin-skill-compiler/` | Compiler skill — wraps `vox compile` as a Vox skill. |
| `crates/vox-plugin-skill-git/` | Git skill — wraps common git operations as Vox skills. |
| `crates/vox-plugin-skill-memory/` | Memory skill — CLAUDE.md / MEMORY.md management as a Vox skill. |
| `crates/vox-plugin-skill-orchestrator/` | Orchestrator skill — agent lifecycle management via Vox skill API. |
| `crates/vox-plugin-skill-rag/` | RAG skill — retrieval-augmented generation pipeline as a Vox skill. |
| `crates/vox-plugin-skill-testing/` | Testing skill — test-run orchestration as a Vox skill. |
| `crates/vox-plugin-skill-testing-validate/` | Test-validate skill — validates test outputs against expected results. |
| `crates/vox-plugin-skill-v0/` | Legacy v0 skill format shim — compatibility bridge. |

## Planned but not yet landed

Crates documented in architecture plan docs but with **no directory on disk yet**.
The [`[planned]` table in `layers.toml`](./layers.toml) is the canonical index;
each row there points to the plan document that owns the work.

Do **not** create a new crate whose name matches these before checking the linked
plan document — the design context and API shape are likely already specified.

### SCIENTIA pipeline phases

These will be folded into `vox-scientia` sub-modules when implemented (Phase I onwards):

| Planned crate | Notes |
|---|---|
| `vox-claim-extractor` | SCIENTIA claim extraction: VeriScore, MiniCheck, T1→T2. |
| `vox-inspect-bridge` | UK AISI Inspect adapter, atomic-NEI novelty, ChronoFact. |
| `vox-prereg` | Pre-registration: Trusty URI signing, deviation detection. |
| `vox-ro-crate` | RO-Crate 1.2 JSON-LD metadata builder. |
| `vox-scientia-ingest` | SCIENTIA corpus ingestion pipeline. |

### Orchestrator extractions

| Planned crate | Notes |
|---|---|
| `vox-cli-ci` | **Planned — not yet landed.** `vox ci` subcommand extraction from `vox-cli/src/commands/ci/` (22K LoC, 74 files). Blocked by 3 shared modules that must move to `vox-cli-core` first. See [2026-05-15-cli-ci-extraction-plan.md](./2026-05-15-cli-ci-extraction-plan.md). |
| `vox-orchestrator-core` | **Planned — not yet landed.** Extract `src/orchestrator/` (12,825 LoC) + `Orchestrator` struct into new crate. Blocked by Rust coherence (all inherent `impl` blocks must co-move with the struct); minimum co-move set ~17K LoC. See [2026-05-15-orchestrator-tier-d-plan.md](./2026-05-15-orchestrator-tier-d-plan.md). |
| `vox-orchestrator-cap-mint` | Sealed-trait façade for capability minting (P3-T6). |

### Mesh / release packaging

| Planned crate | Notes |
|---|---|
| `vox-mesh-models` | Mesh model registry aggregation types. |
| `vox-agentos-mutation` | AgentOS mutation-kind SSOT — code currently lives in `vox-primitives::agentos_mutation`. Extract when fan-in ≥ 3. |
| `vox-checksum-manifest` | SHA-256 release asset verification. |
| `vox-release-artifacts` | `.tar.gz`/`.zip` packaging helpers for `vox compile`. |
| `vox-assets` | `[bundle.assets]` manifest validation + staged copy tree. |

### HTTP surface planned

| Planned crate | Notes |
|---|---|
| `vox-http-envelope` | Wire-format v1 §6 JSON error envelope for generated Axum handlers. |

### Misc

| Planned crate | Notes |
|---|---|
| `vox-share` | Public-URL tunneling (Cloudflare Quick Tunnels, localhost.run, Tailscale). |
| `vox-ssg` | Static site generator for Vox docs surface. |
| `vox-exec-grammar` | AST parser and risk classifier for shell/Vox command invocations. |
| `vox-install-policy` | SSOT constants for Vox install/update surfaces. |
| `vox-dashboard` | Local Axum-served orchestration dashboard (SPA host). |
| `vox-mens-eval` | Mn-T12 eval harness types (`CompileVerdict`). |
