---
category: "Architecture SSOTs"
title: "VoxMens / Populi GUI surfaces — CLI-derived design (Amendment A)"
date: 2026-06-26
status: design
---

# VoxMens / Populi GUI surfaces — CLI-derived design

Amendment A of the ratified GUI-IA blueprint. The `mens` and `populi` surfaces
(nav group `compute`) must get a **real GUI derived from the existing CLI** to
establish GUI/CLI parity, rather than being cut.

> **Premise correction.** `mens` and `populi` are *not* null/empty surfaces.
> They are already registered `curated_decorator` surfaces backed by
> `CommandCardsView` (read-only command cards) in
> `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`. The real gap
> is **coverage**: the decorators expose 3 (`mens`: status/models/probe) and 2
> (`populi`: status/registry-snapshot) read-only cards out of a **very large**
> CLI surface, and there are **zero** Tauri command wrappers for any
> mens/populi *action* (train, serve, mesh up/down, dispatch, etc.). This doc
> closes that gap.

## 1. The CLI command tree (source of truth)

Both command trees live in `crates/vox-ml-cli/` and are exposed as top-level
`vox` subcommands. Full enums:

- `vox mens`  — `crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs` (`PopuliAction`)
- `vox mens corpus` — `crates/vox-ml-cli/src/commands/corpus/mod.rs` (`CorpusAction`)
- `vox populi` — `crates/vox-ml-cli/src/commands/populi_cli.rs` (`PopuliCli`)

### `vox mens` — custom-model train / run (VoxMens)

| Command | Kind | Notes |
|---|---|---|
| `mens pipeline` | orchestrate | full extract→validate→mix→eval→train dogfood loop; `--preset`, `--profile`, `--stages`, `--dry-run` |
| `mens train` | **fine-tune (long job)** | Candle QLoRA (NF4) / Burn LoRA; ~60 flags incl. `--device`, `--preset`, `--domain` (spoke), `--cloud {local,runpod,vast}`, `--adapter-tag`, `--background` |
| `mens dogfood` | fine-tune (long job) | zero-config `train` alias |
| `mens serve` | **run / serve (long job)** | OpenAI-compatible HTTP server; `--model`, `--port`, `--cloud`, `--model-hf` |
| `mens corpus …` | data pipeline | ~30 subcommands: extract / extract-rs / extract-docs / mix / validate / eval / stats / pairs / readiness / replay / … |
| `mens eval-local` | evaluate | run checkpoint vs heldout bench, pass@k |
| `mens eval-gate` | evaluate (gate) | check run dir vs `eval-gates.yaml`; exit 1 on block |
| `mens baseline` | evaluate | capture base-model BFCL baseline for beat-base gate |
| `mens eval-collateral-damage` (`eval`) | evaluate | catastrophic-forgetting check pre vs post |
| `mens probe` | diagnostics (read) | GPU caps + recommended LoRA config |
| `mens status` | status (read) | run telemetry / `--quotas` / `--cloud` / `--db` |
| `mens watch-telemetry` (`watch`) | **stream (read)** | periodic tail of telemetry.jsonl + train.err.log |
| `mens models` | registry (read) | list trained adapters/checkpoints |
| `mens merge-qlora` (`merge-adapter`) | utility (job) | merge adapter into base; `--quantize` |
| `mens export-gguf` | utility | GGUF export (not yet implemented) |
| `mens bench-completion` | benchmark | latency/throughput of completion server |
| `mens system-prompt-template` | utility (read) | emit IDE system prompt |
| `mens generate` / `review` / `check` / `fix` | AI codegen (`mens-dei`) | NLP→code, AI review/bug-detect/fix |

**Qwen3 ladder vocabulary** surfaces as flags, not subcommands: *hub* = base
model (`--model` / VoxMens SSOT default); *spokes* = domain QLoRA adapters via
`--domain` / `--profile` / corpus `--spoke` (`vox-lang`, `rust-expert`,
`agents`, `tool-selection`, `argument-generation`); *RunPod/local* = `--cloud`.

### `vox populi` — distributed mesh / model population

| Command | Kind | Notes |
|---|---|---|
| `populi init` / `up` / `down` | **lifecycle (long job)** | start/stop local mesh node; `up` has visibility/donation/federation flags |
| `populi status` | status (read) | mesh health + overlay diagnostics; `--json` |
| `populi registry-snapshot` (`local-status`) | status (read) | on-disk registry + env |
| `populi serve` | control plane (long job) | HTTP control plane; `--enable` opt-in |
| `populi config show` / `check` | config (read) | resolved config + source attribution |
| `populi node join` / `leave` / `list` | mesh nodes | worker registration |
| `populi federation list` / `pair` | federation | discover/join peer meshes |
| `populi dispatch` / `result` | remote exec | run a `.vox` script on the mesh; poll detached result |
| `populi stats` | status (read) | queue depth by kind/priority |
| `populi identity show / export / set-visibility / reputation / rotate / …` | identity | Ed25519 mesh identity + donation policy |
| `populi admin maintenance / quarantine / exec-lease-revoke` | operator | node drain / block / lease revoke |
| `populi corpus …` | data pipeline | extract / dpo / mutate / snapshot / benchmark-gen / flywheel-check / … |
| `populi attest` / `join` | federation | signed attestation; join public volunteer network |

## 2. mens vs populi — the job split

Grounded in the CLI, not just the Latin:

- **`mens` ("mind") = the single-model train/run lab.** Everything that
  produces or serves *one* custom model on *this* operator's hardware (or a
  cloud GPU they rent): probe → corpus prep → train/fine-tune (hub + spoke
  adapters) → evaluate/gate → serve → merge/export. It is the VoxMens
  authoring surface. The unit of work is a **training run** / **adapter**.

- **`populi` ("the people/crowd") = the mesh + model-population control plane.**
  Everything plural and distributed: bring the mesh node up/down, see who is in
  the network (nodes/federation), dispatch work across the crowd, manage this
  node's identity/reputation/donation policy. The unit of work is a **node** /
  **dispatch** / **mesh**.

**Relationship to the existing `models` surface (`ModelsView`).** `models` is
the *consumer/router* view — the live registry of models the orchestrator can
route to, with scoreboard and active-model selection. It is **distinct** from
`mens`: `models` answers "which model runs my agents right now," while `mens`
answers "train me a new one." Recommended cross-links: a `mens` run that
finishes and registers an adapter should deep-link into `models`; `populi`'s
mesh-served models should appear in `models` with a `mesh` provenance tag (the
`trust_mesh_node` / mesh-policy commands already exist in
`crates/vox-gui/src/commands/`). No overlap to resolve — keep three surfaces.

## 3. Reuse plan (existing GUI patterns)

| Need | Reuse | Location |
|---|---|---|
| Run any read-only CLI path | **`execute_command` Tauri command** (sidecar runner; supports `path` + `__argv`/`__positionals`/`__flags`) | `crates/vox-gui/src/commands/execute.rs` |
| Read-only command-card panels | `CommandCardsView` + `commandSurface()` factory | `decoratorRegistry.ts`, `CommandCardsView.tsx` |
| Live data surface (tables, polling) | `ModelsView` / `RunsView` pattern: `invoke<T>()` in `useCallback` + `useEffect` interval | `surfaces/Models/ModelsView.tsx`, `surfaces/Runs/RunsView.tsx` |
| Long-running job progress (stream) | Tauri event stream `listen<T>('vox://…')` with **polling fallback**; see `useOrchestratorStatus` | `ui/src/transport.ts`, `ui/src/hooks/useOrchestratorStatus.ts` |
| UI primitives | `Glass`, `DataTable`, `StatusPill`, `Button`, `Icon`, `EmptyState` | `ui/src/components/ui/*` |
| Run lifecycle persistence | `start_gui_run` / `finish_gui_run` / `list_gui_runs` + `AgentRunRow` | `crates/vox-gui/src/commands/runs.rs` |
| Surface registry / nav | YAML SSOT + generator; mens/populi already in `compute` group | `contracts/gui/surface-registry.v1.yaml`, `gui_surface_registry.rs` |

**Key architectural lever:** `execute_command` already shells the `vox`
sidecar with an arbitrary subcommand path. So **read-only and fire-and-forget
controls need NO new Rust** — a form just builds `path: ['mens','probe']` (etc.)
and `args`. New `#[tauri::command]` wrappers are only needed where we want
**structured long-running streaming** (train/serve/mesh-up) surfaced as live
progress rather than a blocking shell call.

## 4. GUI/CLI parity mapping

Legend — **Wire:** `exec` = existing `execute_command` seam (no new Rust);
`stream` = needs a new streaming `#[tauri::command]` wrapper; `tauri✓` = a
wrapper already exists.

### mens surface

| CLI command | GUI control | Wire |
|---|---|---|
| `mens probe [-d]` | "Probe GPU" button → results card | exec |
| `mens status [--quotas/--cloud/--db]` | Status panel (auto-refresh) | exec |
| `mens models` | "Trained models" table (link to `models`) | exec |
| `mens corpus stats/readiness/eval` | "Corpus" tab: readiness gauge + stats table | exec |
| `mens corpus mix/validate/extract*` | "Build corpus" form (source + output fields) | exec |
| `mens pipeline` (`--dry-run` first) | "Run pipeline" wizard (preset/profile/stages) | **stream** |
| `mens train` / `mens dogfood` | **"New training run" form** (preset, domain/spoke, device, cloud, epochs) + live progress | **stream** |
| `mens watch-telemetry` | live loss/step chart on the active run | **stream** |
| `mens eval-local` / `eval-gate` / `baseline` / `eval` | "Evaluate" panel; gate pass/fail `StatusPill` | exec |
| `mens serve` | "Serve model" toggle (port, model, cloud) | **stream** |
| `mens merge-qlora` / `export-gguf` | "Export" form (adapter + base shards, quantize) | exec |
| `mens bench-completion` | "Benchmark" button against a served URL | exec |
| `mens system-prompt-template` | "Copy IDE prompt" action | exec |

### populi surface

| CLI command | GUI control | Wire |
|---|---|---|
| `populi status` / `stats` / `registry-snapshot` | Mesh health dashboard (auto-refresh) | exec |
| `populi config show` / `check` | "Config" panel (source attribution) | exec |
| `populi init` | "Initialize mesh" button | exec |
| `populi up` / `down` | **Mesh power toggle** (visibility, donation, federation flags) + live state | **stream** |
| `populi serve --enable` | "Control plane" toggle | **stream** |
| `populi node list` / `join` / `leave` | "Nodes" table + join/leave actions | exec |
| `populi federation list` / `pair` | "Federation" panel + pair-token form | exec |
| `populi dispatch <script>` / `result <id>` | "Dispatch" form (script picker, labels, priority) + result poll | exec (poll) |
| `populi identity show / export / set-visibility / prefer-mesh / reputation / rotate / set-policy` | "Identity" panel (key, reputation, visibility, donation policy editor) | exec (`set-policy` already mirrors `set_task_mesh_policy` tauri✓) |
| `populi admin maintenance / quarantine / exec-lease-revoke` | "Operator" panel (node drain/quarantine/lease) | exec |
| `trust/untrust mesh node` | per-node trust toggle | **tauri✓** (`trust_mesh_node`/`untrust_mesh_node`) |
| `populi corpus …` | (defer — overlaps `mens corpus`; link, don't duplicate) | exec |
| `populi attest` / `join` | "Join public mesh" action | exec |

## 5. New Tauri wrappers needed (the only new Rust)

Everything else rides `execute_command`. Add a small set of **streaming**
wrappers (new files `crates/vox-gui/src/commands/mens.rs`,
`crates/vox-gui/src/commands/populi.rs`) that spawn the sidecar and emit a
`vox://…` event stream the GUI subscribes to, mirroring `useOrchestratorStatus`:

1. `mens_train_start(config) -> run_id` + emits `vox://mens-train` (step/loss/
   eta frames) — wraps `mens train` with `--background`; persists via
   `start_gui_run`/`finish_gui_run`.
2. `mens_train_stop(run_id)` — cooperative cancel.
3. `mens_serve_start/stop` + `vox://mens-serve` (server up/down, port, requests).
4. `populi_up/down` + `vox://populi-state` (node up/down, peer count).
5. (optional) `mens_watch_telemetry(run_id)` if we prefer a Rust tail over a
   shell `watch` — but `execute_command` + client poll is the cheaper v1.

**v1 cut line:** ship all `exec`-wired read panels and forms first (no new
Rust). Add the 4–5 streaming wrappers in v2 for live train/serve/mesh progress.

## 6. Registry / gate updates

- `contracts/gui/surface-registry.v1.yaml`: keep `mens`/`populi` in `compute`;
  optionally promote tier `curated_decorator → live_backend` once dedicated
  `MensView`/`PopuliView` replace the command-card decorators. Regenerate with
  `vox ci gui-surface-registry --write`.
- Honesty scan (`surfaces/__guards__/honestyScan.ts`): new views must avoid
  placeholder text / dead `onClick={() => {}}` — every control wires to a real
  `execute_command` path or a wrapper.
- Replace the two `commandSurface(...)` decorator entries with the real views
  in `decoratorRegistry.ts` when v2 lands.

## 7. Open questions (need human decision)

1. **Local vs RunPod from the GUI.** Does the train/serve form let the operator
   pick `--cloud {local,runpod,vast}` and spend money from the GUI, or is the
   GUI **local-only** (cloud stays CLI-gated for cost safety)? Affects whether
   we surface `--max-budget`, `--cloud`, billing/cost panels.
2. **Launch vs monitor.** Should the GUI **launch** long jobs (train/serve/mesh
   up) — requiring the v2 streaming wrappers — or only **monitor** runs started
   from the CLI in v1 (read-only `status`/`watch-telemetry`)? This sets the v1
   scope and how much new Rust we write now.
3. **populi corpus vs mens corpus.** The two corpus trees overlap heavily. Pick
   one canonical corpus surface (recommend: under `mens`) and have the other
   deep-link, or keep both? Avoids a split-brain data-pipeline UI.
4. **`models` ↔ `mens`/`populi` provenance.** Confirm the cross-link model:
   finished `mens` adapters and `populi` mesh-served models register into the
   existing `models` registry with a provenance tag, vs each surface keeping its
   own list. (Recommended: single `models` registry, tagged provenance.)
5. **Operator/admin gating.** `populi admin` (maintenance/quarantine/lease) and
   `identity export` (private key) are destructive/sensitive. Gate behind a
   confirm + the existing approvals/HITL path, or hide from the default surface?
6. **Spoke ladder UX.** Should "New training run" expose the Qwen3 hub+spoke
   ladder explicitly (pick base hub + one of the 5 domain spokes) as first-class
   UI, or keep it as advanced `--domain`/`--preset` fields? Determines how
   opinionated the train form is.
