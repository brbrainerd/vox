---
title: "Mesh, Dashboard & Distributed Compute — Research (2026-05-09)"
description: "Two-horizon research synthesis for the Vox mesh — personal mesh today, grand volunteer compute network tomorrow. Audits the current mesh / dashboard / durable-workflow / multi-agent-VCS state, surveys prior art (BOINC, Tailscale, Ray, Temporal, Restate, Pijul, JJ, Akash), and lays out the security, killer-feature, language-level, and agent-collaboration changes needed to take Vox from trusted-LAN to bounded-trust internet-facing. Not yet an implementation plan — feeds the next plan-of-record."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Synthesis-of-record for the mesh/distributed-compute direction; names the audited gaps and the Wave-2 design space so subsequent plans don't re-survey."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-populi: control plane, registry, A2A inbox, mens stack, hardware probes"
  - "vox-mesh-types: shared mesh transport types"
  - "vox-plugin-populi-mesh: cdylib transport plugin"
  - "vox-dashboard: Axum SPA host, mesh-control surfaces"
  - "vox-orchestrator: A2A dispatch, file-affinity router, capability tokens, oplog"
  - "vox-orchestrator-queue: locks, oplog, affinity tracking"
  - "vox-orchestrator-types: VCS capability tokens"
  - "vox-workflow-runtime: interpreted durable workflow MVP"
  - "vox-actor-runtime: actors, mailboxes, supervision"
  - "vox-compiler / vox-codegen: workflow / activity / actor keywords; effect annotations"
  - "vox-identity / vox-crypto: Ed25519 / X25519 / JWE primitives"
  - "vox-package: content-addressed code store"
  - "vox-git / vox-forge: git bridge and forge interop"
---

# Mesh, Dashboard & Distributed Compute — Research

> **Status.** Research synthesis. Not an implementation plan. Eventually decomposes into one or more plans-of-record (lock-leader durability, language-level distribution annotation, dashboard "Add a Node" wizard, etc.). Per [feedback](../../../CLAUDE.md): scope-check before any of those plans gets implemented.

## 0. Reading guide

This document spans four concentric horizons:

1. **What we have today** (§1) — the truth about mesh, dashboard, durable workflows, and multi-agent VCS as of 2026-05-09. File-cited; not glossed.
2. **What's missing** (§2) — killer features, ranked. Mesh transport, dashboard control, language primitives.
3. **Security and trust** (§3) — the threat model, the prior-art lessons, and the critical path from trusted-LAN to safely-internet-facing.
4. **The grand VOX network** (§4) — what would make a global volunteer compute mesh tractable: language-level durability, idempotency, attestation, multi-agent VCS over mesh.

Companion specs (already exist; this doc cross-references them rather than restating):

- [populi-mesh-north-star-2026.md](populi-mesh-north-star-2026.md) — three-slice (S1/S2/S3) capability roadmap and seven workstreams (W1–W7).
- [populi-mesh-improvement-backlog-2026.md](populi-mesh-improvement-backlog-2026.md) — flat MESH-001..MESH-210 backlog.
- [populi-mesh-a2a-durability-spec-2026.md](populi-mesh-a2a-durability-spec-2026.md) — durable A2A store (W6).
- [populi-mesh-config-baseline-spec-2026.md](populi-mesh-config-baseline-spec-2026.md) — `[mesh]` config schema (W7).
- [populi-mesh-local-observability-spec-2026.md](populi-mesh-local-observability-spec-2026.md) — `vox.mesh.*` span attributes (W5).
- [populi-mesh-probe-correctness-spec-2026.md](populi-mesh-probe-correctness-spec-2026.md) / [-plan-2026.md](populi-mesh-probe-correctness-plan-2026.md) — hardware probe trait (W2).
- [vox-dashboard-design-brief-2026.md](vox-dashboard-design-brief-2026.md) — surfaces, navigation, anti-patterns.
- [dashboard-migration-research-2026.md](dashboard-migration-research-2026.md) — current Axum + React shape.
- [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md) — auth/transport interop.
- [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md) — workflow runtime gaps.
- [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md) — five-phase language enforcement plan.
- [multi-agent-vcs-replication-research-2026.md](multi-agent-vcs-replication-research-2026.md) / [-spec-2026.md](multi-agent-vcs-replication-spec-2026.md) — convergence sets, op-fragments.
- [agentic-version-control-automation-research-2026.md](agentic-version-control-automation-research-2026.md) — capability tokens, banned commands.
- [git-concurrency-policy.md](git-concurrency-policy.md) — git-exec banned-command policy.
- [cryptography-ssot-2026.md](cryptography-ssot-2026.md) — sole crypto SSOT.
- [ludus-identity-github-integration-research-2026.md](ludus-identity-github-integration-research-2026.md) — GitHub-attested identity model.
- [ludus-security-and-anti-cheat-research-2026.md](ludus-security-and-anti-cheat-research-2026.md) — DevRank, tiered trust.
- [scientia-mesh-integration-research-2026.md](scientia-mesh-integration-research-2026.md) — discovery feedback loop.
- [nextgen-orchestrator-research-2026.md](nextgen-orchestrator-research-2026.md) — synthesis of orchestrator failure modes.

---

## 1. Current state — audited

### 1.1 Mesh transport & control plane (today)

**What works.** [`vox-populi`](../../../crates/vox-populi/) ships single-node-correct: HTTP control plane with bearer auth (Mesh / Worker / Submitter / Admin roles, optional JWT-HS256), node registry persisted as JSON, A2A inbox with idempotency keys and lease envelopes, mens training in-process via Burn/Candle. Hardware probes (NVML / wgpu / DRM / Metal) advertise capacity. Bearer comparison uses [`subtle::ConstantTimeEq`](../../../crates/vox-populi/src/transport/auth.rs).

**Architecture.** A2A messages (`A2ADeliverRequest`, [vox-mesh-types/src/a2a.rs:5](../../../crates/vox-mesh-types/src/a2a.rs)) carry task spec, optional JWE-wrapped secrets, idempotency key, and W3C `traceparent`. Orchestrator forwards through `vox-orchestrator/src/a2a/dispatch/mesh.rs`; receiver polls inbox via `vox-orchestrator/src/a2a/remote_worker.rs`. The transport plugin [`vox-plugin-populi-mesh`](../../../crates/vox-plugin-populi-mesh/) is the cdylib seam between `vox-mesh-types` (L0) and the HTTP handlers; orchestrator and CLI talk through a `MeshDriver` trait.

**Five footguns the audit surfaced (severity-ordered).**

1. **Local-first execution shadows the mesh.** ADR-017 declares leases authoritative; [`a2a/dispatch/mesh.rs`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs) still local-falls-back on any send failure without checking whether the remote node has already claimed. Result: silent duplicate execution under packet loss; "the mesh" is closer to a debug visualization than a runtime. North-star §6.1 already calls this out.
2. **JWE secrets are encrypted at send and never decrypted on the worker.** [`vox-orchestrator/src/a2a/jwe.rs`](../../../crates/vox-orchestrator/src/a2a/jwe.rs) defines `decrypt_jwe_compact()`; the worker hot path validates well-formedness and walks away ([`remote_worker.rs:117`](../../../crates/vox-orchestrator/src/a2a/remote_worker.rs)). Cross-node secret sharing is theatre until W3 lands.
3. **Hardware probes are unvalidated.** No mock harness, no probe-replay test (W2 fixes this); operator labels override probe output, so routing decisions depending on probe results are unreliable.
4. **No resource accounting wired up.** [`vox-mesh-types/src/kudos.rs`](../../../crates/vox-mesh-types/src/kudos.rs) defines `KudosPrimitive` and `CreditJobRequest`; nothing computes `duration_ms`, credits a node, or queries a contribution ledger. The "shared compute volunteer network" depends on this and the plumbing does not exist.
5. **Bare HTTP, no TLS.** All-local testing fine; cross-node deployments send `TaskSpec`, signatures, and JWE-wrapped secrets over plaintext. Bearer tokens are in cleartext `Authorization` headers. Scope-id is membership, not authentication.

### 1.2 Dashboard mesh-control surface (today)

**What ships.** [`vox-dashboard`](../../../crates/vox-dashboard/) is Phase-1: an Axum server hosting a React 19 SPA compiled from Vox view-language. Transport is WebSocket (`/v1/ws`) for events + HTTP `POST /v1/tools/call` for commands ([`transport.ts`](../../../crates/vox-dashboard/src/transport.ts)). Bearer token injects via meta tag. Localhost auto-binds with `allow_unauthenticated = true`.

**What's stub-only.** Mesh routes return fixture JSON acks. The audit table:

| Surface | Route / file | Status |
|---|---|---|
| List nodes | `GET /api/v2/mesh/nodes` ([api/mesh.rs:82](../../../crates/vox-dashboard/src/api/mesh.rs)) | **Fixture stub.** Comment: "Phase 2 replaces this stub with a live read from the orchestrator mesh registry." |
| Add a node | — | **Missing.** No provisioning route. |
| Remove a node | — | **Missing.** |
| Configure node role | — | **Missing.** |
| Dispatch a job | [`generated/TaskDispatch.tsx:3`](../../../crates/vox-dashboard/app/src/generated/TaskDispatch.tsx) | **UI-only stub** (toggles local state). |
| View job status / logs | `GET /api/v2/runs[/{id}]` ([api/runs.rs](../../../crates/vox-dashboard/src/api/runs.rs)) | **Fixture stub.** |
| Kill / pause / replay | `POST /api/v2/mesh/nodes/{id}/{kill,pause,replay}` ([api/mesh.rs:165](../../../crates/vox-dashboard/src/api/mesh.rs)) | **Acks; no orchestrator wiring.** |
| Topology view | [`generated/NetworkTab.tsx`](../../../crates/vox-dashboard/app/src/generated/NetworkTab.tsx) | **Empty placeholder.** |
| Models surface | `GET /api/v2/models/usage_24h` | **Fixture stub.** |

**FFScript is unrelated.** The FFScript mutation API (see [ffscript-mutation-api-spec-2026.md](ffscript-mutation-api-spec-2026.md), [-panel-schema-spec](ffscript-panel-schema-spec-2026.md), [-linter-design](ffscript-linter-design-2026.md)) is a document-mutation API for FableForge — not dashboard panels. Vox dashboard panels are authored in Vox view-language that lowers to TSX. Don't conflate them.

**Gap for personal-mesh spin-up.** The "I just opened the dashboard for the first time → my friend's GPU is running my jobs" journey has no hand-off pairing flow. Stages 4–7 of that journey (wizard, pairing, donation-policy editor, live topology with handshake feedback) do not exist. Stages 8–10 (dispatch a job, watch tokens stream, see the run land in Runs) have stubs but no live wiring.

### 1.3 Durable workflows & language layer (today)

**Syntax-complete, execution-incomplete.** Vox parses three durability keywords ([`vox-compiler/src/ast/decl/fundecl.rs`](../../../crates/vox-compiler/src/ast/decl/fundecl.rs)), lowers them to a `DurabilityKind` enum ([`vox-compiler/src/hir/nodes/durability.rs`](../../../crates/vox-compiler/src/hir/nodes/durability.rs)), and emits identical async Rust for all three. Codegen ignores `schedule_interval` and `durability` metadata. Effect annotations (`Net | Db | Fs | Env | Clock | Random | Spawn | Mcp(name) | Nothing` — [`effect.rs`](../../../crates/vox-compiler/src/ast/decl/effect.rs)) parse and propagate via [`typeck/effect_check.rs`](../../../crates/vox-compiler/src/typeck/effect_check.rs) but don't reject `time.now()` / `random.*` inside a `workflow` body.

**Workflow runtime ([`vox-workflow-runtime`](../../../crates/vox-workflow-runtime/)).** Linear interpreter with a JSON journal in vox-db. Idempotency is opt-in user-supplied `with { activity_id: "…" }` strings. Replay supports literal loops + deterministic conditionals; `match` and complex branching unsupported. Persistent actor state fields parse but do not compile (see [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md) §rationale).

**Distribution today.** Coupled to a `mesh_*` naming convention plus a `with { required_labels, is_detached }` decorator. No `@remote` annotation, no typed `Worker` value, no language awareness of distribution.

**Concrete gaps vs. Temporal/Restate/Cadence:** workflow.ID + execution.ID; structured signals/child-workflows; deterministic activity-ID derivation; saga compensation DSL; side-effect determinism guard at compile time. Phase 5 of [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md) commits to closing these — but as warnings, not errors, in v0.5.

### 1.4 Multi-agent VCS (today)

**Worktree-per-agent under one daemon.** Each agent gets `.claude/worktrees/<branch>/`. [`vox-orchestrator-d`](../../../crates/vox-orchestrator-d/) is single-instance per machine. File-level locks (Exclusive / SharedRead) live in an in-memory `Arc<RwLock<HashMap<PathBuf, LockEntry>>>` ([`vox-orchestrator-queue/src/locks/mod.rs`](../../../crates/vox-orchestrator-queue/src/locks/)).

**Capability tokens.** `WorkingTreeWrite` and `BranchCreate` ([`vox-orchestrator-types/src/vcs_capability.rs`](../../../crates/vox-orchestrator-types/src/vcs_capability.rs)). Soft-private mints (`#[doc(hidden)] pub fn mint`); Phase 4 hardens to `pub(crate)` + sealed traits. No revocation today.

**Banned-command enforcement.** [`git-concurrency-policy.md`](git-concurrency-policy.md): `git stash`, `git reset --hard`, `git clean -f*`, `git restore .`, `git checkout .` all rejected at the `GitExec` layer before spawning git. Agentic commits get three trailers (`Co-authored-by`, `Vox-Model-Id`, `Vox-Workspace`). Pre-commit secret scan; repo-local `pre-commit` hook bypassed.

**Oplog ([`vox-orchestrator-queue/src/oplog/mod.rs`](../../../crates/vox-orchestrator-queue/src/oplog/)).** `OperationEntry` has `id`, `agent_id`, `timestamp_ms`, `kind` (13+ variants), `snapshot_before/after`, `change_id`, `model_id`, and a SHA-3-256 `predecessor_hash` chain. **In-memory only.** Lost on daemon restart. No replication.

**The single-machine assumption is the load-bearing constraint.** Two daemons on different mesh nodes coordinating the same repo would race on locks, oplog causality, file-affinity assignments, and worktree creation. Phase 3 of [multi-agent-vcs-replication-spec-2026.md](multi-agent-vcs-replication-spec-2026.md) plans op-fragment gossip via Populi mesh; today is local-only.

---

## 2. Killer features missing

### 2.1 Mesh transport & control plane

Ranked by impact ÷ effort, drawing on the audit and on the populi-mesh north-star/backlog:

| Rank | Feature | Why it's a killer | Crate home |
|---|---|---|---|
| 1 | **Authoritative leases (W1, ADR-017)** wired through dispatch | Closes the local-first shadow; turns `the mesh` from advisory into runtime | `vox-orchestrator/src/a2a/dispatch` + `vox-orchestrator-queue` (lease persistence) |
| 2 | **JWE decrypt on the worker (W3)** | Secrets actually become end-to-end | `vox-orchestrator/src/a2a/remote_worker.rs` + `vox-crypto` |
| 3 | **TLS or WireGuard sidecar** on the populi HTTP plane | Closes passive eavesdropping; prerequisite for everything else internet-facing | `vox-populi/src/transport/` (option) or sidecar |
| 4 | **Resource accounting end-to-end** — populate `KudosPrimitive::GpuComputeMs` from `TaskResult.duration_ms`, persist a contribution ledger | Volunteer mesh requires this; types are already there | `vox-populi/src/quota/` (new) + `vox-mesh-types::Kudos` |
| 5 | **Per-key quota + reputation EMA** in `vox-mesh-types::NodeRecord` sidecar | Abuse fuse before opening to strangers | `vox-populi/src/quota/` |
| 6 | **GitHub-attestation gate at pairing time** (Ludus device-flow) | Sybil resistance without a Vox-owned account system | `vox-identity` + `ludus-identity-*` integration |
| 7 | **Per-node sandbox boundary** — move in-process executor behind `SkillRuntime` trait | Prerequisite for accepting work from non-paired peers | `vox-skill-runtime` (trait already exists) |
| 8 | **Result attestation via signed deterministic replay** with submitter-side spot-checking | Trust-but-verify scales without TEEs | `vox-mesh-types::TaskResult.attestation` (already a field) |
| 9 | **Cross-node trace continuation (W5/S2)** — receiver continues `traceparent` | Debugging multi-node failures becomes possible | `vox-populi/src/transport/` + `vox-orchestrator/src/a2a/` |
| 10 | **Mesh-wide model inventory** — refresh + aggregate so the planner sees what LoRAs live where | Ends "have to retry on local because we forgot the remote has the weights" | `vox-populi/src/registry/` + `vox-orchestrator::models` |

### 2.2 Dashboard mesh-control GUI

Ranked top-18; the table maps each to its crate home and to a complexity tier (S/M/L). Sources: prior art (Tailscale, Coolify, Ray, Grafana, Slurm-web, Modal/Replicate, Temporal UI) — see §3.5 for citations.

| # | Feature | Value (one line) | Crate | Cx |
|---|---|---|---|---|
| 1 | **Live topology canvas with health colors** | One glance → which node is on fire | `vox-dashboard` (frontend) + orchestrator EventBus | M |
| 2 | **"Add a Node" wizard** (one-shot install + QR-code as coequal) | The on-ramp; without this, mesh is a bookshelf | `vox-dashboard/src/api/mesh` + `vox-crypto` + `vox-identity` | L |
| 3 | **Per-node spend gauge + mesh-wide budget bar** | Stops bill-shock; matches existing `budget.*` settings | `vox-dashboard` + `vox-telemetry` | S |
| 4 | **Donation-policy editor** (slots, kinds, NSFW filter, per-peer overrides) | Volunteers gate what their hardware does | new `vox-mesh-policy` (L2) consumed by orchestrator admission | M |
| 5 | **"Replay this run on another node"** (stub route already exists) | Ergonomics win on top of an existing seam | `vox-dashboard` + `vox-orchestrator-queue` | S |
| 6 | **Privacy-class indicator on every job + every span** | Ties dashboard to `vox.mesh.privacy_class` from S1 obs spec | `vox-dashboard` + `vox-mesh-types` | S |
| 7 | **Audit-log scrubber** (timeline slider over oplog → state at instant) | The Temporal-replay equivalent for Vox | `vox-orchestrator-queue` (oplog reader) + `vox-dashboard` | M |
| 8 | **Mesh-wide model registry view** ("which LoRA / Ollama tag lives where") | Answers "who can run llama-70b?" before dispatch | `vox-dashboard` + new `vox-mesh-models` query | M |
| 9 | **Workflow visual debugger** (timeline of activity calls) | Pairs Forge with the durable workflow runtime | `vox-actor-runtime` event hooks + `vox-dashboard` | L |
| 10 | **Dry-run / cost-preview before dispatch** | "$0.18 on opus-4.7 at sonnet's p50 latency" | `vox-orchestrator` planner cost estimator + `vox-dashboard` | M |
| 11 | **"What is this worker computing now?"** (privacy-tier-gated live span) | Mesh-debugging-as-a-feature | `vox-populi` + `vox-orchestrator-queue` + `vox-dashboard` | M |
| 12 | **Onboarding wizard for joining someone else's mesh** | Inverse of #2; paste an invite → become a worker | `vox-dashboard` + `vox-populi` | M |
| 13 | **GitHub-Identity-bound role assignment** (mesh-mate "trust ledger") | "Allow @aurelia code-edit jobs; everyone else read-only" | `vox-identity` trust ledger + `vox-mesh-policy` | M |
| 14 | **Mesh-aware command palette (`⌘K`)** ("kill on node X", "drain Y", "send latest to friend-gpu") | Operator power tool; already in design brief | `vox-dashboard` (cmdk.vox) | S |
| 15 | **Per-node real-time terminal** (Coolify-style) over WS, gated by privacy/role | Drops into the peer without SSH | `vox-dashboard` + new `vox-mesh-shell-bridge` | L |
| 16 | **"Drag-to-assign-role"** on the topology canvas | Direct manipulation beats menus | `vox-dashboard` + orchestrator affinity API | M |
| 17 | **Run-row drawer with full event tree + trace_id deep-link** | Wires `vox.mesh.trace_id` through the UI | `vox-dashboard` + `vox-db` `llm_interactions.trace_id` | S |
| 18 | **Node-health probe inline sparklines** (CPU/RAM/GPU VRAM/queue) | Quick triage; populi probes already emit them | `vox-populi` + `vox-dashboard` | S |

**Anti-features.** Force-graph that re-layouts every event; toast-confetti per run; YAML-as-the-primary-editor; right-click context menus as the only path to actions; topology that pretends to be physically accurate; mesh-peer chat (becomes Slack); fan-out HTTP polling instead of using the WS bus.

**Five things the dashboard should NOT do.** Become a code editor. Gate or replace the CLI (every action must have a `vox …` equivalent and the dashboard should display it). Require login on `127.0.0.1`. Run as a hosted multi-tenant SaaS. Become an end-user chat product.

### 2.3 Language-level features

Top-7 ranked by ROI (impact ÷ effort) for a Vox→mesh→durable pipeline:

| Rank | Feature | Why now |
|---|---|---|
| 1 | **Workflow determinism check** (Phase 5 Task 3 brought to error-not-warning) | Turns ADR-019 from advisory to law; one rule, one diagnostic; extends existing `effect_check.rs` |
| 2 | **Auto-derived `activity_id`** = `blake3(workflow_id ‖ call-site-span ‖ structural-arg-hash ‖ replay-counter)` + `@with_id` override | Removes manual idempotency keys — the #1 distributed footgun |
| 3 | **`@remote` annotation + serializable trait bound** | Distribution becomes a typed concept; replaces `mesh_*` naming convention |
| 4 | **`DurablePromise[T]` as the single awaitable primitive** (Restate-shape) | One canonical shape (C4 in [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md)); kills `Future`/`Promise`/`Activity` divergence |
| 5 | **Content-addressed workflow versioning** via existing `vox-package` | Eliminates version-skew bug class structurally; almost-free given existing CAS |
| 6 | **`par_map` / `pipeline` / `event_sourced` syntax** (desugars over existing primitives) | Three high-traffic patterns become one-liners; reduces decision count for LLM authors |
| 7 | **`vox workflow preview <fn>(args)`** — type-check + effect-infer + dry-run journal projector | Converts runtime questions to compile-time; LLM can self-verify before dispatch |

**Notable omissions** (lower ROI right now): supervision-tree restart strategies (port from OTP); typed `Mailbox[T]`; `signal name within timeout` sugar; CRDT integration for replicated state.

---

## 3. Security & trust for an open volunteer mesh

### 3.1 Threat model

The mesh's collapse condition today: **any peer with `VOX_MESH_TOKEN` is fully trusted.** Acceptable for power-user-LAN (current ambition per [populi-mesh-north-star-2026.md §6.2](populi-mesh-north-star-2026.md)); unsafe for anything wider.

| Adversary | Capability today | Component impacted |
|---|---|---|
| Malicious worker | Forge results; steal in-flight prompts; sell donated time; lie about hardware (operator labels override probes); tasks run in-process → full RCE on submitter side if exfil | `vox-orchestrator/src/a2a/dispatch/mesh.rs`, `executor`, no sandbox |
| Malicious submitter | Send malware-shaped TaskKinds; exhaust GPU memory; ship VoxScript with exfil; spam expensive jobs (no per-key quota) | `vox-orchestrator/src/a2a/inbox`, `vox-skills`, `vox-wasm-engine` |
| Malicious mesh peer | Replay old A2A envelopes; forge HS256-signed messages (symmetric secret); squat lease IDs | `vox-populi/src/transport/handlers.rs`, JWT auth |
| Sybil attacker | Spin up N fake nodes; vote-stuff redundancy checks; harvest credit/reputation; flood inbox | `vox-mesh-types::NodeRecord`, future trust ledger |
| Network attacker | Bare HTTP → passive read of prompts/results/headers; active MITM swaps results | All of `vox-populi::transport`, no TLS |
| Byzantine collator (post-leases) | Lease holder withholds completion to grief originator; gateway selectively drops dispatches | `vox-orchestrator/src/a2a/dispatch/`, ADR-017 lease path |

### 3.2 Lessons from prior art

- **BOINC** — Steal: redundant execution + majority voting; *homogeneous redundancy* classes for floating-point determinism; *adaptive replication* (only re-verify untrusted hosts) cuts overhead. Avoid: hand-tuned per-project credit economy; weak Sybil resistance. ([Anderson 2019 paper](https://arxiv.org/pdf/1903.01699), [BOINC HR wiki](https://github.com/BOINC/boinc/wiki/Homogeneous-Redundancy))
- **Folding@home** — Steal: server-signed work units; validate returned WUs against expected energy/checksum bounds. Avoid: top-down trust; closed core.
- **Akash / Bacalhau / Golem** — Steal: Akash 2026's Kata-Containers micro-VMs + composite CPU/GPU TEE attestation as the eventual sandbox tier. Avoid: blockchain payment rails — wrong cost curve for power-user dogfood. ([Akash roadmap 2026](https://akash.network/roadmap/2026/), [Akash TEE discussion](https://github.com/orgs/akash-network/discussions/872))
- **Tor / I2P** — Steal: assume a hostile network. Avoid: onion routing — wrong threat model (we want auditable identity, not anonymity); latency cost is fatal.
- **Tailscale + Headscale** — Steal: identity-bound overlay; control-plane / data-plane separation; WireGuard transport; MagicDNS-like simple naming. Avoid: SaaS control plane lock-in.
- **Nostr / ATProto** — Steal: identity = keypair; signatures travel with messages; ATProto's `mutable handle + immutable DID` pattern. Avoid: relay-as-only-Sybil-defense.
- **libp2p / Iroh / Hyperswarm** — Steal: PeerID = BLAKE3(Ed25519 pubkey); pubkey-based dialing through NAT; bounded Circuit Relay V2. Avoid: full DHT-as-SSOT (ADR-020 settles this — HTTP populi remains SSOT).

### 3.3 Recommended architecture

**Layered identity** (extends existing [`vox-identity`](../../../crates/vox-identity/) + Ludus design):

1. **Long-term Ed25519 node identity** (already present). PeerID = BLAKE3(pubkey). Persist; never rotate without explicit revocation.
2. **GitHub OAuth attestation** for Sybil resistance (per [ludus-identity-github-integration-research-2026.md](ludus-identity-github-integration-research-2026.md)). Bind `node_pubkey ↔ github_numeric_id` via signed self-attestation hosted as a verifiable gist/repo file. DevRank multipliers ([ludus-security-and-anti-cheat-research-2026.md](ludus-security-and-anti-cheat-research-2026.md)) gate trust tiers.
3. **Per-pairing X25519 keys** (W3, already speced).
4. **Per-job ephemeral Ed25519 subkey** signed by long-term node key, scoped to single `task_id`, lifetime = lease TTL.

**Hybrid trust topology** (not pure web-of-trust, not central registry):

- **Local pairing graph** is SSOT for "who can I dispatch to."
- **Optional public attestation registry** = a signed JSON manifest in a known git repo; mirrors ATProto DID-doc.
- **Trust ledger** lives in vox-db on each node; replicated by gossip only between paired peers — *no global consensus*.

**Revocation:** short-lived attestations (≤ 30 days) + manual `vox populi revoke <peer-id>` propagating as a tombstone in the pairing-store. Refuses dispatch even if cert hasn't expired. Deliberately not OCSP-shaped infra.

### 3.4 Sandbox escalation ladder

Vox already has three executors at increasing strength (in-process Rust → WASM via [`vox-wasm-engine`](../../../crates/vox-wasm-engine/) → containers via [`vox-container`](../../../crates/vox-container/)). Add a fourth tier (micro-VM via firecracker/kata) for the "stranger" scenario.

| TaskKind | Tier | Reason |
|---|---|---|
| **VoxScript** | T2 WASM (default), T3 container if file/net I/O declared | Vox lang has capability annotations; WASM enforces fuel + memory cheaply |
| **Embed** | T1 in-process | Pure compute on trusted model binary; untrusted *input* only |
| **TextInfer** | T1 trusted models, T3 container for arbitrary HF checkpoints | `trust_remote_code=True` makes weights code-equivalent |
| **ImageGen / SpeechTranscribe** | T3 container | Same model-as-code risk; large weights amortize container cost |
| **TrainQLoRA** | T3 container with GPU passthrough | Long-running, writes checkpoints, easy to abuse for crypto-mining |
| **Anything from a non-paired peer** | T4 micro-VM (kata/firecracker) | Same posture Akash 2026 lands on. Pre-wire the trait; ship later |

**Hard rule:** the orchestrator process itself never executes untrusted task payloads. Move the in-process executor behind the existing [`vox-skill-runtime`](../../../crates/vox-skill-runtime/) trait so the boundary is consistent.

### 3.5 Result attestation & verification

| Approach | Cost | Determinism needed | Right for |
|---|---|---|---|
| Redundant exec + majority voting (BOINC) | 2–3× compute | Yes — homogeneous-redundancy lesson: vote only across "numerically equivalent" hosts | TrainQLoRA quality gates, deterministic Embed, batch ImageGen. *Not* sampled TextInfer |
| TEE attestation (SGX / SEV-SNP / H100 confidential compute) | Hardware-tied; Akash-style is real but heavyweight | No | Confidential inference where worker mustn't read the prompt. Defer; no H100s in a personal mesh |
| Signed deterministic replay (proof-of-effort) | 1× compute + signed trace | Yes for replay | VoxScript, Embed, batch jobs. Worker signs `(task_id, input_hash, output_hash, gpu_seconds, blake3 of intermediate states)` with per-job ephemeral key; submitter spot-checks ~1% by re-running |

Mapping per TaskKind: VoxScript / Embed → signed deterministic replay (1×). TextInfer (greedy / temp=0) → signed replay; sampled → trust + spot-redundancy on `seed` matching. TrainQLoRA → redundant exec on 2 nodes for the *first epoch* (shape, loss-curve match), trust thereafter. ImageGen / SpeechTranscribe → 1–5% submitter-side replay sample.

### 3.6 Critical path to safe-to-deploy-publicly

Ordered by what unblocks what:

1. **TLS or WireGuard sidecar** on the populi HTTP plane. Closes passive eavesdropping; prerequisite for everything else.
2. **Replace JWT-HS256 with Ed25519-signed envelope** (worker-signature handler path that already exists in `vox-identity` but is unused). Fixes "any token-holder can forge."
3. **Land W3 pairing decrypt** so JWE round-trip stops being theatre.
4. **Per-node sandbox boundary** — move in-process executor behind `SkillRuntime`, default to container for non-vetted TaskKinds.
5. **Per-key quota + reputation persistence** — abuse fuse before opening to strangers.
6. **GitHub-attestation gate at pairing time**.
7. **Result attestation (signed deterministic replay) on the worker, with submitter spot-check sampler.**
8. *(Only after 1–7)* TEE path for confidential inference; redundant-execution voting for batch jobs.

Steps 1–4 take Vox from LAN-only → vetted-public-peers. Steps 5–7 from vetted → open-with-bounded-blast-radius. Step 8 is the long tail.

### 3.7 Five things to explicitly NOT build

1. **Custom crypto.** No new AEAD, no novel signature scheme, no homegrown ZK. Stick to [`vox-crypto`](../../../crates/vox-crypto/) per [cryptography-ssot-2026.md](cryptography-ssot-2026.md).
2. **Blockchain / token economy.** Cost curve wrong for personal mesh; introduces governance burden.
3. **TEE-first architecture.** SGX deprecated on consumer Intel; SEV-SNP server-only; H100 CC real but no consumer access. Build the *interface*; stub the implementation.
4. **Onion routing / Tor-style anonymity.** Wrong threat model; we want auditable identity, not unlinkability.
5. **Web-of-trust transitive scoring.** Math is right, UX is fatal. Use paired peers + GitHub attestation as binary gates; reputation as a *signal*, not a transitive *capability*.

---

## 4. The grand VOX network — what makes it tractable

### 4.1 Language-level distributable durability

The user's load-bearing ask: durable Vox workflows distribute across the mesh **without the usual footguns** (non-determinism, retries, partial failure, idempotency, version skew, leaked secrets), especially for AI-managed code where an LLM is the author.

#### 4.1.1 Effects-as-types: minimum credible enforcement

Vox already does subset-checked propagation in [`effect_check.rs`](../../../crates/vox-compiler/src/typeck/effect_check.rs). Smallest credible step: **close the holes**, not redesign.

- **Bottom-up inference over the call graph.** A function's inferred set = union of body's builtin effects ∪ callees' inferred sets. Phase 5 Task 1.
- **Concrete row, not row-polymorphic for v1:** `Net | Fs(read: GlobSet, write: GlobSet) | Time | Random | Secret | Spawn | Mcp(name) | Mailbox(actor_ty)`.
- **`workflow` body restriction:** inferred set ⊆ `{Mailbox(_), Spawn(activity)}`. `Time | Random | Net` rejected unless wrapped in an `activity` callee.
- **Error-message shape (Koka-style symmetric pairs):**
  ```
  vox/effect/missing-net-decl: function `fetch_quote` calls `std.http.get`
    requiring `net`, but is declared `@uses(fs)`.
    └─ called via: fetch_quote → resolve_url → std.http.get  [span]
    suggested fix: replace `@uses(fs)` with `@uses(fs, net)`.
  ```

#### 4.1.2 Determinism boundary

**Forbidden inside `workflow` body:** `time.now`, `time.sleep`, `clock.*`, `random.*`, `std.http.*`, `db.*`, `fs.*`, `env.*`, calls to non-activity/non-workflow fns with effect ≠ Mailbox, direct mailbox `recv()` from arbitrary actors (signals only).

**Allowed:** pure computation, `await activity_call(...)`, `await child_workflow(...)`, `await signal(name) within timeout`, `select { ... }` over awaitables.

**Escape hatch:** `side_effect { expr }` desugars to an auto-generated single-shot activity whose `activity_id` is `(call-site span hash, replay-counter)`. Temporal `SideEffect` semantics — the *only* sanctioned non-determinism inside a workflow.

#### 4.1.3 Distribution as a typed concept

Three options, scored against ergonomics, type safety, and effect interaction:

| Option | Verdict |
|---|---|
| `@remote fn foo()` annotation (Ray-shape) | **Default winner.** Adds `Spawn` and `Net` to the fn's effect row. Familiar to LLMs. Return type = `DurablePromise[T]`. |
| `Worker` value + `worker.spawn(fn, args)` (Erlang-shape) | Keep as power-user escape. Excellent type safety; verbose at call site. |
| Runtime-only (today) | **Reject.** Loses every compile-time guarantee. The bug class "I called a function that *will* be remote and broke at deploy" is structurally preventable with an annotation. |

#### 4.1.4 Idempotency primitives

Borrow Restate's "durable promises": every awaitable side-effecting op gets an *ID-bearing journal entry*; replay finds the entry and returns its value rather than re-running.

- Auto-derived `activity_id` = `blake3(workflow_id ‖ call_site_span_hash ‖ structural_arg_hash ‖ replay_counter)`. Stable across renames; broken by code motion (acceptable — see versioning §4.1.6).
- `@with_id(expr)` override for business identity (`order_id`, `payment_intent_id`).
- `@activity(dedup = "7d")` window.
- Compile-time warning `vox/activity/nondeterministic-id-args` if effect inference shows args contain `time.now()` / `random.*`.
- Activity calls return `DurablePromise[T]` not `Future[T]` — unifies awakeable / side_effect / activity into one primitive.

#### 4.1.5 Three durable parallel patterns as one-liners (sketches)

```vox
// vox:skip
// Map-reduce — fan out N independent activities, await all, fold
let receipts = orders |> par_map fn(o) { @activity charge(o) }
                     |> fold(Receipt::empty, Receipt::merge)
```

```vox
// vox:skip
// Pipeline with backpressure — stage1 → stage2 → stage3
pipeline {
    source:  read_orders()                  // stream emit
    stage:   validate     buffered(64)      // bounded channel
    stage:   @activity charge buffered(16)  // each is journaled
    sink:    @activity persist
}
```

```vox
// vox:skip
// Event sourcing — append-only log + projection
event_sourced actor InventoryStream {
    state: Inventory = Inventory::empty()
    on Restock(sku, qty)  => state.add(sku, qty)
    on Reserve(sku, qty)  => state.try_take(sku, qty)
    projection daily = state |> group_by_day  // auto-materialised view
}
```

Each desugars to existing primitives — no new keywords beyond `pipeline` and `event_sourced`.

#### 4.1.6 Versioning & code mobility

Three options ranked for Vox specifically:

1. **Content-addressed code (Unison-shape) — strongly recommended.** [`vox-package`](../../../crates/vox-package/) already has CAS; compiler emits stable `@generated-hash` headers (Phase 1 Task 9). A workflow run is `(workflow_fn_hash, args_hash)`. Replay loads code by hash; the wrong-version problem is *structurally absent*. New versions have different hashes — they coexist until drained.
2. **Patch markers** (`workflow.version("change-1", 0, 2)`). Necessary escape hatch when the author legitimately needs to alter logic for in-flight runs (Temporal proves this). Lint nudges toward content-addressed deploys.
3. **Submit-time pinning.** Useful for short workflows; deployment policy, not language feature.

Operational tool: `vox workflow drain --version <hash>` retires old code.

#### 4.1.7 AI-agent ergonomics — features that reduce LLM footguns

- **Stable diagnostic IDs in `vox/<category>/<kebab>` namespace** (Phase 2). Models trained on 0.5 still recognize 0.7 errors.
- **`vox check --for-llm` mode** emitting minimal repro per diagnostic.
- **`vox workflow preview <fn>(args)`** — type-check + effect-infer + dry-run journal projector; the schedule of activities that *would* run, without doing them. Highest-value LLM tool because it converts "does this distributed program do what I think" from runtime to compile-time.
- **Stable `activity_id` across refactors** — auto-derive from span+arg-hash; LLMs that rename functions don't accidentally invalidate dedup history.
- **Required `@uses(...)` on all `pub fn`** — effect annotations are *prompt scaffolding the model already uses*; making them mandatory means LLM-generated code carries its own audit trail.
- **Symmetric error pairs** (`missing-X` ↔ `unjustified-X`): LLMs learn the inverse rule for free.
- **One canonical shape per concept** (C4 in [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md)): no `Future[T]` *and* `Promise[T]` *and* `DurablePromise[T]` — pick one, kill the rest.
- **Distinctive surface keywords** for distributed concepts (P2): `workflow` not `async fn`, `activity` not `@retry`, `mailbox` not `chan` — resists drift to JavaScript/Python idioms.

### 4.2 Multi-agent collaboration via mesh — what it takes

#### 4.2.1 Distributed lock arbitration

| Architecture | 2–10 nodes | 100+ nodes | Vox fit |
|---|---|---|---|
| Single coordinator-d (today) | Works with hot-standby + WAL replication. Ceiling ≈ low thousands of locks/s | Bottleneck; SPoF unless elected | **Phase-1 winner.** Promote one daemon as `lock-leader`; others forward via existing A2A envelope |
| Lease-based Raft (etcd-shape) | 3-/5-node quorum; tolerates `(N-1)/2` failures | Native habitat | **Phase-3 target.** Pair with capability-token-mint Raft group |
| Optimistic + retry (CRDT) | Best when contention is rare; vector-clock LWW per file | Scales as long as conflicts stay sparse | **Already the model for non-overlapping byte-range edits.** Keep for affinity hints; *not* for hard locks |
| Token-passing ring (Erlang) | Latency = ring × node count; bad failure handling | Falls over | Skip |

**Lightest 2–10 node design:** single coordinator-d with hot-standby + heartbeat-promoted leader. Existing in-memory `Exclusive`/`SharedRead` map becomes the durable Raft state-machine value when scaling to 100+; Phase-1 needs only durable WAL of lock grants so a leader crash doesn't lose a held lock.

#### 4.2.2 Op-log gossip & replication

Given `predecessor_hash` already gives causal ordering:

- **Gossip pattern.** Push-rumor-mongering for fresh ops + periodic anti-entropy (Demers et al. epidemic algorithms). Each peer maintains a Bloom filter / Merkle tree of known op-IDs; sync neighbors on a 30 s tick. Hot path O(new ops); catch-up O(log N × repo size). The `OpFragmentEnvelope` in [multi-agent-vcs-replication-spec-2026.md](multi-agent-vcs-replication-spec-2026.md) §Wire-protocol is already shaped for this.
- **Unknown-parent fragment.** Hold + request backfill (spec's stated behavior). `parking-lot` keyed by missing parent op-ID; on inbound op insert, drain dependents. Bound the holding queue (1024 entries × 64 KiB) and DLQ overflow to a `vox-db` table for human triage.
- **Retention.** Tiered: hot (last 10 K ops or 7 days) in `VecDeque`, warm in `vox-db` indexed by `(convergence_set, op_id, parent_op_ids)`, cold compacts by collapsing snapshot ops older than the latest coherent base into a single `Checkpoint` op (jj-shape squash). Prune when no live `AgentChange` references.
- **Durability home.** New table `convergence_op_log(op_id PK, set_id, parent_op_ids JSON, payload BLOB, signature, agent_id, produced_at)` plus `(set_id, produced_at)` index in [`vox-db`](../../../crates/vox-db/). Same SQLite file as the capability ledger; separate tables.

#### 4.2.3 File-affinity replication

Today affinity is per-daemon. To go mesh-wide:

- LWW per file → cheap, double-claim race for ~RTT. OK for *hint* affinity; not authoritative.
- Vector-clock per file → right shape; widen `(path → daemon_id)` to `(path → (daemon_id, lamport))`. Still needs anti-entropy gossip.
- Quorum via consensus → strong but pays Raft latency on every file-write decision.

**Recommendation:** vector-clock LWW for *hint*; ExclusiveLock (single-coordinator §4.2.1) for the hard guarantee. Affinity = soft routing; lock token = hard contract.

#### 4.2.4 Conflict tiering

Four-tier funnel ordered by latency:

1. **Patch-commutativity.** Today's `MergePolicyV1` byte-range overlap. Auto-merges the easy ~70%.
2. **Lock-first.** Hot paths (e.g., `Cargo.toml`, schema files) → `Exclusive`. Queue policy = FIFO with model-tier weighting (higher-trust tie-breaker). Max wait = 60 s soft → re-plan or requeue.
3. **Three-way merge with LLM arbiter.** "Socrates arbitration" (spec §Phase 4) — two semantically related edits; arbiter scores hallucination + author trust; if Δ > threshold pick a side, else fall through.
4. **Manual escalation.** `vox vcs conflicts list` materializes navigable artifact (spec Phase 2); human ack required for path-policy class (e.g., [`vox-secrets/src/spec/mod.rs`](../../../crates/vox-secrets/src/spec/mod.rs)).

Add a `LockWait` outcome between AutoMerge and SurfaceConflict so tier-2 doesn't masquerade as tier-3.

#### 4.2.5 Branch & PR coordination

- **Per-task branches**, not per-agent. The agentic-vcs research already binds branch to `AgentWorkspace`; per-agent branches conflate identity with scope.
- **Push timing.** When the *task's* convergence set marks the change ready (tests pass, secret-scan clean, `vox_pr_open` precondition). Don't push WIP across the mesh — keep them in the convergence set. `vox_push` is Phase-2 capability per the agentic plan.
- **Convergence vs. N-PR.** The `ConvergenceSet` *is* the answer. Shared set → one PR; separate sets → separate PRs. Set membership = explicit user/orchestrator action.
- **Cross-node branch discovery.** Reuse `ConvergenceSetAnnouncement` envelope. Each daemon advertises sets it owns; orchestrator joins listen + merge into a global set-registry. No forge round-trip for in-flight work.

#### 4.2.6 Capability-token federation

Three options for the Phase-1 → Phase-4 ramp:

- **Mint locally + sign + validate-on-remote.** Each daemon has a vox-secrets-issued keypair; tokens carry an Ed25519 signature; remote daemons verify against the federation's root-of-trust. **Lowest coordination cost; cleanest evolution.** Matches Phase 4 of the agentic plan ("agent identities for op signing").
- **Mint via coordinator only.** Forces every BranchCreate through the lock-leader. Tightly couples mint to lock-server availability — bad for partitioned operation.
- **Per-task ephemeral tokens scoped to a single mesh dispatch.** MENS dispatch envelope already has `idempotency_key`; bind a capability set to the dispatch and let it expire on ack/fail. Excellent posture; layers on top of either of the above.

#### 4.2.7 Code mobility

| Need | Mechanism | Latency |
|---|---|---|
| Worker spawning, near-current snapshot | `vox-package` content-addressed bundle (already a primitive). Daemon ships CAS bundle of (working tree + recent op-log tail) | One round-trip; tens of MB |
| Long-running worker catch-up | Stream patches via mesh op-log gossip | Continuous; smallest payloads |
| Cold start / fallback | Pull from upstream forge ([`vox-forge`](../../../crates/vox-forge/)) with explicit `git fetch` | Slowest; bypasses mesh trust |

Minimum-staleness combination: ship CAS bundle as seed (single round-trip), then subscribe to op-log gossip for the relevant convergence set. Forge pull only as backstop when the bundle's base is older than the forge's main.

#### 4.2.8 Smallest credible MVP — two daemons, multi-agent, same repo, no data loss

1. **Persist the lock map.** Move the in-memory map into `vox-db` table `vcs_lock(path, kind, holder, expires_at, lease_id)`; WAL replay on daemon start.
2. **Single lock-leader election.** `lock_leader` row in same DB with heartbeat; loser proxies via existing A2A envelope. Worktree-creation and branch-name minting flow through. Coordination overhead = one A2A round-trip per lock op.
3. **Sign every capability mint and op-fragment** with vox-secrets keys; `verify_signature` on the receiver. Extends existing JWE envelope.
4. **Op-log durability + bounded gossip topic.** Persist `OpFragment` to `convergence_op_log`; add the `OpFragment` topic to A2A dispatch with Bloom-filter anti-entropy every 30 s. MVP doesn't need backfill ladders or epidemic-push tuning.
5. **Code-mobility seed via vox-package bundle.** Daemon B accepts a job from A → A ships CAS bundle (working tree + last 100 ops) → B subscribes to op-log topic. No forge involvement.

Steps 1–2 alone close the highest-likelihood-highest-severity risk (silent two-daemon lock race) and are the **first credible "two daemons, no data loss" milestone**. Steps 3–5 widen safety/freshness.

#### 4.2.9 Top-5 risks for "100 agents, 10 nodes, one repo"

| # | Risk | L | S | Smallest fix |
|---|---|---|---|---|
| 1 | Two daemons race the same file lock — wrong-branch commits stack | High | High | Persist locks to `vox-db` + elect single leader before any second daemon comes online |
| 2 | Op-fragment with unknown parent stalls forever (producer offline) | Med | High | Bound wait queue (1024 / 64 KiB), DLQ overflow to `vox-db`, surface in dashboard |
| 3 | Capability token forged on a remote node (mint paths still `pub`) | Med | Very High | Sign every minted token with the local daemon's vox-secrets key when a second daemon joins (Phase-3 brought forward) |
| 4 | Local op-log diverges from gossip → agent commits a force-push-ish op via banned-list bypass | Low | Very High | Move banned-list check from `git_exec` to a `vox-bounded-fs` process-spawn deny-list (already in agentic-vcs Phase 4 plan) |
| 5 | Affinity LWW oscillation — two daemons keep stealing the same file | Med | Med | Hold-down timer (60 s) + vector-clock comparison so the older claim wins ties |

---

## 5. Open questions / Wave-3 research

These weren't answerable from the codebase alone and need either prototyping or another round of design:

1. **Sandbox for VoxScript with mesh-effects.** WASM-based capability propagation is clean for in-process; what's the right shape when the WASM module dispatches a remote activity? Does the ephemeral per-job key live in the host's heap or the guest's?
2. **Workflow signal typing.** Restate's awakeables are stringly-typed externally. Should Vox's `signal name within timeout` carry a `Signal[T]` type or stay string-keyed for cross-language interop?
3. **Durable promise shape vs. existing `Future[T]`.** Single canonical primitive (C4) demands removing `Future[T]` from std — what's the compat story for non-distributed callers?
4. **Per-region attestation revocation latency.** Tombstones via gossip — what's the worst-case revocation propagation time? Acceptable bound for personal-mesh; need numbers for public-internet.
5. **Lock-leader split-brain on partition heal.** When two leaders briefly co-exist (network partition), how do their op-logs reconcile? Same answer as op-fragment unknown-parent, or is there a leader-specific case?
6. **Dashboard-as-orchestrator-extension.** If the dashboard surfaces "drag-to-assign-role" with capability-token semantics, does the mint API need to gain a `via_dashboard: bool` audit field, or does the existing trailer chain suffice?
7. **Mesh-aware `vox audit` umbrella.** [tooling-convergence-findings-2026.md](tooling-convergence-findings-2026.md) proposes a `vox audit` umbrella; should mesh-health (probe correctness, lock backlog, oplog lag, attestation freshness) join the same surface, or stay in a separate `vox populi audit`?
8. **Federation envelope shape.** Should the op-fragment envelope adopt ForgeFed/ActivityPub's Activity-object pattern for forge-event federation, or stay pure-binary? ActivityPub is too verbose as transport; the *shape* may still be useful.

---

## 6. Sources

External (Wave-2 prior-art research):

- [BOINC: A Platform for Volunteer Computing (Anderson, 2019)](https://arxiv.org/pdf/1903.01699) · [BOINC HR wiki](https://github.com/BOINC/boinc/wiki/Homogeneous-Redundancy)
- [Akash Network 2026 Roadmap](https://akash.network/roadmap/2026/) · [Akash TEE discussion](https://github.com/orgs/akash-network/discussions/872)
- [Why Tailscale](https://tailscale.com/why-tailscale) · [Tailscale Auth Keys](https://tailscale.com/docs/features/access-control/auth-keys)
- [Nostr (Wikipedia)](https://en.wikipedia.org/wiki/Nostr) · [AT Protocol (Wikipedia)](https://en.wikipedia.org/wiki/AT_Protocol)
- [P2P Networking: WebRTC vs libp2p vs Iroh](https://medium.com/@ark-builders/the-deceptive-complexity-of-p2p-connections-and-the-solution-we-found-d2b5cbeddbaf)
- [SoK: Analysis of Accelerator TEE Designs (NDSS '26)](https://cse.sustech.edu.cn/faculty/~zhangfw/paper/sok-xputee-ndss26.pdf)
- [Acurast: Decentralized Serverless Cloud (2026)](https://arxiv.org/html/2503.15654v2)
- [Confidential Computing Blueprint for AI Workloads](https://medium.com/@naeemulhaq/implementing-confidential-computing-a-technical-blueprint-for-securing-ai-workloads-with-hardware-f87e5338d62f)
- [Coolify](https://coolify.io/docs/) · [Ray Summit 2025](https://www.anyscale.com/blog/ray-summit-2025-anyscale-product-updates) · [Ray Clusters](https://docs.ray.io/en/latest/cluster/getting-started.html)
- [Grafana Loki OSS](https://grafana.com/oss/loki/) · [Loki Live Tailing](https://grafana.com/blog/2019/08/13/lokis-path-to-ga-live-tailing/)
- [k9scli.io](https://k9scli.io/) · [Kubernetes Dashboard vs Lens vs k9s](https://puffersoft.com/kubernetes-dashboard-vs-lens-vs-k9s-which-one-should-you-choose-in-2025/)
- [Slurm-web](https://slurm-web.com/) · [HuggingFace Inference Endpoints](https://endpoints.huggingface.co/)
- [Temporal Workflow Replay Debugger](https://temporal.io/code-exchange/temporal-workflow-replay-debugger) · [Temporal Events & History](https://docs.temporal.io/workflow-execution/event)
- [Pijul Theory](https://pijul.org/manual/theory.html) · [Jujutsu](https://github.com/jj-vcs/jj) · [GitButler Workspace Branch](https://docs.gitbutler.com/workspace-branch)
- [Automerge](https://github.com/automerge/automerge) · [Yjs](https://docs.yjs.dev/) · [Replicache Consistency Model](https://doc.replicache.dev/concepts/consistency)
- [Fossil Sync Protocol](https://www.fossil-scm.org/home/doc/trunk/www/sync.wiki) · [Plastic SCM (Unity Version Control)](https://unity.com/products/plastic-scm)
- [etcd Raft library](https://github.com/etcd-io/raft) · [Raft consensus](https://raft.github.io/)
- [ForgeFed protocol](https://forgefed.org/) · [Forgejo FAQ](https://forgejo.org/faq/)
- [Demers et al.: Epidemic algorithms for replicated database maintenance](https://www.cis.upenn.edu/~bcpierce/courses/dd/papers/demers-epidemic.pdf)

Internal (cross-references):

- Mesh: [populi-mesh-north-star-2026.md](populi-mesh-north-star-2026.md), [-improvement-backlog](populi-mesh-improvement-backlog-2026.md), [-a2a-durability-spec](populi-mesh-a2a-durability-spec-2026.md), [-config-baseline-spec](populi-mesh-config-baseline-spec-2026.md), [-local-observability-spec](populi-mesh-local-observability-spec-2026.md), [-probe-correctness-spec](populi-mesh-probe-correctness-spec-2026.md), [-probe-correctness-plan](populi-mesh-probe-correctness-plan-2026.md), [scientia-mesh-integration-research](scientia-mesh-integration-research-2026.md).
- Dashboard: [vox-dashboard-design-brief-2026.md](vox-dashboard-design-brief-2026.md), [dashboard-migration-research-2026.md](dashboard-migration-research-2026.md), [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md), [ffscript-mutation-api-spec-2026.md](ffscript-mutation-api-spec-2026.md) (FableForge, not dashboard panels).
- Durability / language: [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md), [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md), [vox-language-rules-phase1-ssot-collapse-2026.md](vox-language-rules-phase1-ssot-collapse-2026.md), [feature-growth-boundaries.md](feature-growth-boundaries.md), [v0.5-core-ssot.md](v0.5-core-ssot.md), [nextgen-orchestrator-research-2026.md](nextgen-orchestrator-research-2026.md).
- Multi-agent VCS: [multi-agent-vcs-replication-research-2026.md](multi-agent-vcs-replication-research-2026.md), [-spec-2026.md](multi-agent-vcs-replication-spec-2026.md), [-impl-plan-phase1](multi-agent-vcs-replication-impl-plan-phase1-2026.md), [agentic-version-control-automation-research-2026.md](agentic-version-control-automation-research-2026.md), [agentic-vcs-automation-impl-plan-phase1](agentic-vcs-automation-impl-plan-phase1-2026.md), [git-concurrency-policy.md](git-concurrency-policy.md).
- Security / identity: [cryptography-ssot-2026.md](cryptography-ssot-2026.md), [share-policy-2026.md](share-policy-2026.md), [ludus-identity-github-integration-research-2026.md](ludus-identity-github-integration-research-2026.md), [ludus-security-and-anti-cheat-research-2026.md](ludus-security-and-anti-cheat-research-2026.md), [telemetry-trust-ssot.md](telemetry-trust-ssot.md).
- Tooling / architecture: [where-things-live.md](where-things-live.md), [layers.toml](layers.toml), [tooling-convergence-findings-2026.md](tooling-convergence-findings-2026.md), [phase-numbering-index.md](phase-numbering-index.md), [v1-release-criteria.md](v1-release-criteria.md).
