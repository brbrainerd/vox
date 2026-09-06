---
title: "Populi Mesh North-Star (2026-05-01)"
description: "Design intent and capability-slice plan for taking the Populi mesh from single-node-correct to multi-node power-user dogfood. Decomposes seven workstreams into three sequenced slices with a child-spec roadmap."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Architectural plan-of-record for Populi mesh evolution; names the workstreams, slices, and child specs that subsequent work decomposes into."
---

# Populi Mesh North-Star

**Scope.** The Populi mesh subsystem: `vox-populi` (registry, HTTP control plane, A2A inbox, mens training stack, hardware probes, cloud providers), `vox-mesh-types` (shared records), and the orchestrator paths that drive it (`vox-orchestrator::a2a::dispatch::mesh`, `vox-orchestrator::a2a::remote_worker`, `vox-orchestrator::a2a::jwe`). Out of scope: agent surface (`vox-skills`), pricing (`vox-orchestrator::models`), vox-secrets resolver internals.

**How to read this.** This is a north-star, not an implementation plan. It declares the seven workstreams the mesh needs to converge on, organizes them into three capability slices that ship usable mesh at increasing scale, and points to the child specs each slice decomposes into. Companion document: [`populi-mesh-improvement-backlog-2026.md`](populi-mesh-improvement-backlog-2026.md) — the flat tagged list of everything not load-bearing enough to deserve its own spec.

**Ambition level.** *Power-user dogfood.* A vox contributor with 2–4 personal boxes can run the mesh, trust it, debug it, and recover from failure. Not enterprise-grade auth, not multi-tenant, not autoscaling. See §7 for what that means concretely.

---

## Part 1 — Problem statement

### What ships and works

- **Single-node** mesh: `vox populi serve`, registry, bearer-auth HTTP control plane, A2A inbox with idempotency and lease semantics, mens training in-process. ADR-008 is fully implemented.
- **Crypto primitives** are present: Ed25519 identity ([`vox-identity/src/identity.rs`](../../../crates/vox-identity/src/identity.rs)), X25519 sealed-box ([`vox-crypto/src/facades.rs:189`](../../../crates/vox-crypto/src/facades.rs)), JWE compact encrypt/decrypt ([`vox-orchestrator/src/a2a/jwe.rs:29`](../../../crates/vox-orchestrator/src/a2a/jwe.rs)), vox-secrets resolver chain.
- **Type backbone** (`vox-mesh-types`) is stable: NodeRecord, A2ADeliverRequest, ExecLeaseGrant, WorkerDonationPolicy, TaskCapabilityHints. All field changes have been additive.

### What is broken or pretending

1. **Remote execution is not authoritative.** ADR-017 says leases own task identity and local fallback fires only on lease failure. Shipped behavior is local-first, with mesh dispatch as a best-effort relay. The skill descriptions tell agents not to assume remote execute. Until this is fixed, "the mesh" is closer to a debug visualization than a runtime.
2. **GPU truth is operator-asserted.** ADR-018 specifies a probe-driven hardware-truth layer that outranks operator labels. Today, [`node_record_for_current_process()`](../../../crates/vox-populi/src/node_registry.rs) reads `VOX_MESH_ADVERTISE_*` env vars; NVML / wgpu / DRM / Metal probes exist but are not validated against any test, and there's no admission control that uses them.
3. **JWE secret pipe is half-built.** Encryption-on-send works for tasks whose `capability_requirements_json` declares secret needs ([`a2a/dispatch/mesh.rs:71-99`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs)). Storage/transport carries `jwe_payload` end-to-end. But `decrypt_jwe_compact` is **defined and never called**. No worker decrypts the secrets. No device-pairing flow installs the X25519 keys that would make this work in the first place.
4. **Model discovery is one-shot.** No scheduled refresh, no mesh-wide aggregation, no `vox populi models inventory`. Checkpoints trained on box B are invisible to a planner running on box A unless an operator manually re-registers them.
5. **Observability stops at process boundary.** No distributed trace propagation across mesh-delivered tasks. Failures on a remote node manifest as opaque A2A errors without correlation back to the originating journey/task.
6. **Test coverage is thin where it matters.** Hardware probes (NVML, wgpu, DRM, Metal) have no inline unit tests; cloud providers (RunPod, Vast) have no test files at all; lease renewal/expiry untested because not yet shipped; A2A durability tested only against the in-memory store.
7. **Operator UX assumes a developer.** ~10 `VOX_MESH_*` env vars to learn, no `vox populi join` quickstart, admin commands buried behind feature flags.

### The "good enough" target

A user who:
- runs `vox populi join <peer-url>` once on each box, and
- runs `vox populi pair <peer-id>` once between any two boxes that need to share secrets,

…can then submit work from any box, watch it route to the right hardware based on real probe data, follow a single trace ID across hops, and recover predictably when a node disappears mid-task. That's the bar.

---

## Part 2 — Seven workstreams

These are the themes; each is a charter, not a plan. Each gets its own child spec when it reaches the front of the queue (§9).

### W1 — Authoritative leases (ADR-017 implementation)

**Problem.** Local-first execution shadows mesh dispatch. There's no authoritative "this task belongs to node X for the next N seconds" claim. Lease grant types exist in `vox-mesh-types` but are not consulted by the orchestrator's dispatch loop.

**Charter.** Make leases the source of truth for task ownership. Local fallback is explicit and observable, not silent. Renewal and revocation are first-class. The `exec-lease-revoke` admin command actually unblocks a stuck task. Done means: a task in flight on a remote node and a duplicate submission attempt on the originator both see the same `lease_id`, and revoking the lease causes a clean handover.

**Cost driver.** This rewires the dispatch path. Touches `vox-orchestrator::a2a::dispatch`, `vox-orchestrator::orchestrator`, `vox-populi::transport::handlers`, the executor pool, and the persistence schema. Test surface dominated by integration tests across two in-process nodes.

### W2 — GPU truth probes (ADR-018 implementation)

**Problem.** Probes exist but their output is not authoritative. No NVML mocking, no probe-replay test. Routing decisions can't trust what `node_record_for_current_process()` returns, so they fall back to env vars.

**Charter.** Probe output (Layer A, "what does the driver say") and capacity calculation (Layer B, "what fraction is allocatable after current usage") are reliable enough to be the routing input. Operator labels (Layer C) become hints, not advertisements. Done means: a node with no `VOX_MESH_ADVERTISE_*` flags advertises correct capacity; a node with conflicting flags has the probe data win on Layer A/B fields.

**Cost driver.** NVML / wgpu / DRM / Metal probe correctness on real hardware. Mock harnesses for CI. A capacity model that's right more often than wrong. Touches `vox-populi::mens::hardware::*`, `vox-populi::node_registry`, and the orchestrator's planner.

### W3 — Cross-node secret pairing

**Problem.** JWE encrypt-on-send is wired but no pairing flow distributes X25519 public keys, no worker decrypts, and `vox populi pair` doesn't exist.

**Charter.** Two boxes can complete a one-time pairing handshake that establishes mutual X25519 trust. After pairing, secrets named in a task's `capability_requirements` are wrapped to the receiving node's pubkey at dispatch and unwrapped on receipt. Rotation is supported (re-pair clears the old key). Done means: `OPENROUTER_API_KEY` installed only on box A is consumable by a task that runs on box B, with no operator action between dispatch and consumption.

**Cost driver.** New pairing storage (peer pubkey table per node), wiring decryption into the worker hot path, and key-rotation semantics. Adjacent to but not replacing the vox-secrets resolver chain.

### W4 — Mesh model discovery

**Problem.** Trained checkpoints, fetched HF models, and OpenRouter catalog snapshots are all single-process state. No "what models does the mesh know about" view.

**Charter.** Each node publishes its model inventory (HF cache, mens checkpoints, configured provider catalogs) on a poll. Aggregation lives in the orchestrator daemon and feeds `ModelRegistry::best_for()` so a planner can route to a node that has a model already loaded. Refresh is scheduled, not on-demand-only. Done means: training a LoRA on box B and immediately submitting an inference task from box A routes to box B without manual config.

**Cost driver.** New endpoint on the populi server (`GET /v1/models/inventory`), a new orchestrator poll job, and registry integration. Schema is additive.

### W5 — Cross-node observability

**Problem.** No distributed trace ID. `journey_id` / `session_id` / `run_id` are local. A remote-task failure shows up as an A2A error on the originator with no link to the receiver-side log.

**Charter.** Every mesh-bound task carries OpenTelemetry GenAI-conforming context (`trace_id`, `span_id`, `gen_ai.*` attributes). Receivers continue the trace. Spans land in the same telemetry table the local path uses. Done means: a single trace query returns the originator's submit span, the dispatch hop, the receiver's execute span, and the response hop, in order, with token/cost attribution.

**Cost driver.** Trace propagation in A2A envelope, GenAI semconv attribute coverage in worker code, and aligning with the existing telemetry-driven cost-accounting work (see [`telemetry-driven-cost-accounting-research-2026.md`](telemetry-driven-cost-accounting-research-2026.md)).

### W6 — Test coverage for the under-tested

**Problem.** NVML / wgpu / DRM / Metal probes have no inline tests. Cloud provider clients (RunPod, Vast) have no test files. Lease renewal logic is untested because not shipped. A2A durability tested only against in-memory store.

**Charter.** Probe modules expose a trait-shaped surface that admits a mock; mocks live in test-only modules. Cloud provider clients get recorded-fixture tests against captured API responses. Lease lifecycle (grant → renew → expire → revoke) gets a dedicated integration test pair. Durability gets a test against the JSON-file store *and* whatever durable backend W7 introduces. Done means: a `cargo test -p vox-populi` run exercises every probe code path and every cloud provider response shape.

**Cost driver.** Mostly mechanical. Largest cost is recording cloud fixtures without leaking credentials.

### W7 — Operator UX

**Problem.** Mesh setup is for people who already understand it. ~10 env vars, no quickstart, admin commands feature-flagged out of normal builds.

**Charter.** `vox populi join`, `vox populi pair`, `vox populi status`, `vox populi inventory` are first-class CLI verbs with helpful errors. A `Vox.toml [mesh]` block replaces most env-var configuration; env vars remain as overrides. The admin path (`maintenance`, `quarantine`, `exec-lease-revoke`) is discoverable in `vox populi --help` without feature flags. Done means: a contributor on a fresh box reaches "joined the mesh and ran a remote task" in under 10 minutes by following the quickstart, without reading `populi.md`.

**Cost driver.** CLI surface design, `Vox.toml` schema additions, error-message work, and a quickstart how-to in `docs/src/how-to/`.

---

## Part 3 — Capability slices

Three slices. Each ships an end-to-end usable mesh at a higher capability ceiling. Each draws on multiple workstreams at varying depth — no slice fully completes any workstream by itself, but every slice produces a mesh that's better than what came before.

### Slice S1 — Single-machine baseline rock-solid

**User capability gained.** A single box runs `vox populi serve` and trusts every probe value, every test, every error message. This is the foundation that S2 and S3 build on; if S1 isn't solid, multi-node correctness is impossible to diagnose.

**What lands.** Hardware probe correctness with mock-driven tests (W2/W6 partial). Trace propagation inside the local path so single-node debugging benefits from the same observability the multi-node path will need (W5 partial). Operator UX baseline: `vox populi serve` defaults sensibly without env vars, errors are actionable, `Vox.toml [mesh]` schema lands (W7 partial). A2A durability gets one durable backend option past the in-memory store (W6 partial).

**Definition of done.**
- `cargo test -p vox-populi` exercises every hardware probe via mock and every A2A storage backend.
- A failing probe surfaces a structured error with a `secrets doctor`–style remediation hint.
- A local task carries an OTel trace_id from submit to completion.
- `vox populi serve` runs without setting any `VOX_MESH_*` env var.
- Quickstart how-to in `docs/src/how-to/populi-quickstart.md` walks a contributor from clone to serve.

### Slice S2 — Two-node correct

**User capability gained.** Two boxes paired, with authoritative leases, real GPU-aware routing, and traces that cross the hop. This is the slice that makes "the mesh" mean what the docs claim it means.

**What lands.** Full ADR-017 lease semantics: grant, renew, expire, revoke, with explicit local fallback (W1). Full ADR-018 admission control: probe data outranks env-var advertisements; planner consults real capacity (W2). Cross-node secret pairing: `vox populi pair`, X25519 trust, JWE decrypt on the worker side (W3). Trace propagation across A2A hops (W5). Lease lifecycle integration tests; pairing handshake tests; under-load A2A persistence tests (W6).

**Definition of done.**
- A task submitted on box A executes on box B under a lease whose ID is visible on both nodes.
- Revoking the lease on box A causes box B to abort and box A to retry locally with a clean error path.
- Box B can consume a secret installed only on box A, after a one-time `vox populi pair`.
- The trace for any cross-node task spans both nodes' telemetry tables and links via `trace_id`.
- The planner refuses to dispatch a 12 GB job to a box that the NVML probe reports as having 8 GB free, regardless of `VOX_MESH_ADVERTISE_GPU` value.

### Slice S3 — N-node operable

**User capability gained.** Three or four boxes, with a model inventory the mesh agrees on, key rotation, and admin tools that work without a feature flag. This is where the mesh becomes routine to live with.

**What lands.** Mesh model discovery: scheduled inventory refresh, aggregation in the orchestrator daemon, planner integration (W4). Pairing-key rotation and revocation (W3 completion). Cloud provider client fixture tests and budget-watchdog under load (W6 completion). Operator UX completion: `vox populi inventory`, `vox populi status` rich output, admin commands out of feature flags, `Vox.toml [mesh]` covers everything env vars do (W7 completion).

**Definition of done.**
- Training a LoRA on any box of an N-node mesh and immediately running inference from any other box routes to the producing box without operator action.
- A pairing key rotation propagates within one poll cycle and old keys stop decrypting new envelopes.
- `vox populi --help` lists `maintenance`, `quarantine`, `exec-lease-revoke`, and `inventory` in the default build.
- Cloud provider tests run in CI against recorded fixtures with no live API calls.
- A node leaving the mesh (graceful or hard) is reflected in inventory within 2× poll interval and routing skips it.

---

## Part 4 — Slice × workstream contribution matrix

Read across a row to see how a workstream is built up over slices. Read down a column to see what a slice draws on. Cell content: the slice-specific contribution. Empty cell means that slice doesn't touch that workstream.

| Workstream | S1 (single-node) | S2 (two-node) | S3 (N-node) |
|------------|------------------|---------------|-------------|
| W1 leases  | — | grant/renew/expire/revoke + explicit fallback | (stable, no slice work) |
| W2 GPU truth | probe correctness + mocks | admission control consumes probes | (stable, no slice work) |
| W3 secret pairing | — | `vox populi pair` + decrypt on worker | rotation + revocation |
| W4 model discovery | — | — | scheduled inventory + aggregation + planner integration |
| W5 observability | local OTel trace_id propagation | cross-node trace propagation | (stable; no slice work) |
| W6 test coverage | probe mocks + A2A durability tests | lease lifecycle + pairing + multi-node integration | cloud fixtures + N-node soak |
| W7 operator UX | sensible defaults + `Vox.toml [mesh]` baseline + quickstart | `pair`, `status` improvements, helpful errors | `inventory`, admin out of feature flags, full `Vox.toml [mesh]` parity |

**Implication.** S1 is dominated by W2/W5/W6/W7 *partial*. S2 is dominated by W1/W2/W3/W5 *full*. S3 is dominated by W4 *full* + completions. No workstream lands in a single slice; every workstream lands across at least two.

---

## Part 5 — Sequencing & dependencies

```
S1 (single-node)
  ├─ blocks: nothing (can start now)
  └─ unblocks: S2 (probe correctness gates admission control;
                    durable A2A storage gates lease persistence;
                    OTel local gates cross-node trace propagation)

S2 (two-node)
  ├─ blocks on: S1 complete
  └─ unblocks: S3 (lease semantics gate inventory propagation
                    over leases; pairing gates rotation;
                    cross-node traces gate N-node soak)

S3 (N-node)
  └─ blocks on: S2 complete
```

**Hard dependencies inside slices.**

- W1 leases require W2 admission control to be useful, but admission control depends on probe correctness from S1. Therefore W2's probe work must finish before S2 starts; admission integration happens inside S2.
- W3 pairing decrypt depends on W3 pairing handshake; both land in S2 as a unit.
- W5 cross-node propagation requires the local OTel work in S1.
- W4 inventory is intentionally deferred to S3 because the inventory poll itself is a remote-execution-shaped concern: it benefits from the lease/observability foundation S2 provides.

**What can run in parallel inside a slice.** Probes (W2), durability (W6 storage), and operator UX (W7) within S1 are independent. Pairing (W3) and lease implementation (W1) within S2 are independent up to integration. Inventory (W4) and rotation (W3 finalization) within S3 are independent.

---

## Part 6 — Cross-cutting concerns

These rules apply to every workstream and every slice. They go in the child specs by reference.

### 6.1 Backward compatibility

Power-user dogfood means **BC can break with notes**. Rules:

- Type changes in `vox-mesh-types`: additive only when feasible. Breaking field changes require a CHANGELOG entry under "BREAKING" and a migration note in `docs/src/operations/`.
- HTTP control plane (the ADR-008 surface): new endpoints fine; existing endpoints additive only. Removal/breaking change requires an ADR amendment.
- Persistence schema changes (`vox-populi::store`, A2A inbox): forward-compat read with a versioned envelope; no in-place mutation that can't be reverted.
- Config: env vars stay supported until the `Vox.toml [mesh]` equivalent has been in a release. Then env vars are deprecated for one release before removal.

### 6.2 Security model

- **Trust boundary.** The mesh trusts paired peers (mutual X25519 + Ed25519) and nothing else. Bearer auth (`VOX_MESH_TOKEN`) is for control-plane access, not for cross-node trust.
- **Secret handling.** Secrets cross the mesh only as JWE-wrapped payloads keyed to the recipient's pairing pubkey. Plaintext secrets in A2A envelopes are a security defect.
- **Audit.** Every lease grant, every pairing operation, every secret unwrap emits a structured event consumable by `vox telemetry`.
- **Out of scope at this ambition.** OIDC / SSO, multi-tenant isolation, hardware-attestation-based trust. ADR amendments needed if these come back into scope.

### 6.3 Observability conventions

- Trace propagation uses W3C `traceparent` in A2A envelopes.
- Span names follow OTel GenAI semconv (`gen_ai.request.model`, `gen_ai.usage.input_tokens`, etc. — same conventions the cost-accounting work uses).
- Mesh-specific span attributes: `vox.mesh.lease_id`, `vox.mesh.peer_id`, `vox.mesh.dispatch_kind` (`local|remote|fallback`).
- Errors carry both the originator's `task_id` and the receiver's `worker_id` in span events.

### 6.4 Testing discipline

- Hardware probes: trait-shaped surface, mock for CI, real-hardware test gated behind a feature flag.
- Cloud providers: recorded fixtures, no live API in CI.
- Lease lifecycle: at least one integration test per state transition.
- A2A persistence: a test per backend (in-memory + each durable backend introduced).
- Cross-node: in-process two-node harness for unit/integration; a manual three-node soak for slice acceptance.

### 6.5 Documentation discipline

- `docs/src/reference/populi.md` is the SSOT for the runtime contract; every spec change updates it.
- ADR amendments for architectural changes; new ADRs for new architectural decisions (libp2p adoption, durable-queue choice, etc.).
- Operator-facing changes ship with a how-to in `docs/src/how-to/`.
- The auto-generated `architecture-index.md` is regenerated, never hand-edited. `research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.

---

## Part 7 — Out of scope

Explicitly *not* part of this north-star, even though the work might suggest them:

- **libp2p / gossip-as-source-of-truth.** ADR-020 settles this: HTTP Populi remains SSOT. Gossip is for hints, optional, after the foundation lands.
- **Hosted Populi BaaS.** ADR-009 covers the eventual offering; nothing in this north-star prepares for it. Self-hosted-only.
- **OAuth device-code flows for control-plane access.** Power-user ambition does not require this. Bearer tokens stay the contract.
- **Multi-tenant isolation.** One mesh, one operator (possibly with multiple agents). No cross-org boundary.
- **Autoscaling cloud nodes from local triggers.** RunPod / Vast clients exist for manual launches; automated scaling-up belongs to a separate decision not on this roadmap.
- **GUI for mesh management.** CLI-only at power-user ambition. The marquee/GUI roadmap is its own track.
- **Durable queue choice.** S1 introduces *one* durable backend option; choosing the long-term right answer (sled? sqlite? something else?) is a follow-on ADR.

---

## Part 8 — Success criteria for "north-star reached"

The mesh is at the north-star ambition when **all** of these hold:

1. A vox contributor on a fresh box, with no prior mesh experience, joins the mesh and submits a remote task in under 10 minutes following only `docs/src/how-to/populi-quickstart.md`.
2. A task submitted from any box of a 3-box mesh executes on the right box (by GPU/CPU capacity) without operator routing intervention.
3. A trace ID submitted on box A is queryable from telemetry and contains spans from every box the task touched, with token-level cost attribution.
4. A secret installed only on box A is consumable by a task running on box B after a single `vox populi pair`, with no other operator action.
5. A node going down hard (process kill, network unplug) is detected within 2× heartbeat interval; in-flight tasks fail over (per ADR-017 fallback) within one lease-renewal interval.
6. `cargo test -p vox-populi -p vox-mesh-types` passes with zero live-API or live-hardware dependencies.
7. The combined LOC of "experimental" / "best-effort" / "TODO" markers in the mesh code path is reduced to zero.
8. ADR-017 and ADR-018 statuses move from "Accepted (design intent)" to "Accepted (implemented)".

---

## Part 9 — Decomposition into child specs

Each child spec lives in `docs/src/architecture/` and follows the existing audit/blueprint pattern (frontmatter + part-numbered sections + numbered FIX items where mechanical). Each child spec gets its own implementation plan written via the superpowers `writing-plans` skill at the moment its slice is picked up.

**Slice S1 child specs.** Authored before S1 starts.

- `populi-mesh-probe-correctness-spec-2026.md` — W2 partial. Probe trait shape, mock harness, NVML/wgpu/DRM/Metal correctness criteria.
- `populi-mesh-a2a-durability-spec-2026.md` — W6 partial. Durable A2A backend choice, schema, migration.
- `populi-mesh-local-observability-spec-2026.md` — W5 partial. OTel trace propagation in the local path.
- `populi-mesh-config-baseline-spec-2026.md` — W7 partial. `Vox.toml [mesh]` schema, sensible defaults, quickstart.

**Slice S2 child specs.** Authored after S1 completes.

- `populi-mesh-leases-spec-2026.md` — W1 full. ADR-017 implementation, fallback semantics, persistence, integration tests.
- `populi-mesh-admission-spec-2026.md` — W2 full. Probe data → planner integration; capacity model.
- `populi-mesh-pairing-spec-2026.md` — W3 partial. Handshake, X25519 trust, decrypt on worker, integration with existing JWE encrypt path.
- `populi-mesh-trace-propagation-spec-2026.md` — W5 full. A2A envelope traceparent, cross-node correlation.

**Slice S3 child specs.** Authored after S2 completes.

- `populi-mesh-inventory-spec-2026.md` — W4 full. Per-node inventory endpoint, orchestrator aggregation, planner integration.
- `populi-mesh-key-rotation-spec-2026.md` — W3 full. Pairing rotation, revocation, propagation semantics.
- `populi-mesh-cloud-provider-fixtures-spec-2026.md` — W6 full. RunPod/Vast recorded fixtures.
- `populi-mesh-operator-ux-completion-spec-2026.md` — W7 full. `inventory`, admin out of feature flags, `Vox.toml [mesh]` parity.

**The flat backlog.** Improvements that are real but don't deserve their own spec live in [`populi-mesh-improvement-backlog-2026.md`](populi-mesh-improvement-backlog-2026.md). They get picked up opportunistically when their area is being touched for slice work.

---

## Part 10 — What this north-star explicitly does not commit to

- Specific timeboxes per slice. Slice S1 is sized to fit one focused contributor-month; S2 is two; S3 is one. These are estimates, not commitments.
- Specific durable A2A backend (sled vs sqlite vs other). The S1 spec decides.
- Whether traces use the existing telemetry pipeline or a new collector. The S1 observability spec decides, in alignment with [`telemetry-driven-cost-accounting-research-2026.md`](telemetry-driven-cost-accounting-research-2026.md).
- Whether inventory aggregation lives in the orchestrator daemon or a new mesh-local service. The S3 inventory spec decides.
- The specific shape of `Vox.toml [mesh]`. The S1 config spec decides.

These decisions are deferred to the child specs because making them here would freeze choices before the people writing the child specs have evaluated alternatives.

---

## Appendix A — Workstream → file/crate map

For agents and contributors: which existing code each workstream most touches.

- **W1 leases:** [`vox-orchestrator/src/a2a/dispatch/`](../../../crates/vox-orchestrator/src/a2a/dispatch/), [`vox-orchestrator/src/orchestrator/`](../../../crates/vox-orchestrator/src/orchestrator/), [`vox-populi/src/transport/handlers.rs`](../../../crates/vox-populi/src/transport/handlers/mod.rs), [`vox-mesh-types/src/`](../../../crates/vox-mesh-types/src/) (lease record types).
- **W2 GPU truth:** [`vox-populi/src/mens/hardware/`](../../../crates/vox-populi/src/mens/hardware/), [`vox-populi/src/node_registry.rs`](../../../crates/vox-populi/src/node_registry.rs), orchestrator planner.
- **W3 secret pairing:** [`vox-orchestrator/src/a2a/jwe.rs`](../../../crates/vox-orchestrator/src/a2a/jwe.rs), [`vox-orchestrator/src/a2a/dispatch/mesh.rs`](../../../crates/vox-orchestrator/src/a2a/dispatch/mesh.rs), [`vox-populi/src/transport/handlers.rs`](../../../crates/vox-populi/src/transport/handlers/mod.rs), [`vox-crypto/src/facades.rs`](../../../crates/vox-crypto/src/facades.rs), new pairing storage.
- **W4 model discovery:** new endpoint in [`vox-populi/src/transport/`](../../../crates/vox-populi/src/transport/), new poll job in [`vox-orchestrator/src/orchestrator/`](../../../crates/vox-orchestrator/src/orchestrator/), [`vox-orchestrator/src/models/registry.rs`](../../../crates/vox-orchestrator/src/models/registry.rs).
- **W5 observability:** [`vox-orchestrator/src/a2a/`](../../../crates/vox-orchestrator/src/a2a/), [`vox-populi/src/transport/`](../../../crates/vox-populi/src/transport/), telemetry sink crate.
- **W6 test coverage:** [`crates/vox-populi/tests/`](../../../crates/vox-populi/tests/), inline `#[cfg(test)]` modules in probe and cloud-provider files.
- **W7 operator UX:** [`vox-cli/src/commands/`](../../../crates/vox-cli/src/commands/), [`vox-config/`](../../../crates/vox-config/), [`docs/src/how-to/`](../how-to/).

---

## Revision history

- **2026-05-01.** Initial north-star. Status: current.
