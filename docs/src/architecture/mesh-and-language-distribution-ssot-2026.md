---
title: "Mesh & Language-Level Distribution — SSOT & Upgrade Plan (2026-05-09)"
description: "Authoritative single-source-of-truth and seven-phase upgrade plan for the Vox mesh + the language-level features that make distributed durable workflows safe to author, dispatch, and replay across it. Defines the canonical mental model (DurablePromise, effect rows, content-addressed code, signed op-fragments, capability tokens), audits the codebase against the model, sequences the work into Phase 0–6 with concrete files / contracts / acceptance per phase, and pins three release contracts (v0.6 single-machine multi-agent, v0.7 two-daemon LAN mesh, v1.0 internet-facing personal mesh) plus a v1.x grand-network direction. Subsumes prior research; supersedes scattered intent."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Plan-of-record for the load-bearing mesh + language convergence; agents and contributors should orient from this doc before changes that cross mesh, populi, orchestrator, workflow runtime, or compiler boundaries."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-populi: control plane, A2A inbox, registry, mens, hardware probes"
  - "vox-mesh-types: shared transport types"
  - "vox-plugin-populi-mesh: cdylib transport plugin"
  - "vox-orchestrator: dispatch, file-affinity, capability tokens, oplog"
  - "vox-orchestrator-queue: locks, oplog, affinity"
  - "vox-orchestrator-types: VCS capability tokens"
  - "vox-orchestrator-d: orchestrator daemon binary"
  - "vox-workflow-runtime: durable workflow MVP"
  - "vox-actor-runtime: actors, mailboxes, supervision"
  - "vox-compiler / vox-codegen: workflow / activity / actor / @remote"
  - "vox-package: content-addressed code store"
  - "vox-identity / vox-crypto: Ed25519 / X25519 / JWE / BLAKE3"
  - "vox-skill-runtime: sandbox boundary trait"
  - "vox-wasm-engine / vox-container: existing sandbox tiers"
  - "vox-secrets: signing keys, secret resolver"
  - "vox-dashboard: mesh-control surfaces"
  - "vox-arch-check: layer + forbidden-deps enforcement"
  - "vox-db: durable substrate for locks, oplog, attestations, kudos"
  - "vox-telemetry: vox.mesh.* / vox.workflow.* / vox.vcs.* spans"
  - "vox-forge / vox-git: forge interop, git bridge"
---

# Mesh & Language-Level Distribution — SSOT & Upgrade Plan

> ### Council ratification 2026-05-15 — mesh demoted from v1.0 to v1.1
>
> Per council decision D16 in
> [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md)
> §8 ratification log:
>
> - **v1.0 acceptance contract no longer includes mesh Phase 2 LAN.**
>   The release contract previously named "v1.0 internet-facing personal mesh"
>   is **demoted to v1.1**. v1.0 ships as single-machine + the LLM-target story
>   ([`v1-release-criteria.md`](v1-release-criteria.md) §5 CR-L0..CR-L8).
> - **v0.6 (single-machine multi-agent) and v0.7 (two-daemon LAN mesh) targets
>   remain unchanged.** Phase 0–1 work proceeds as scheduled; Phase 2–6 work
>   continues but is not on the v1.0 critical path.
> - Rationale: shipping both mesh and LLM-target stories in one release
>   weakens both — they compete for code budget, attention, and field-testing
>   time. One coherent story per release ages better.
>
> Sections below referring to "v1.0 = internet-facing personal mesh" should be
> read as "v1.1 = internet-facing personal mesh" until the body of this SSOT
> is rewritten in full. This banner is normative; inline body references are
> not retroactively edited to avoid invalidating phase-plan cross-references.

> **What this is.** The single source of truth for how the Vox mesh and the
> Vox language converge into a runtime where durable, distributable workflows
> are a one-liner — for human and AI authors alike. It pins the canonical
> mental model, audits the codebase against it, sequences the work, and
> nails down acceptance per release.
>
> **What this is not.** Detailed per-phase implementation plans (those decompose
> downstream — see §10 spawn list). Speculative research (that's
> [mesh-dashboard-and-distributed-compute-research-2026.md](mesh-dashboard-and-distributed-compute-research-2026.md)).
> A re-litigation of decided ADRs.

## 0. Charter

**Scope (in).** `vox-populi` and the orchestrator paths it touches; the
language surface for distributed durable workflows (`workflow`, `activity`,
`actor`, `@remote`, effect rows, `DurablePromise[T]`); the runtime that
executes them (`vox-workflow-runtime`, `vox-actor-runtime`); the vox-db
durability tier they share with the orchestrator; the multi-agent VCS
substrate that lets agents collaborate over the mesh; the dashboard
surfaces that make all of it operable; the developer-facing task-intake
(hopper) layer that sits above per-agent queues and inherits the durability
+ capability + signed-envelope guarantees defined here.

**Scope (out).** Pricing model internals (`vox-orchestrator::models`); skill
catalog UX (`vox-skills`); FableForge / Ludus / Scientia application surfaces
that ride atop the mesh (they get a paragraph each in §11.7 because they're
the load test, not the substrate).

**Audience.** Contributors, agents, and the future-self of whoever inherits
this. Read §1 first; §2 next if you're orienting; §3 if you're sequencing
work; §6 if you're shipping a release.

**Non-goals (reaffirmed; binding).**

1. **No custom crypto.** [`vox-crypto`](../../../crates/vox-crypto/) is the sole
   crypto SSOT per [cryptography-ssot-2026.md](cryptography-ssot-2026.md). No
   new AEAD, no novel signature scheme, no homegrown ZK.
2. **No blockchain or token economy.** The Vox trust ledger is local-then-gossip,
   not global consensus. Cost curve and governance burden are wrong for the
   ambition (power-user dogfood → opt-in volunteer compute).
3. **No TEE-first architecture.** Build the *attestation interface*; stub the
   TEE implementation. Don't gate the roadmap on consumer H100s.
4. **No onion routing / Tor-style anonymity.** Wrong threat model — we want
   auditable identity, not unlinkability.
5. **No transitive web-of-trust capability.** Reputation is a *signal*, not a
   *capability*. Paired peers + GitHub attestation are the binary gates.
6. **No public SaaS multi-tenant control plane.** Per
   [vox-dashboard-design-brief-2026.md §12](vox-dashboard-design-brief-2026.md);
   discovery is opt-in peer-to-peer.
7. **No dashboard becoming a code editor.** Code surface is a viewer, not Cursor.
8. **No `.ps1` / `.sh` / `.py` automation glue.** Scripts are `.vox`. See
   [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md).
9. **No mesh-replicated hopper inbox before mesh dispatch is authoritative.** The hopper's
   Option C (mesh-native inbox replicated via op-log gossip) is deferred until P0-T3
   (authoritative leases) lands. Until then, hopper is single-machine.
10. **No automatic priority-learning policy that overrides the developer.** A learning
    policy MAY emit advisory `priority_suggestion` events; it MUST NOT mutate a developer-set
    priority without an explicit developer action. See §3.5 Hp-T3 (typed `PrioritySource`
    partial order) and Hp-T4 (`DeveloperOverride` capability token).

---

## 1. The mental model — five spine primitives

Everything in this plan reduces to five primitives. If a feature can't be
expressed as one of these (or a small composition), it doesn't ship.

### 1.1 `DurablePromise[T]` — the only awaitable

A `DurablePromise[T]` is a journaled, possibly-remote, possibly-long-running
handle to a future value of type `T`. It subsumes today's `Future`,
`Promise`, `Activity`-call result, signal awaitable, and Restate-style
"awakeable":

| Behavior | Resolves how |
|---|---|
| Local pure compute | Resolves immediately; no journal entry |
| Local side-effecting activity | Journaled with auto-derived `activity_id`; replay returns cached value |
| Remote (`@remote`) call | Dispatched via populi A2A; journal records dispatch + lease + return |
| External signal / awakeable | Suspends until external `resolve(name, value)` lands; journaled |

**One canonical shape.** Per
[`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md) C4:
no `Future[T]` *and* `Promise[T]` *and* `DurablePromise[T]` — pick one, kill
the rest. The std library exposes only `DurablePromise[T]`. Compiler
diagnostics rewrite legacy spellings during deprecation.

**Replay semantics.** `await p` on replay first consults the workflow journal
keyed by the promise's content-derived ID. Hit → return cached. Miss →
execute and journal.

### 1.2 Effect rows — typed side effects per fn

Every function in the workspace has an inferred (and optionally declared)
**effect row**: a finite set drawn from `{ Net, Fs(read: GlobSet, write:
GlobSet), Time, Random, Secret, Spawn, Mcp(name), Mailbox(actor_ty),
GpuCompute, Mutate }`.

**`GpuCompute`** is the effect of running on a GPU/accelerator (CUDA, Metal, MPS, mlx). Required by
`@inference` and `@training_step` declarations from the [MENS plan](mesh-mens-distributed-training-and-execution-plan-2026.md)
(`Mn-T4`, `Mn-T5`). The compiler infers `GpuCompute` for any function transitively calling a
`vox-inference` or `vox-distributed-training` builtin; declared `@uses(gpu)` declares it
explicitly.

**`Mutate`** is the effect of mutating in-process state outside the workflow journal — the
non-journaled side of training-step state evolution that cannot be cleanly modeled as `Fs` or
`Spawn`. Allowed inside `@training_step` (where mutation is the point), forbidden inside `workflow`
bodies (where determinism is the point). Phase 1 `P1-T6` flips inference to bottom-up and adds
both variants.

**`Pure`** (empty effect row) is the zero-effect complement: no effects declared or inferred, and
all arguments and return values are pure value types. The compiler infers `Pure` for any function
with an empty effect set. `Mutate` and `Pure` are mutually exclusive — a function annotated
`@training_step` always carries at minimum `{ GpuCompute, Random, Mutate }` and is therefore never
`Pure`. The partial order is: `Pure < {any single row effect} < {any superset}` (more effects =
more restricted placement — a function requiring `Mutate` cannot be inlined into a `workflow` body
because `workflow` bodies enforce the `Pure | Net | Fs(read) | Time | Random | Secret | Spawn |
Mcp | Mailbox` whitelist and explicitly ban `Mutate`).

Three rules:

- **Bottom-up inference.** A function's row = builtins it calls ∪ rows of
  callees it calls. Today's [effect_check.rs](../../../crates/vox-compiler/src/typeck/effect_check.rs)
  does top-down validation; Phase 1 of this plan flips it.
- **Subset check at the boundary.** A `pub fn` declares `@uses(...)`; declared
  must be ⊇ inferred. Mismatch is an error with symmetric diagnostics
  (`missing-X` ↔ `unjustified-X`).
- **`workflow` body restriction.** A `workflow`'s inferred row must be
  `⊆ {Mailbox(_), Spawn(activity)}`. `Time | Random | Net` rejected unless
  wrapped in an `activity` callee or an explicit `side_effect { … }` block
  (which compiles to a single-shot activity).

This is the spine of safe distribution: an `@remote fn` only ships if
its row is serializable; a workflow is deterministic-by-construction; an
LLM-authored function carries its own audit trail.

### 1.3 Content-addressed code (CAS) — Unison-shape mobility

Every distributable artifact — workflow, activity, package, op-fragment —
has a stable BLAKE3 / SHA3-512 content hash. The substrate already exists:
[`vox-package/src/artifact_cache.rs`](../../../crates/vox-package/src/artifact_cache.rs)
indexes builds by SHA3-512 over inputs; the compiler emits stable
`@generated-hash` headers (Phase 1 of language-rules plan).

**Implications:**

- A workflow run is identified by `(workflow_fn_hash, args_hash)`. Replay
  loads code by hash; **the wrong-version bug class is structurally absent**.
- New workflow versions don't conflict with old ones — they have different
  hashes and run side-by-side until drained.
- Mesh dispatch ships code-by-hash (or the bytes if not cached); workers
  cache by hash; cold-start fetch is content-addressed deduplication.
- Op-fragments referencing a function carry its hash; replay-on-another-node
  is hash-fetch then evaluate.

### 1.4 Signed op-fragments — event-sourced mesh state

Every state-mutating event in the orchestrator (lock acquire, branch create,
file write, capability mint, lease grant, kudos credit, attestation post)
is an entry on the **op-log** with:

- `op_id` — sequential within daemon
- `predecessor_hash` — SHA3-256 chain hash (already in
  [`oplog/store.rs`](../../../crates/vox-orchestrator-queue/src/oplog/store.rs))
- `agent_id`, `model_id`, `change_id`, `produced_at` (already in
  [`oplog/mod.rs:116`](../../../crates/vox-orchestrator-queue/src/oplog/mod.rs))
- `signature` — Ed25519 over `(op_id, predecessor_hash, payload_hash)` using
  the daemon's vox-secrets-issued key (Phase 3)

**The op-log is the single source of mesh state.** Every other table —
locks, file affinity, capability ledger, attestations, kudos balances —
is a *projection* of the op-log. Restart replays the log and reconstructs
every projection. Distribution = gossip the log. Cross-machine sync =
anti-entropy on op-IDs known.

This collapses what would otherwise be five separate sync protocols into
one.

### 1.5 Capability tokens — typed proofs of authority

Sealed, typed values issued by a trusted minter that prove a function was
authorized to do something. Examples:

- `WorkingTreeWrite { workspace, branch }` — prove right to stage+commit on
  a specific branch
- `BranchCreate { workspace, parent }`
- `ExecLease { task_id, expires_at, holder }` — prove right to execute a task
  authoritatively
- `MeshDispatch { task_id, peer_id, scope }` — prove a dispatch was
  orchestrator-blessed
- `KudosCredit { peer_id, primitive, amount, task_id }` — prove a credit was
  earned

Today the mint paths are `#[doc(hidden)] pub fn` (verified at
[`vox-orchestrator-types/src/vcs_capability.rs:93–117`](../../../crates/vox-orchestrator-types/src/vcs_capability.rs)).
Phase 3 hardens to sealed traits and signs every mint with the daemon's
vox-secrets key, making forgery cross-node-detectable.

---

## 2. Audited current state

Per the research synthesis ([mesh-dashboard-and-distributed-compute-research-2026.md](mesh-dashboard-and-distributed-compute-research-2026.md)) and the verification pass on plan-critical primitives. One-page summary per area; this plan touches all five.

### 2.1 Mesh transport (`vox-populi`)

**Works.** Bearer auth (Mesh / Worker / Submitter / Admin roles, optional JWT-HS256), constant-time bearer compare ([transport/auth.rs](../../../crates/vox-populi/src/transport/auth.rs)), JSON node registry, A2A inbox with idempotency keys. Mens training in-process. Hardware probes (NVML / wgpu / DRM / Metal). `[mesh]` config parser ([vox-repository/src/populi_toml.rs:13](../../../crates/vox-repository/src/populi_toml.rs)) reads `Vox.toml` keys.

**Doesn't.**

1. ADR-017 leases are not authoritative — leases exist as types but the dispatch path doesn't consult them before fallback decisions. Recovery on crash is best-effort. **Severity: P0** for any multi-node use.
2. JWE secrets *are* decrypted at [`a2a/remote_worker.rs:124`](../../../crates/vox-orchestrator/src/a2a/remote_worker.rs) but only `secret_count` is logged — comment line 133 reads "task-scoped injection is S2." Cross-node secret use is dead code today. **Severity: P0** for any task that needs a remote secret.
3. `traceparent: None` hardcoded at [`a2a/dispatch/mesh.rs:119`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs) — receiver ignores even if set. **Severity: P1** for multi-node debugging.
4. Hardware probes have no mock harness; operator labels override probe output. **Severity: P1** for routing reliability.
5. Bare HTTP / no TLS. Tokens, signatures, payload metadata in cleartext. **Severity: P0** for anything off-LAN.
6. Kudos types defined; nothing computes `duration_ms` or persists a contribution ledger. **Severity: P1** for the volunteer-compute story.

### 2.2 Language layer (`vox-compiler`, `vox-codegen`)

**Works.** `workflow` / `activity` / `actor` keywords parse and lower to `DurabilityKind` ([`hir/nodes/durability.rs:13`](../../../crates/vox-compiler/src/hir/nodes/durability.rs)). Effect annotations parse (9 variants in [`ast/decl/effect.rs`](../../../crates/vox-compiler/src/ast/decl/effect.rs)). [`typeck/effect_check.rs`](../../../crates/vox-compiler/src/typeck/effect_check.rs) does **top-down subset validation** and emits **errors** (not warnings, contrary to earlier audit).

**Doesn't.**

1. Inference is top-down (declared-set checking) not bottom-up. Unannotated callees don't propagate effects upward. **Severity: P1** — Phase 1 work.
2. No determinism check inside `workflow` body. `time.now()` and `random.*` work. **Severity: P0** — turns workflows into a lie.
3. No `@remote` annotation. Distribution is via `mesh_*` naming-convention. **Severity: P1**.
4. No `DurablePromise[T]` type. `Future`, `Promise`, activity-call result, signal-await are separate shapes. **Severity: P0** — every distributed primitive bloats the surface area an LLM author has to track.
5. Codegen emits identical async Rust for `fn`, `activity`, `workflow`, `actor`. `schedule_interval` and `durability` metadata ignored. **Severity: P0**.
6. Persisted actor state fields parse but don't compile. **Severity: P2**.

### 2.3 Workflow runtime (`vox-workflow-runtime`)

**Works.** Linear interpreter; `interpret_workflow_durable` emits JSON journal entries to vox-db; `tracker.load_activity_result(workflow_name, &activity_id)` ([run.rs:58](../../../crates/vox-workflow-runtime/src/workflow/run.rs)) implements user-supplied-ID replay. `__durable_signal_wait:key` planner step exists.

**Doesn't.**

1. Idempotency is opt-in user-supplied `activity_id` strings. No auto-derivation. **Severity: P0**.
2. Replay limited to literal loops + deterministic `if`. `match` and complex branching unsupported. **Severity: P1**.
3. No content-addressed code mobility — workflow at version A vs B has no separation. **Severity: P0** for in-flight upgrades.
4. No saga / compensation DSL. **Severity: P2**.
5. No structured signals (`Signal[T]`); strings only. **Severity: P2**.

### 2.4 Multi-agent VCS

**Works.** Per [git-concurrency-policy.md](git-concurrency-policy.md): banned-command enforcement at GitExec, capability tokens, three commit trailers (Co-authored-by, Vox-Model-Id, Vox-Workspace), pre-commit secret scan. Oplog with `predecessor_hash` SHA3-256 chain ([`oplog/store.rs:5`](../../../crates/vox-orchestrator-queue/src/oplog/store.rs)). File-level locks (Exclusive / SharedRead).

**Doesn't.**

1. Locks are in-memory `Arc<RwLock<HashMap>>`. **Two daemons silently race.** **Severity: P0**.
2. Oplog is in-memory `VecDeque`. Lost on restart. No replication. **Severity: P0** for any cross-machine coordination.
3. Capability mints are `#[doc(hidden)] pub fn` — soft-private; forgeable from another crate. **Severity: P1** until cross-node; **P0** when crossing trust boundaries.
4. No arch-check rule for raw `Command::new("git")` outside `git_exec.rs`. Banned-list bypass possible. **Severity: P1**.
5. `vox-forge` exists as a crate but is not invoked by the orchestrator loop. PR creation is manual. **Severity: P2**.

### 2.5 Dashboard mesh control (`vox-dashboard`)

**Works.** Axum + React 19 SPA, WebSocket event bus + HTTP `POST /v1/tools/call`, localhost-friendly auth, Bearer-token meta-tag injection.

**Doesn't.** Every mesh-control route is a fixture stub. NetworkTab is empty placeholder. TaskDispatch is local-state-only. No "Add a Node" wizard. No live event subscription on mesh routes. (Full table in [research doc §1.2](mesh-dashboard-and-distributed-compute-research-2026.md).) **Severity: P0** for personal-mesh adoption — dashboard is the on-ramp.

---

## 3. The phased upgrade plan

Seven phases (0 through 6) sequenced by what unblocks what. Each phase is
independently shippable and adds a coherent capability slice. Phase numbers
are cardinal, not parallel — Phase 1 depends on Phase 0; Phase 4 on 0–3.

Within a phase, tasks have stable IDs (`P0-T1`, `P3-T5`, etc.) so downstream
plan documents and PRs can reference them.

### Phase 0 — Foundations (P0)

**Goal.** Make the mesh authoritatively trustworthy at LAN scale, and put
the substrate in place that every later phase depends on. Nothing in
Phase 1+ ships unless Phase 0 holds.

**Killer feature delivered.** *"Two daemons, multi-agent, same repo, no
data loss."* Plus: a Vox node is no longer a debug visualization.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P0-T1` | **Persist file-lock map to vox-db** | new `vcs_lock` table; rewrite [`vox-orchestrator-queue/src/locks/mod.rs`](../../../crates/vox-orchestrator-queue/src/locks/mod.rs) to read/write through `vox-db` | WAL replay on daemon start; lock schema = `(path PK, kind, holder, expires_at, lease_id)` |
| `P0-T2` | **Single lock-leader election** with heartbeat | new `lock_leader` row in vox-db; loser proxies via existing A2A envelope | One A2A round-trip per lock op when leader is remote; sub-millisecond when local |
| `P0-T3` | **Authoritative leases (W1, ADR-017)** | [`vox-orchestrator/src/a2a/dispatch/mesh.rs`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs); higher-level dispatcher that picks local-vs-mesh | Before falling back to local executor, consult lease state; if remote holds an unexpired lease, return error rather than duplicate-execute |
| `P0-T4` | **Inject decrypted JWE secrets into task exec context** (close W3 dead-end) | [`a2a/remote_worker.rs:120`](../../../crates/vox-orchestrator/src/a2a/remote_worker.rs); secrets propagation into [`vox-skills`](../../../crates/vox-skills/) execution path | Drop the `secret_count` log line; replace with task-scoped secret injection that respects `@uses(secret)` effect declarations |
| `P0-T5` | **TLS / WireGuard option** on populi HTTP plane | new `[mesh.transport]` config keys; document Tailscale-Funnel as the recommended off-LAN deployment | One ergonomic default (rustls cert from a known path); WireGuard sidecar documented but not bundled |
| `P0-T6` | **Hardware probe trait + mock harness** (W2 already speced) | per [populi-mesh-probe-correctness-spec-2026.md](populi-mesh-probe-correctness-spec-2026.md) | Implementation plan: [populi-mesh-probe-correctness-plan-2026.md](populi-mesh-probe-correctness-plan-2026.md) — proceed as-is |
| `P0-T7` | **Move in-process executor behind `SkillRuntime` trait** | wire [`vox-skill-runtime`](../../../crates/vox-skill-runtime/) as the seam; orchestrator's in-process executor becomes one impl among (wasm, container) | The verification pass found 0 uses of the trait in vox-orchestrator today. This unblocks Phase 5 sandbox tiering. |
| `P0-T8` | **Populate `traceparent` on dispatch + read on receiver** (W5 from spec → wired) | [`a2a/dispatch/mesh.rs:119`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs); receiver in [`a2a/remote_worker.rs:100`](../../../crates/vox-orchestrator/src/a2a/remote_worker.rs) | Cross-node traces become navigable; dashboard run-row deep-links work. **Natural place to bundle the three new `AgentEvent` variants from `Hp-T2`** (`TaskReprioritized`, `HopperItemAdmitted`, `HopperItemOverridden`) — same `events.rs` touch, no separate task needed. |

**Acceptance.**

- New integration test fixture: two `vox-orchestrator-d` instances on same
  host, three agents, forced lock contention → no double-write, no dropped
  task. Replay after kill-9 of leader → no data loss.
- `cargo run -p vox-arch-check` clean.
- All mesh dispatch paths consult lease state before local fallback.
- Encrypted secrets land in task env when `@uses(secret)` declares them.
- TLS smoke test: `vox populi serve --tls cert.pem` accepts a peer over HTTPS.

**Dependencies.** None outside this phase.

**Estimated PR count.** 8 (one per task), serial-ish: T1 → T2; T3 parallel to T1/T2; T4–T8 fully parallel.

---

### Phase 1 — Language primitives (the durability spine)

**Goal.** The Vox language has a single canonical primitive for distributed
durable work. Effect rows are inferred and enforced. The compiler refuses to
compile a workflow that calls `time.now()`.

**Killer feature delivered.** *Type-safe distributed programs.* An LLM
author writing a `workflow` cannot accidentally introduce non-determinism.
A `@remote` call cannot be invoked with non-serializable args. `vox check`
is the safety net.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P1-T1` | **Introduce `DurablePromise[T]`** as the single awaitable primitive | new `std::durable_promise` (Vox); compiler intrinsic; codegen lowers to `tokio::sync::oneshot` + journal entry | Subsumes `Future`, `Promise`, `Activity`-result, signal-await, awakeable |
| `P1-T2` | **Deprecate `Future[T]` / `Promise[T]`** with migration diagnostics | [`vox-compiler/src/typeck`](../../../crates/vox-compiler/src/typeck/) | Auto-rewrite hint in `vox check`; one-release deprecation window |
| `P1-T3` | **`@remote fn foo()` annotation** | parser, AST, HIR; effect inference adds `Spawn` + `Net` | Replaces `mesh_*` naming convention. Args must impl serializable trait (compile-time check) |
| `P1-T4` | **Auto-derived `activity_id`** = `blake3(workflow_id ‖ call_site_span ‖ structural_arg_hash ‖ replay_counter)` | [`workflow/run.rs:58`](../../../crates/vox-workflow-runtime/src/workflow/run.rs); compiler emits the hash inputs at the call site | `@with_id(expr)` override for business identity; warning when args contain `time.now()` / `random.*` |
| `P1-T5` | **Workflow determinism check** (forbidden builtins inside `workflow` body) | extend [`effect_check.rs`](../../../crates/vox-compiler/src/typeck/effect_check.rs) with `DurabilityKind::Workflow` row restriction | Diagnostic: `vox/workflow/non-deterministic-builtin` with auto-suggest "wrap in activity" |
| `P1-T6` | **Bottom-up effect inference** | [`effect_check.rs`](../../../crates/vox-compiler/src/typeck/effect_check.rs) | Today is top-down (declared-validation only); flip to compute-then-check (extends enum with `GpuCompute` and `Mutate` per §1.2; required by MENS Mn-T4/Mn-T5). |
| `P1-T7` | **`side_effect { … }` block** as the only sanctioned non-determinism inside workflows | parser + desugar to single-shot activity | Temporal `SideEffect` semantics |
| `P1-T8` | **`vox workflow preview <fn>(args)` dry-run projector** | new subcommand in [`vox-cli/src/commands`](../../../crates/vox-cli/src/commands/) | Type-checks, infers effects, projects schedule of activities that *would* run; no side effects |
| `P1-T9` | **Stable diagnostic IDs** in `vox/<category>/<kebab>` namespace for everything new in this phase | per [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md) Phase 2 | LLMs trained on 0.5 still recognize 0.7 errors |

**Acceptance.**

- A `workflow` body containing `time.now()` fails `vox check` with
  `vox/workflow/non-deterministic-builtin`.
- A `@remote fn foo(x: i32) → i32` compiles; `@remote fn bar(x: NotSerializable)`
  fails to compile with a diagnostic naming the offending parameter.
- An activity called twice in a workflow with the same args returns the
  cached value on the second invocation, with no user-supplied ID.
- `vox workflow preview my::workflow(arg1, arg2)` prints the projected
  schedule without dispatching.
- All new diagnostics carry `vox/<kebab>` IDs.

**Dependencies.** Phase 0 complete (lease wiring is consumed by `@remote`).

**Estimated PR count.** 9 (one per task); T1–T2 must merge first as they
shape every later codegen change.

---

### Phase 2 — Code mobility & versioning (the version-skew killer)

**Goal.** Code that runs on the mesh is content-addressed. Version skew is
structurally impossible — workflow versions A and B coexist by hash and
drain on a schedule.

**Killer feature delivered.** *Hot-deploy a workflow without breaking
in-flight runs.* Plus: a fresh node joins the mesh and runs jobs by fetching
content-addressed bundles, without a forge round-trip.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P2-T1` | **Workflow content-hash via vox-package CAS** | extend [`vox-package/src/artifact_cache.rs`](../../../crates/vox-package/src/artifact_cache.rs) to expose `lookup(fn_hash) → Option<Bundle>`; compiler stamps stable `@generated-hash` on each `workflow` and `activity` | Already SHA3-512 over inputs; reuse |
| `P2-T2` | **`workflow.version("change-1", min, max)`** patch-marker primitive | parser + runtime; journals `WorkflowPatch` op | Temporal-style escape hatch when content-addressed isn't enough |
| `P2-T3` | **`vox workflow drain --version <hash>`** operational tool | [`vox-cli`](../../../crates/vox-cli/src/commands/) | Marks a version "no new starts"; existing in-flight finish |
| `P2-T4` | **CAS-bundle code seeding for mesh-dispatched jobs** | [`a2a/dispatch/mesh.rs`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs) ships a CAS bundle reference (or full bytes if peer doesn't have it cached) along with the envelope | Worker fetches by hash; cache hits everywhere |
| `P2-T5` | **Activity result caching ledger** keyed by `(activity_id, structural_arg_hash)` | new vox-db table `activity_result_cache(activity_id, arg_hash, result, dedup_window_until)`; consumed by [`workflow/run.rs:58`](../../../crates/vox-workflow-runtime/src/workflow/run.rs) | `@activity(dedup = "7d")` configurable window |
| `P2-T6` | **`vox dispatch preview`** — generalize the preview tool to "what would the orchestrator route where" | extends `P1-T8` shape to dispatch-time | Operators run before they touch production |
| `P2-T7` | **Codegen: lower `DurabilityKind` to specific runtime calls** | [`vox-codegen`](../../../crates/vox-codegen/) — `workflow` → `interpret_workflow_durable`; `activity` → journaled call; `actor` → mailbox spawn | Closes the "all three emit identical async Rust" gap |

**Acceptance.**

- A workflow at content-hash A and a refactored version at hash B coexist
  in vox-db without conflict; `vox workflow ls` shows both.
- Killing a worker mid-activity then restarting → workflow resumes from
  the last journaled `DurablePromise` without re-running completed activities.
- A second daemon receives a dispatch envelope; it fetches the bundle by
  hash from the sender; subsequent jobs of the same hash hit the cache.
- `vox dispatch preview my::workflow(...)` prints the routing decision
  without dispatching.

**Dependencies.** Phase 1 complete (`DurablePromise`, auto-derived
`activity_id`, `@remote` are inputs).

**Estimated PR count.** 7.

---

### Phase 3 — Multi-agent VCS over mesh (op-log gossip)

**Goal.** The op-log is durable and gossip-replicated. Capability mints
and op-fragments are signed. Two daemons coordinate on the same repo via
gossip + single lock-leader.

**Killer feature delivered.** *Mesh-distributed multi-agent code editing
with no data loss.* Plus: the op-log is the single source of mesh state;
every other table (locks, affinity, capabilities, kudos) becomes a
projection.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P3-T1` | **Persist oplog to vox-db** | new table `convergence_op_log(op_id PK, set_id, parent_op_ids JSON, payload BLOB, signature, agent_id, produced_at)` + index `(set_id, produced_at)`; rewrite [`oplog/store.rs`](../../../crates/vox-orchestrator-queue/src/oplog/store.rs) | Tiered retention: hot (last 10 K) in `VecDeque`, warm in db, cold compacted to `Checkpoint` ops |
| `P3-T2` | **Sign every capability mint and op-fragment** with daemon's vox-secrets-issued Ed25519 key | [`vox-orchestrator-types/src/vcs_capability.rs`](../../../crates/vox-orchestrator-types/src/vcs_capability.rs); [`oplog/store.rs`](../../../crates/vox-orchestrator-queue/src/oplog/store.rs); receiver verifies | Reuses `vox-crypto` Ed25519 — no new crypto |
| `P3-T3` | **Bounded gossip topic over A2A envelope** with Bloom-filter anti-entropy | extend [`vox-orchestrator/src/a2a/dispatch/mesh.rs`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs) with new message kind `OpFragmentSync`; sweep every 30 s | Demers et al. epidemic algorithm |
| `P3-T4` | **Vector-clock file affinity** for mesh-wide hint routing | [`vox-orchestrator-queue/src/affinity.rs`](../../../crates/vox-orchestrator-queue/src/affinity.rs) — widen value to `(daemon_id, lamport)` | LWW with hold-down timer (60 s); affinity is hint, lock is hard |
| `P3-T5` | **`LockWait` outcome** added to `MergeOutcome` enum | per spec [§Wire-protocol](multi-agent-vcs-replication-spec-2026.md) | Tier 2 of the conflict funnel becomes explicit |
| `P3-T6` | **Sealed-trait hardening for capability mint** | replace `#[doc(hidden)] pub fn mint` with `pub(crate)` + `Sealed` trait via new internal facade crate | Forgery becomes type-error, not convention |
| `P3-T7` | **`vox-arch-check` rule: raw `Command::new("git")` outside `git_exec.rs` fails CI** | [`vox-arch-check/src/main.rs`](../../../crates/vox-arch-check/src/main.rs) — new `[[forbidden_pattern]]` rule type | Per agentic-vcs Phase 4 plan; raises banned-list from prose to Rust |
| `P3-T8` | **Unknown-parent fragment hold + backfill** with bounded queue (1024 / 64 KiB) and DLQ to vox-db | gossip handler; surface in dashboard | Phase 1 of multi-agent-vcs spec, brought forward |
| `P3-T9` | **Op-log projections architecture** — locks, affinity, capabilities, kudos all rebuild from log | new `Projection` trait in [`vox-orchestrator-queue`](../../../crates/vox-orchestrator-queue/) | Restart replays log → reconstructs every table; one place to look for "what happened?" |

**Acceptance.**

- 5-agent + forced-conflict golden test (currently single-machine) passes
  across two daemons.
- Capability mint forged by a non-leader daemon is rejected with signature
  failure, surfaced in dashboard audit log.
- Daemon A crashes; daemon B continues; A restarts and catches up via
  Bloom-filter sync within ≤ 30 s; locks held by A are released after lease
  timeout, not silently abandoned.
- `cargo run -p vox-arch-check` fails when test fixture introduces a
  raw `Command::new("git")` outside the wrapper.
- Op-log replay reconstructs every projection bit-identically.

**Dependencies.** Phase 0 complete (vox-db substrate, lock leader). Phase 2
complete (`DurablePromise` semantics inform op-fragment payload shape).

**Estimated PR count.** 9.

---

### Phase 4 — Dashboard mesh control (the on-ramp)

**Goal.** The dashboard can provision, configure, monitor, and operate a
personal mesh end-to-end. Five-minute journey from "first open" to
"friend's GPU is executing my jobs" works.

**Killer feature delivered.** *The on-ramp.* Without this, mesh adoption
is gated on CLI fluency and we ship a working tool nobody can use.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P4-T1` | **Wire mesh routes to live orchestrator state** (replace fixtures) | [`vox-dashboard/src/api/mesh.rs`](../../../crates/vox-dashboard/src/api/mesh.rs); subscribe to orchestrator EventBus over WS | Per design brief Phase 2 |
| `P4-T2` | **"Add a Node" wizard** with one-shot install command + QR-code as coequal | new wizard flow; `vox-crypto` for Ed25519 keypair gen; `vox-identity` for handle | One-shot bearer ≤ 10 min TTL; embedded peer_id; install command prints itself first (`--print` mode) before piping to shell |
| `P4-T3` | **Donation-policy editor** (slots, kinds, NSFW filter, per-peer overrides) | new `vox-mesh-policy` (L2) consumed by orchestrator admission; UI in dashboard | Policy file is `donations.vox` — first-class Vox source, type-checked, version-controlled |
| `P4-T4` | **Live topology canvas** with health colors (replaces empty `NetworkTab.tsx`) | [`vox-dashboard/app/src/generated/NetworkTab.tsx`](../../../crates/vox-dashboard/app/src/generated/NetworkTab.tsx) | Force-graph that *doesn't* re-layout per event; click-to-pin; status pill per node |
| `P4-T5` | **Audit-log scrubber** — timeline slider over op-log → state at instant | new `/api/v2/oplog/at/{ts}` route; UI; consumes Phase 3 op-log | Temporal-replay equivalent for Vox |
| `P4-T6` | **Per-node spend gauge + mesh-wide budget bar** | extends existing `budget.*` settings; cost from Phase 2 dispatch envelope | |
| `P4-T7` | **Mesh-aware `⌘K` palette** ("kill on node X", "drain Y", "send latest to friend-gpu") | extends existing `cmdk.vox` | |
| `P4-T8` | **Workflow visual debugger** — timeline of activity calls; click → state at instant | builds on Phase 1 `vox workflow preview` | Pairs Forge with the durable workflow runtime |
| `P4-T9` | **Run-row drawer with full event tree + trace_id deep-link** | wires `vox.mesh.trace_id` from Phase 0 T8 | |
| `P4-T10` | **Privacy-class indicator** on every job + every span | enforces `vox.mesh.privacy_class` from S1 obs spec | |
| `P4-T11` | **Onboarding wizard for joining someone else's mesh** | inverse of T2; paste invite → become a worker | |
| `P4-T12` | **Mesh-wide model registry view** — "which LoRA / Ollama tag lives where" | new `vox-mesh-models` query | Answers "who can run llama-70b?" before dispatch |

**Acceptance.**

- The "personal mesh in 5 minutes" journey from
  [research §2.1](mesh-dashboard-and-distributed-compute-research-2026.md)
  works end-to-end on two laptops.
- "Kill on node X" via `⌘K` lands a real signal at the orchestrator and
  surfaces in the audit log.
- Donation policy edits in the GUI persist as a `donations.vox` file under
  version control.
- Workflow visual debugger shows the live activity timeline of an
  in-flight workflow.
- All destructive actions (kill, pause, drain, replay) require explicit
  confirmation and emit an audit-log entry.

**Dependencies.** Phases 0–3 complete (live data, op-log, signed mints).

**Estimated PR count.** 12.

---

### Phase 5 — Public-internet safety (the trust ladder)

**Goal.** A Vox node is safe to expose to the internet under bounded trust
— vetted public peers only, with abuse fuses, attestation, and identity
binding.

**Killer feature delivered.** *Two GitHub-attested strangers can pair their
personal meshes and share compute.* Kudos accounting is real, end-to-end.

| ID | Task | Files | Notes |
|---|---|---|---|
| `P5-T1` | **Replace JWT-HS256 with Ed25519-signed envelope** | [`vox-populi/src/transport/auth.rs`](../../../crates/vox-populi/src/transport/auth.rs); fix "any token-holder forges" | Use existing `vox-identity` keys; node signature handler path that's already there but unused |
| `P5-T2` | **GitHub-attestation gate at pairing** | per [ludus-identity-github-integration-research-2026.md](ludus-identity-github-integration-research-2026.md) device-flow; refuse pairing without verifiable attestation | Gist/repo-hosted signed JSON; verifiable by anyone, no Vox-owned server |
| `P5-T3` | **Per-key quota + reputation EMA** | new `vox-populi/src/quota/` keyed on `node_pubkey`; persist counters to vox-db | Successful jobs / failed validations / last seen → `PeerReputation` sidecar to `NodeRecord` |
| `P5-T4` | **Result attestation via signed deterministic replay** | extend `TaskResult.attestation` field already in [`vox-mesh-types`](../../../crates/vox-mesh-types/) — populate; worker signs `(task_id, input_hash, output_hash, gpu_seconds, trace_blake3)` with per-job ephemeral key | Per-TaskKind mapping per [research §3.5](mesh-dashboard-and-distributed-compute-research-2026.md) |
| `P5-T5` | **Submitter-side spot-check sampler** (default p=0.05 replay; configurable via [mesh.attestation.spot_check_rate]) | orchestrator-side validator that re-runs a fraction of attested results | Detects forged attestations |
| `P5-T6` | **Per-job ephemeral Ed25519 subkey** scoped to single `task_id`; lifetime = lease TTL | minted via [`vox-identity`](../../../crates/vox-identity/) at dispatch; signed by long-term node key | Limits blast radius if worker is compromised mid-task. Ephemeral key lifetime MUST equal the lease TTL — bind `ephemeral.expires_at_unix_ms == lease.expires_at_unix_ms` at dispatch (assertion in `P5-T6b` substep). |
| `P5-T7` | **Kudos accounting end-to-end** — populate `KudosPrimitive::GpuComputeMs` from `TaskResult.duration_ms`; persist contribution ledger; surface in dashboard | [`vox-mesh-types/src/kudos.rs`](../../../crates/vox-mesh-types/src/kudos.rs) types already exist; close the plumbing | Single signed envelope is BOTH attestation AND kudos credit — two birds |
| `P5-T8` | **Mesh-wide model inventory aggregation** | scheduled refresh; planner sees what LoRAs / quantizations live where | Ends "have to retry locally because forgot remote has the weights" |
| `P5-T9` | **Privacy-of-submitted-work signaling** — `WorkerDonationPolicy.accept_sensitive_workloads: bool` | extend `WorkerDonationPolicy` in [`vox-mesh-types`](../../../crates/vox-mesh-types/) | Submitter learns "this worker will see plaintext" and can route around |
| `P5-T10` | **Per-pairing X25519 keys for JWE** (W3 closure) | replace shared mesh-secret BLAKE3 derivation | Currently a single derived key per cluster; per-pairing isolates blast radius |

**Acceptance.**

- Fresh public mesh node accepts work from a paired peer with valid GitHub
  attestation; refuses paired peer with revoked attestation; refuses
  unpaired peer.
- Fuzz testing fires the per-key quota fuse before depleting node resources.
- Submitter-side spot-check detects an injected forged result with
  > 99% probability over 100-job run (achieved at p≥0.05; p=0.01 only achieves ~63%, see §6 risk row 13).
- Kudos ledger reconciles: sum of credited GpuComputeMs across all tasks =
  sum of TaskResult.duration_ms within ε.
- Revocation of a peer's attestation propagates as a tombstone within
  ≤ 60 s for paired peers.

**Dependencies.** Phases 0–4 complete.

**Estimated PR count.** 10.

---

### Phase 6 — The grand network (volunteer compute)

**Goal.** Opt-in, joinable, bounded-trust global mesh. Strangers with
GitHub-attested identities can contribute compute to (and consume from)
each other's meshes.

**Killer feature delivered.** *A volunteer compute network where compute is
freely shared between vetted contributors without a central server, a token,
or a SaaS to depend on.*

| ID | Task | Files | Notes |
|---|---|---|---|
| `P6-T1` | **Federation envelope shape** — op-fragment compatible-in-concept with ATProto/ForgeFed (signed Activity-object) | extends `OpFragmentEnvelope` from Phase 3 | Adopt the *shape*, not the transport; ActivityPub is too verbose |
| `P6-T2` | **Optional public attestation registry** — signed JSON manifest in a known git repo, like ATProto DID-doc | new `vox populi attest publish` subcommand | Lets a new node bootstrap discovery without a Vox-owned server |
| `P6-T3` | **Tier-4 micro-VM sandbox interface** (firecracker/kata) | extend [`vox-skill-runtime`](../../../crates/vox-skill-runtime/) trait; mock impl ships first | Real impl deferred to v1.x; pre-wire the seam |
| `P6-T4` | **Redundant-execution voting** for deterministic batch jobs (BOINC adaptive replication) | new `RedundancyPolicy` in `WorkerDonationPolicy`; dispatch path forks N-redundant on declared-deterministic tasks | Adaptive: only re-verify untrusted hosts; skip for trust-tier-3 peers |
| `P6-T5` | **TEE attestation interface** (stubbed; H100 / SEV-SNP impl deferred) | extend `TaskResult.attestation` with optional `tee_quote` field | Build the *envelope*, not the implementation |
| `P6-T6` | **Discovery feedback loop** — the mesh becomes the discovery substrate for Scientia | per [scientia-mesh-integration-research-2026.md](scientia-mesh-integration-research-2026.md) | Auto-publish "this LoRA on this node performs X% better at Y task" as a Scientia Finding |
| `P6-T7` | **Public mesh quickstart docs + `vox populi join <invite>`** flow | docs/howto + CLI subcommand | The "I want to volunteer my GPU to a friend's project" experience |
| `P6-T8` | **Self-publication of trust-graph snapshots** to forge ([vox-publisher](../../../crates/vox-publisher/) integration) | per [scientia-self-publication-finalization-plan-2026.md](scientia-self-publication-finalization-plan-2026.md) | Auditable trust history without a Vox-owned ledger |

**Acceptance.**

- Two contributors who have never met before pair their meshes via published
  attestation manifests; share compute on a deterministic Embed task with
  redundant-execution; kudos credit reconciles to within ε.
- A TrainQLoRA result attests via signed deterministic replay on first
  epoch; submitter-side spot-check passes; loss curve matches second-runner
  within tolerance.
- Revoking a contributor (their GitHub identity is compromised) propagates
  via gossip; new dispatches refuse them within ≤ 5 min.
- The Scientia feedback loop publishes a `Vox Provider Atlas` quarterly
  Finding sourced from real mesh telemetry.

**Dependencies.** Phases 0–5 complete.

**Estimated PR count.** 8.

---

### 3.5 Cross-cutting Hopper track (Hp-T1..Hp-T9)

The unified-task hopper ([unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md))
sits **above** the per-phase tracks: a single developer-facing intake surface that fans work
across the per-agent queues already present in `vox-orchestrator`. It is *not* a phase — it is
a thin layer that depends on a small subset of P0/P3/P4/P5 deliverables and ships across the
release contracts in §7.

**Scope.** Single-machine Option A first; persistent Option B after P3-T1 lands; mesh-replicated
Option C deferred until P0-T3 makes mesh dispatch authoritative.

**Killer feature delivered.** *A single front door for work* — the developer drops ideas into
one chat, the orchestrator fans them across agents, and the developer retains formal,
type-enforced override authority over priority.

| ID | Task | Files | Notes |
|---|---|---|---|
| `Hp-T1` | New `crates/vox-orchestrator/src/hopper/` L1 module | new module; types `IntakeItem { intent, affinity_hints, priority_hint, source }`, `HopperItemId`, `IntakeSource`, `PriorityHint` | Forward-compat with Option B persistence; content-addressed IDs only |
| `Hp-T2` | Three new `AgentEvent` variants on existing event bus | `crates/vox-orchestrator/src/events.rs` — add `TaskReprioritized`, `HopperItemAdmitted`, `HopperItemOverridden` | Bundles cleanly with `P0-T8` traceparent work which already touches events.rs |
| `Hp-T3` | `TaskPriority { value, source: PrioritySource }` typed partial order | `crates/vox-orchestrator-types/src/agent_types/`; dispatcher reorder API | `PrioritySource = Developer \| Orchestrator \| LearningPolicy`; Developer dominates orchestrator dominates learning |
| `Hp-T4` | New `DeveloperOverride` capability token | slots into the sealed-trait facade introduced in `P3-T6` | Mutating a `Developer`-sourced priority requires this token; only hopper intake / dashboard can mint |
| `Hp-T5` | Hopper persistence schema (Option B forward-compat) | new vox-db table `hopper_inbox(item_id PK, intent, affinity_hints JSON, classified_priority, source, batch_id, state, produced_at)` | Lands in `P3-T9`'s op-log projection registry as a new `HopperInbox` projection |
| `Hp-T6` | Dashboard `/api/v2/hopper/{inbox,assigned,history}` + WS `/api/v2/hopper/events` | `crates/vox-dashboard/src/api/hopper.rs` (new); UI panel | Co-equal with `P4-T13`; uses canonical WS convention from §5.6 (`/v1/ws` topic, NOT `/api/v2/hopper/events`); routes through canonical audit-log writer per §5.7. |
| `Hp-T7` | Worktree-per-batch lifecycle hook | Claude Code harness convention; document under `docs/src/contributors/` | Each hopper batch maps to one branch and one PR; auto-archive worktree at batch close |
| `Hp-T8` | Mid-flight reprioritization state machine: `Inbox → Triaged → Assigned → Started → CommitMinted → Pushed → Closed` | dispatcher; persisted to vox-db (Option B+) | Reprioritization allowed at every state with state-dependent semantics (re-queue / cooperative pause / no-op-but-affects-next) |
| `Hp-T9` | Optional opt-in `vox-priority-policy` crate | new L2 crate; emits advisory `priority_suggestion` events; never mutates developer-set priorities | Off by default; opt-in via `[hopper.learning_policy.enabled = true]`; future MENS application surface (see MENS §6) |

**Acceptance.**

- A `Developer`-sourced priority cannot be mutated by any orchestrator policy without a
  `DeveloperOverride` capability token. Integration test asserts this invariant.
- Hopper intake admits an item, the dashboard surfaces it, the developer reorders it, and the
  audit trail (`vox.orchestrator.hopper.*` telemetry) records every transition.
- Worktree-per-batch creates one branch and one PR per hopper batch; closing the batch
  garbage-collects the worktree.
- Option B persistence schema replays from disk on orchestrator restart; in-memory state is
  reconstructed without data loss.

**Dependencies.** Hp-T1..T3 stand alone (single-machine Option A). Hp-T4 depends on P3-T6
sealed-trait facade. Hp-T5 depends on P3-T1 vox-db op-log substrate. Hp-T6 depends on P4-T1
live mesh routes. Hp-T8 (full state machine) depends on P3-T9 op-log projections. Mesh-replicated
Option C is gated behind P0-T3 + Phase 3 complete.

**Estimated PR count.** 9 (one per Hp-T<n>, mostly serial within the L1 module).

**Cross-references.**

- [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md) — design space,
  three options, override-contract rationale, telemetry-driven priority learning.
- §6 risk register additions: rows 11–13 below.
- §7 release contracts: v0.6 includes Hp-T1..T4 (Option A); v0.7 includes Hp-T5..T8 (Option B);
  v1.x adds mesh-replicated Option C.

---

## 4. The huge high-win improvements (TL;DR for impatient readers)

Distilled from the seven-phase plan; one paragraph each on **why this is
specifically a high-win** rather than incremental polish.

### 4.1 `DurablePromise[T]` collapses five primitives into one

Today an LLM author has to decide between `Future`, `Promise`, an activity
call, a signal-await, and an awakeable. Each has subtly different replay
semantics. The single canonical primitive (P1-T1) means: every distributed,
durable, awaitable thing in Vox is the same shape. Models burn fewer
decisions; refactors don't change types; the surface area an audit needs to
cover drops by ~80%. This is C4 ("one canonical shape per concept") in
[`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md)
realized.

### 4.2 Auto-derived `activity_id` removes the #1 distributed footgun

Every Temporal/Restate/Cadence story-of-pain anyone has lived through
involves a manual idempotency key that was wrong. Auto-deriving from
`(call-site span hash ‖ structural arg hash ‖ replay counter)` (P1-T4)
makes the right thing the default. LLMs that rename functions don't
accidentally invalidate dedup history. Refactors are safe. Manual override
is one annotation away when business identity matters.

### 4.3 The orchestrator's existing oplog is 90% of distributed VCS

[`oplog/mod.rs:116`](../../../crates/vox-orchestrator-queue/src/oplog/mod.rs)
already has `agent_id`, `model_id`, `change_id`, SHA3-256 chain,
`snapshot_before/after`. Persist it (P3-T1), sign it (P3-T2), gossip it
(P3-T3) — and you have multi-agent VCS over mesh. **Three PRs land what
took years for Pijul.** The architectural insight: every other piece of
mesh state (locks, affinity, capabilities, kudos) is a *projection* of the
op-log (P3-T9). One source, many views, automatic convergence.

### 4.4 vox-package CAS makes version skew structurally impossible

[`vox-package/src/artifact_cache.rs`](../../../crates/vox-package/src/artifact_cache.rs)
is already SHA3-512 content-addressed. Lifting workflow code into the same
namespace (P2-T1) means: a workflow at version A and version B coexist
without conflict. New deploys never break in-flight runs. Replay always
finds the right code by hash. Workers cache by hash; cold-start fetch is
content-addressed dedup. This is Unison's headline feature, available
because the substrate already exists.

### 4.5 Lock-leader on vox-db unblocks two daemons in one PR

The most-likely-most-severe failure mode of Vox today is "two daemons
silently race on the same file lock." (Risk #1 in
[research §4.2.9](mesh-dashboard-and-distributed-compute-research-2026.md).)
Persisting locks to vox-db + a heartbeated leader row (P0-T1, P0-T2) closes
it. Two PRs. No new dependencies. Coordination overhead = one A2A
round-trip per lock op when leader is remote, sub-millisecond when local.
After this, "two daemons, multi-agent, same repo, no data loss" is the
default.

### 4.6 `@remote` obsoletes the `mesh_*` naming convention

Vox's distributed dispatch today is keyed on a function-name prefix
(`mesh_*`). This is fragile, untyped, invisible to the compiler. A
single-annotation change (P1-T3) makes distribution a typed concept:
`@remote fn` → effect row gains `Spawn` + `Net`; args must be serializable;
return is `DurablePromise[T]`; orchestrator dispatches via populi. The
typo class "I forgot the `mesh_` prefix and now my code runs locally and
deadlocks" becomes a compile-time error.

### 4.7 `vox workflow preview` converts runtime questions to compile-time

Distributed programs have one terrifying property: you can't tell what
they'll do without running them. `vox workflow preview <fn>(args)` (P1-T8)
type-checks, infers effects, and projects the schedule of activities that
*would* run, without doing them. For LLM authors this is the single
highest-leverage tool — they can self-verify before dispatch. Generalizes
in Phase 2 to `vox dispatch preview` (orchestrator routing) and `vox mesh
preview` (mesh effects). Operators run all three before touching production.

### 4.8 Op-log projections — one source of mesh state

Locks today live in a separate map. File affinity in another. Capability
ledger in a third. Kudos in a fourth. Each has its own sync protocol when
we go multi-node — *or* it doesn't and silently desyncs.

Every one of those is a *projection* of the op-log. Persist the log
(P3-T1), sign it (P3-T2), gossip it (P3-T3), define each projection as a
fold over the log (P3-T9). Restart replays the log; reconstructs every
table. Distribution sync becomes anti-entropy on op-IDs. **One protocol,
infinite views.**

### 4.9 Donations as a Vox file

Donation policy as a JSON blob in vox-db is the obvious shape. The
non-obvious win: make it a `donations.vox` file (P4-T3) — first-class Vox
source, type-checked by the compiler, version-controlled in git. Effect
rows + capability requirements + slot limits expressed in the same
language as the workloads they gate. Dashboard reads/writes it; CLI
edits it; agents reason about it. Once the language is good enough, the
language is the configuration too.

### 4.10 The "one envelope, two birds" attestation + kudos primitive

Worker signs `(task_id, input_hash, output_hash, gpu_seconds,
trace_blake3)` with a per-job ephemeral key (P5-T4). That single envelope
is **simultaneously** the attestation (submitter spot-checks) and the
kudos credit (ledger entry). One primitive ships two features. The
volunteer-compute story snaps into focus: every task earns a verifiable,
non-repudiable receipt that doubles as the contribution ledger entry.

---

## 5. Cross-cutting concerns

### 5.1 Telemetry namespace contracts

Each new primitive emits structured telemetry under a stable prefix:

- `vox.mesh.*` — transport, lease, dispatch, gossip
  ([populi-mesh-local-observability-spec-2026.md](populi-mesh-local-observability-spec-2026.md))
- `vox.workflow.*` — lifecycle, journal, replay, signal, drain
- `vox.activity.*` — invocation, hit/miss, dedup, retry
- `vox.vcs.*` — lock, op-fragment, capability, sign, verify
  ([git-concurrency-policy.md](git-concurrency-policy.md))
- `vox.crypto.*` — pairing, JWE, attestation, signature

Constants live in [`vox-telemetry`](../../../crates/vox-telemetry/) per
[telemetry-unification-design-2026.md](telemetry-unification-design-2026.md).
Each new event must include `trace_id` continuation and `peer_id` for
mesh-crossing events. No span name is a free string — every span name is a
constant in `vox-telemetry/src/types.rs`.

### 5.2 Migration policy

**Single-node users continue to work through every phase.**

- Phase 0–1: net-new behavior is opt-in via `[mesh]` config keys; defaults
  preserve current behavior.
- Phase 2: workflow content-hashing is enabled by default but old workflows
  without stable hashes get an auto-derived one with a deprecation warning.
- Phase 3: oplog persistence is on by default; gossip topic is opt-in
  per cluster.
- Phase 4: dashboard mesh routes light up incrementally; old fixtures stay
  behind a feature flag for one release.
- Phase 5: TLS becomes the default for non-loopback bind; `--insecure` flag
  required for HTTP off-loopback (and emits a warning).
- Phase 6: federation is opt-in; nothing leaves your machine until you
  publish an attestation.

Each phase ships behind a feature flag in
`contracts/orchestration/feature-flags.v1.yaml` until acceptance is met,
then the flag is removed.

### 5.3 Documentation obligations per change

Every PR that lands a task in this plan must:

1. Update [where-things-live.md](where-things-live.md) if it adds a new
   concept-to-crate mapping.
2. Add an ADR in `docs/src/adrs/` if it changes a load-bearing decision
   (consensus model, signature scheme, on-wire format).
3. Update this SSOT with the actual landed shape (file paths, accepted
   trade-offs) so future readers see the current state, not the planned
   one.
4. Cite the task ID (`P3-T2` etc.) in commit message and PR description.
5. Land a failing test first per
   [test-driven-development](../../../AGENTS.md), then the implementation.

Auto-generated docs (SUMMARY.md, architecture-index.md, feed.xml,
*.generated.md, .cursorignore) **are not hand-edited** — re-run the
generator (`vox-doc-pipeline`) per [feedback memory](../../../CLAUDE.md).

### 5.4 Anti-goals (binding; reject if proposed)

Re-stated from §0 with one-line rationale per:

- **Custom crypto.** Use `vox-crypto`. New crypto is a multi-quarter
  audit liability we cannot afford.
- **Blockchain / token economy.** Cost curve and governance burden wrong
  for power-user dogfood.
- **TEE-first.** No consumer hardware; build the *interface*.
- **Onion routing.** Wrong threat model; latency cost fatal.
- **Transitive web-of-trust.** UX intractable; reputation as signal,
  not capability.
- **Public SaaS multi-tenant control plane.** Discovery is opt-in p2p.
- **Dashboard as code editor.** Code surface is a viewer.
- **`.ps1` / `.sh` / `.py` automation glue.** Scripts are `.vox`.

### 5.5 Vox-db migration policy (canonical)

The canonical schema-evolution mechanism for `vox-db` is the **`BASELINE_VERSION` constant** in
`crates/vox-db/src/schema/manifest.rs`, governed by
[`contracts/db/baseline-version-policy.yaml`](../../../contracts/db/baseline-version-policy.yaml).
Every phase that adds a new table, column, or index MUST:

1. **Bump `BASELINE_VERSION`** by one (from current to current+1).
2. **Add the new schema fragment** as Rust DDL inside the manifest, gated on the new version.
3. **Provide a forward migration** (synchronous; on-startup `vox-db` runs all migrations < target).
4. **Cite the new version number** in the PR description and in the Notes column of the relevant
   phase task table.

What this plan **rejects**:

- Date-stamped SQL files under any path (`crates/vox-db/src/migrations/YYYYMMDD_*.sql`).
- Numeric SQL files under top-level `crates/vox-db/migrations/00NN_*.sql`.
- Any out-of-band schema mutation that bypasses the baseline-version manifest.

Phase plans whose drafts proposed alternative schemes are corrected to follow `BASELINE_VERSION`:

- `P2-T5` (activity result cache): bump from current to current+1; schema fragment lives in the
  manifest, not in a date-stamped SQL file.
- `P3-T1` (convergence op-log): bump again from P2's value; not a `0042_*.sql` file.
- `Hp-T5` (hopper inbox, when Option B lands): bump from P3's value.

Each phase plan's File map and substeps must be edited to reflect this canonical policy when
those plans are next revised.

### 5.6 Dashboard route convention

REST routes mount under `/api/v2/<surface>/<resource>` (versioned, plural-nouns). WebSocket
subscriptions mount under `/v1/ws/<topic>` — the dashboard ships ONE WS upgrade endpoint and
multiplexes typed sub-channels by message envelope. This is the canonical convention; new
surfaces follow it.

Hopper concrete routes:

- REST: `/api/v2/hopper/{inbox, assigned, history, submit, reprioritize, start_batch}`
- WS topic: `/v1/ws` with `topic: "hopper"` envelope (NOT a separate `/api/v2/hopper/events`
  endpoint — the P4-T13 draft was inconsistent here and is corrected on its next revision).

Mesh, oplog, vcs, and other surfaces follow the same rule.

### 5.7 Audit-log signing surface (canonical sinks)

All capability-token mints and all developer-overridable actions emit a signed audit-log entry
through the same writer introduced in `P4-T7` (the `audit_log.rs` writer that signs with the
daemon's `vox-secrets`-issued Ed25519 key per `P3-T2`). This is the single sink for:

- Capability mints (`WorkingTreeWrite`, `BranchCreate`, `PushAllowed`, `ForcePushAllowed`,
  `DestructiveOp`, `DeveloperOverride`).
- Destructive dashboard actions (kill, pause, drain, replay).
- Hopper reprioritizations that mint `DeveloperOverride` (the `P4-T13` draft routed these
  through bare event emission; corrected on its next revision to flow through `audit_log.rs`).
- Op-log entries that include capability mints (already wired via `P3-T2`).

Anything that should be auditable across nodes via gossip (Option C / P6-T9) MUST flow through
this writer; anything emitted only as a `tokio::broadcast` event without signing is dashboard-UI
state, not audit history.

---

## 6. Top-10 risk register (likelihood × severity)

Ordered by `L × S`. For each: smallest fix, where in the plan it's
addressed.

| # | Risk | L | S | Smallest fix | Plan task |
|---|---|---|---|---|---|
| 1 | Two daemons race the same file lock; wrong-branch commits stack silently | High | High | Persist locks to vox-db + lock-leader election | `P0-T1`, `P0-T2` |
| 2 | Dispatch falls back to local even when remote node is mid-task; duplicate execution | High | High | Consult lease state before fallback | `P0-T3` |
| 3 | Capability token forged from another crate via `pub fn mint` | Med | Very High | Sealed-trait hardening | `P3-T6` |
| 4 | JWE decrypt happens but secrets never reach exec context — task fails opaquely | High | Med | Inject decrypted secrets into task env per `@uses(secret)` | `P0-T4` |
| 5 | Bare HTTP off-LAN leaks tokens, payloads, attestations | Med | Very High | TLS / WireGuard sidecar | `P0-T5` |
| 6 | Hardware probes lie; planner routes to overcommitted GPU | Med | High | Probe trait + mock harness; operator-label vs probe reconciliation | `P0-T6` |
| 7 | Workflow contains `time.now()` → replay drift → silent corruption | High | Med | Determinism check at compile time | `P1-T5` |
| 8 | Activity dedup key changes after refactor → re-execution → external side effect duplication | High | Med | Auto-derived `activity_id` from span+arg-hash | `P1-T4` |
| 9 | Op-fragment with unknown parent stalls forever (producer offline) | Med | High | Bounded queue + DLQ to vox-db; surface in dashboard | `P3-T8` |
| 10 | Force-push-ish op via banned-list bypass (raw `Command::new("git")`) | Low | Very High | Arch-check rule | `P3-T7` |
| 11 | Hopper admission policy starves queues; system *looks* hung | Med | High | CLI fallthrough (`vox task submit --bypass-hopper`); dashboard surfaces inbox depth | `Hp-T1` |
| 12 | Single-intake hopper = single point of failure; hopper crash drops in-flight intake | Low | High | Persist inbox (Option B) once P3-T1 lands; CLI fallthrough always available | `Hp-T5` |
| 13 | Submitter-side spot-check sampling at p=0.01 cannot meet >99% sensitivity over 100 jobs (math: 1-(0.99)^100 ≈ 63%, not 99%); raise default to p=0.05 | High | Med | Default p=0.05 in `[mesh.attestation.spot_check_rate]`; 100-job acceptance updated to read "≥99% over 100 jobs at p=0.05" | `P5-T5` |
| 14 | Three phase plans drafted three different vox-db migration schemes (`BASELINE_VERSION` bump vs date-stamped SQL vs numeric SQL); landing them as drafted produces a confused schema-evolution story | Med | High | §5.5 canonical policy; phase plans updated in their next revision | `P2-T5`, `P3-T1`, `Hp-T5` |

---

## 7. Release acceptance contracts

Three pinned releases between today and v1.0; each has a precise definition
of what's shippable.

### 7.1 v0.6 — "single-machine multi-agent, no data loss"

**Phases:** Phase 0 + Phase 1 (T1–T5) + Phase 3 (T1–T4) + Hp-T1..Hp-T4 (single-machine hopper, Option A)

**The user-visible promise.** "Run multiple agents on one machine; no
silent data loss; workflows are deterministic-by-construction; no more
non-deterministic replay drift."

**Acceptance check.**

- Two agents on the same machine forced into a lock conflict on the same
  file → no double-write, deterministic queueing.
- A workflow that calls `time.now()` directly fails `vox check`.
- A workflow that uses `DurablePromise[T]` survives kill-9 → restart →
  resumes from journal.
- The op-log persists; restart reconstructs every projection.

### 7.2 v0.7 — "two-daemon LAN mesh, durable distributed workflows"

**Phases:** Phase 0 complete + Phase 1 complete + Phase 2 (T1–T5) +
Phase 3 (T5–T9) + Phase 4 (T1–T7) + Hp-T5..Hp-T8 (persistent hopper, Option B)

**The user-visible promise.** "Spin up a personal mesh in 5 minutes from
the dashboard. Distribute durable workflows across your boxes without
worrying about version skew, idempotency, or partial failure. Watch every
edit and dispatch in the dashboard's audit log."

**Acceptance check.**

- "Personal mesh in 5 minutes" works end-to-end.
- A workflow at version A and refactored version B coexist in vox-db.
- `@remote fn` dispatches via populi; non-serializable args fail to
  compile.
- Op-fragments gossip across two daemons in ≤ 30 s.
- The hopper accepts intake from a single chat surface; the dashboard surfaces a cross-agent global view; reordering a task emits TaskReprioritized; orchestrator restart replays the hopper inbox without loss.

### 7.3 v1.0 — "internet-facing personal mesh"

**Phases:** v0.7 + Phase 5 complete + Phase 6 (T1–T2)

**The user-visible promise.** "Pair your mesh with a vetted (GitHub-attested)
peer. Share compute. Earn auditable kudos. Revoke trust at any time."

**Acceptance check.**

- Fresh public mesh node refuses unpaired peers; accepts paired peers
  with valid GitHub attestation; respects revocation within ≤ 60 s.
- Per-key quota fuse fires under fuzz before resource exhaustion.
- Result attestation verifies under spot-check at > 99% sensitivity.
- Kudos ledger reconciles to within ε.

### 7.4 v1.x — the grand network

**Phases:** Phase 6 complete

**The user-visible promise.** "Two strangers with verified GitHub identities
share compute through their personal meshes. Auditable. No central server.
No token. No SaaS dependency."

**Acceptance check.**

- Two unrelated contributors successfully share compute, with kudos
  reconciliation, redundant-execution voting on a deterministic batch
  job, and Scientia-published trust-graph snapshots.
- Mesh-replicated hopper (Option C) inbox replicates via op-log gossip; reprioritization on node A converges to node B within ≤30s.

---

## 8. Open questions for Wave-3 research

Carried forward from the research synthesis. Each blocks at least one task
in the plan above and should be answered before that task ships.

1. **Sandbox for VoxScript with mesh effects.** WASM capability propagation
   is clean for in-process; what's the right shape when the WASM module
   dispatches a remote activity? Where does the per-job ephemeral key live?
   *(Blocks P5-T6.)*

2. **Workflow signal typing.** `Signal[T]` with type information vs
   string-keyed for cross-language interop?
   *(Blocks signal sugar in Phase 1; can ship strings first, type later.)*

3. **`Future[T]` deprecation compat.** Single canonical primitive (C4)
   demands removing `Future[T]` from std — what's the compat story for
   non-distributed callers? One-release deprecation window with auto-rewrite
   diagnostic, or longer?
   *(Blocks P1-T2.)*

4. **Tombstone propagation latency.** Acceptable bound for personal mesh;
   need numbers for public-internet (ranged from "minutes" to "hours" in
   prior art).
   *(Blocks P5-T2 acceptance criteria.)*

5. **Lock-leader split-brain on partition heal.** When two leaders briefly
   co-exist, how do their op-logs reconcile? Same answer as op-fragment
   unknown-parent, or is there a leader-specific case?
   *(Blocks P0-T2 finalization.)*

6. **Dashboard-as-orchestrator-extension audit fields.** When the dashboard
   surfaces "drag-to-assign-role" with capability-token semantics, does
   the mint API need a `via_dashboard: bool` audit field?
   *(Blocks P4-T7 — the `⌘K` palette ergonomics.)*

7. **Mesh-aware `vox audit` umbrella.** Per
   [tooling-convergence-findings-2026.md](tooling-convergence-findings-2026.md);
   should mesh-health (probe correctness, lock backlog, oplog lag,
   attestation freshness) join the same surface, or stay in
   `vox populi audit`?
   *(Blocks tooling-convergence Phase 3 work.)*

8. **Federation envelope shape compatibility.** Should op-fragment envelope
   adopt ForgeFed/ActivityPub Activity-object shape for forge-event
   federation, or stay pure-binary? The *shape* may be useful even if the
   transport is too verbose.
   *(Blocks P6-T1.)*

---

## 9. The orchestrator as a Vox program (long arc)

Beyond v1.x there's a structural goal worth naming: **the orchestrator
itself becomes expressible in Vox.**

Once Phase 1 lands `DurablePromise[T]`, `@remote`, effect rows, and
auto-derived `activity_id`, and Phase 2 lands content-addressed code, the
language is expressive enough to write distributed durable workflows in
Vox. The orchestrator's own routing logic, file-affinity learning, and
dispatch decisions are themselves a workflow — they touch
state, fan out activities (LLM calls, lock acquires, branch creates),
and need to survive crashes.

The migration path is gradual: each policy module in
[`vox-orchestrator/src/orchestrator_policy.rs`](../../../crates/vox-orchestrator/src/orchestrator_policy.rs)
becomes a Vox source file under `crates/vox-orchestrator/policies/*.vox`,
compiled and linked into the daemon. Hot-reload becomes possible; LLM
agents can propose policy changes that are reviewable as PRs against
typed Vox source. The Rust glue thins to the activity-and-effect bindings.

This is dogfooding at the largest scale Vox has — the orchestrator runs
its own language. Not a v1.0 goal; a direction-of-travel marker.

---

## 10. Spawn list (downstream plan documents to author)

When this SSOT is approved, the following plan documents decompose its
phases. Each is an author-able artifact in
`docs/src/architecture/`.

- [mesh-phase0-foundations-plan-2026.md](mesh-phase0-foundations-plan-2026.md) — P0-T1..T8 (landed 2026-05-09)
- [mesh-phase1-language-spine-plan-2026.md](mesh-phase1-language-spine-plan-2026.md) — P1-T1..T9 (landed 2026-05-09)
- [mesh-phase2-code-mobility-plan-2026.md](mesh-phase2-code-mobility-plan-2026.md) — P2-T1..T7 (landed 2026-05-09)
- [mesh-phase3-vcs-gossip-plan-2026.md](mesh-phase3-vcs-gossip-plan-2026.md) — P3-T1..T9 (landed 2026-05-09; merges with multi-agent-vcs-replication-impl-plan-phase1-2026.md)
- [mesh-phase4-dashboard-control-plan-2026.md](mesh-phase4-dashboard-control-plan-2026.md) — P4-T1..T12 + Hp-T6 hopper panel (landed 2026-05-09)
- [mesh-phase5-public-internet-plan-2026.md](mesh-phase5-public-internet-plan-2026.md) — P5-T1..T10 (landed 2026-05-09)
- [mesh-phase6-grand-network-plan-2026.md](mesh-phase6-grand-network-plan-2026.md) — P6-T1..T8 (landed 2026-05-09)
- [mesh-mens-distributed-training-and-execution-plan-2026.md](mesh-mens-distributed-training-and-execution-plan-2026.md) — Mn-T1..Mn-T15 cross-cutting (landed 2026-05-09)
- `unified-task-hopper-spec-2026.md` — Hp-T1..Hp-T9 narrowed spec (deferred; author when hopper work is queued)
- `unified-task-hopper-impl-plan-phase1-2026.md` — TDD plan (deferred)

Each follows the established TDD-plan template (e.g.,
[agentic-vcs-automation-impl-plan-phase1-2026.md](agentic-vcs-automation-impl-plan-phase1-2026.md)):
goal, file-by-file changes, test-first ordering, acceptance, rollback
plan.

---

## 11. Crosslinks

### 11.1 Authoritative companions

- [mesh-dashboard-and-distributed-compute-research-2026.md](mesh-dashboard-and-distributed-compute-research-2026.md)
  — research synthesis; this SSOT consolidates and supersedes the design
  recommendations there. The research doc remains canonical for prior-art
  citations and threat model discussion.
- [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md)
  — five-phase language enforcement plan; Phase 1 of *this* plan implements
  Phase 5 of *that* plan (effects to errors).
- [populi-mesh-north-star-2026.md](populi-mesh-north-star-2026.md) —
  three-slice (S1/S2/S3) capability roadmap. This SSOT integrates S1–S3
  into Phases 0–5.
- [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md) — design space
  for the cross-cutting hopper track integrated as §3.5.

### 11.2 Mesh

- [populi-mesh-improvement-backlog-2026.md](populi-mesh-improvement-backlog-2026.md)
- [populi-mesh-a2a-durability-spec-2026.md](populi-mesh-a2a-durability-spec-2026.md)
- [populi-mesh-config-baseline-spec-2026.md](populi-mesh-config-baseline-spec-2026.md)
- [populi-mesh-local-observability-spec-2026.md](populi-mesh-local-observability-spec-2026.md)
- [populi-mesh-probe-correctness-spec-2026.md](populi-mesh-probe-correctness-spec-2026.md)
  / [-plan](populi-mesh-probe-correctness-plan-2026.md)
- [scientia-mesh-integration-research-2026.md](scientia-mesh-integration-research-2026.md)

### 11.3 Dashboard

- [vox-dashboard-design-brief-2026.md](vox-dashboard-design-brief-2026.md)
- [dashboard-migration-research-2026.md](dashboard-migration-research-2026.md)
- [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md)

### 11.4 Durability & language

- [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md)
- [vox-language-rules-phase1-ssot-collapse-2026.md](vox-language-rules-phase1-ssot-collapse-2026.md)
- [feature-growth-boundaries.md](feature-growth-boundaries.md)
- [v0.5-core-ssot.md](v0.5-core-ssot.md)
- [nextgen-orchestrator-research-2026.md](nextgen-orchestrator-research-2026.md)
- [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md)
- [mesh-mens-distributed-training-and-execution-plan-2026.md](mesh-mens-distributed-training-and-execution-plan-2026.md) — distributed-AI track (Mn-T1..Mn-T15)

### 11.5 Multi-agent VCS

- [multi-agent-vcs-replication-research-2026.md](multi-agent-vcs-replication-research-2026.md)
- [multi-agent-vcs-replication-spec-2026.md](multi-agent-vcs-replication-spec-2026.md)
- [multi-agent-vcs-replication-impl-plan-phase1-2026.md](multi-agent-vcs-replication-impl-plan-phase1-2026.md)
- [agentic-version-control-automation-research-2026.md](agentic-version-control-automation-research-2026.md)
- [agentic-vcs-automation-impl-plan-phase1-2026.md](agentic-vcs-automation-impl-plan-phase1-2026.md)
  / [phase2](agentic-vcs-automation-impl-plan-phase2-2026.md)
  / [phase3](agentic-vcs-automation-impl-plan-phase3-2026.md)
  / [phase4](agentic-vcs-automation-impl-plan-phase4-2026.md)
  / [phase5](agentic-vcs-automation-impl-plan-phase5-2026.md)
- [git-concurrency-policy.md](git-concurrency-policy.md)

### 11.6 Identity, security, telemetry

- [cryptography-ssot-2026.md](cryptography-ssot-2026.md)
- [share-policy-2026.md](share-policy-2026.md)
- [ludus-identity-github-integration-research-2026.md](ludus-identity-github-integration-research-2026.md)
- [ludus-security-and-anti-cheat-research-2026.md](ludus-security-and-anti-cheat-research-2026.md)
- [telemetry-trust-ssot.md](telemetry-trust-ssot.md)
- [telemetry-unification-design-2026.md](telemetry-unification-design-2026.md)
- [tooling-convergence-findings-2026.md](tooling-convergence-findings-2026.md)

### 11.7 Application surfaces (load-test consumers)

- [fableforge-roadmap-audit-2026-04-23.md](fableforge-roadmap-audit-2026-04-23.md)
  — visual-novel platform; primary consumer of `@remote` for image/text gen
- [ludus-adjudication-implementation-plan-2026.md](ludus-adjudication-implementation-plan-2026.md)
  — multi-agent collegium; primary consumer of trust ledger + reputation
- [scientia-self-publication-finalization-plan-2026.md](scientia-self-publication-finalization-plan-2026.md)
  — discovery feedback loop; primary consumer of mesh telemetry

### 11.8 Architectural reference

- [where-things-live.md](where-things-live.md) — concept-to-crate lookup
- [layers.toml](layers.toml) — layer rules + forbidden_deps
- [phase-numbering-index.md](phase-numbering-index.md) — phase-ID dictionary
- [v1-release-criteria.md](v1-release-criteria.md) — v1.0 acceptance
