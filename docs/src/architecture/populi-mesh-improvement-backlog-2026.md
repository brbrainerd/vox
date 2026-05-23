---
title: "Populi Mesh Improvement Backlog (2026-05-01)"
description: "Flat tagged list of Populi mesh improvements that aren't load-bearing enough to deserve their own spec. Picked up opportunistically when the area is being touched for slice work in the north-star plan."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Inventory of small-to-medium mesh improvements; useful as a queue when an agent is asked to clean up a specific subsystem."
---

# Populi Mesh Improvement Backlog

**Companion document.** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md) — strategic plan. This backlog is everything that doesn't deserve its own child spec but is real and worth doing.

**How to use this list.** When working on a slice or any mesh code, scan the relevant tag(s) and pick up adjacent items. Items don't need to ship with the slice that owns their area; they're "while you're there" work. Don't bundle large items into PRs unrelated to their slice.

**Tag legend.**

- `[bug]` — possible defect or behavioral wart in shipped code
- `[test]` — missing or weak test coverage
- `[docs]` — documentation gap, drift, or contradiction
- `[health]` — code health: size, dead code, naming, organization
- `[sec]` — security hardening
- `[ux]` — operator-facing pain
- `[obs]` — observability gap
- `[contract]` — schema or wire-contract clarity
- `[config]` — configuration surface
- `[cli]` — CLI verb / flag / output
- `[perf]` — performance smell or known cost
- `[resil]` — resilience / failure-handling
- `[a11y]` — accessibility (CLI output legibility, error messages)
- `[mig]` — migration or upgrade path

Each item is keyed `MESH-NNN`. Stable; references can be made from PRs and other specs.

---

## Part A — Transport (vox-populi/src/transport/)

- `MESH-001` `[health]` `transport/handlers.rs` is 1627 lines. Split by route group (join/heartbeat, A2A inbox, leases, models, admin) into separate modules under `transport/handlers/`.
- `MESH-002` `[test]` `transport/handlers.rs` has zero inline `#[cfg(test)]` modules. Cover request validation, idempotency dedupe, and bearer scope checks.
- `MESH-003` `[test]` `transport/auth.rs` constant-time compare path has no inline test for forged/short tokens.
- `MESH-004` `[test]` `transport/mesh_replay.rs` replay-detection logic untested; add fixture-driven test for duplicate `idempotency_key` handling.
- `MESH-005` `[test]` `transport/store.rs` JSON-file persistence: no test for partial-write recovery on crash.
- `MESH-006` `[test]` `transport/result_attestation.rs` (97 LOC, no test): cover signature verify happy path + tampered payload + wrong-key rejection.
- `MESH-007` `[bug]` 7 `unwrap()`/`expect(` in `transport/*.rs` — audit each; replace user-input-derived ones with `?`.
- `MESH-008` `[contract]` `A2ADeliverRequest::message_type` is a free-form `String`. Define an enum (or at least a documented vocabulary in `populi.md`).
- `MESH-009` `[contract]` `A2ADeliverRequest::privacy_class` accepts `public|private|trusted` per docs but isn't enum-typed; promote.
- `MESH-010` `[contract]` `A2ADeliverRequest::priority: u8` (0–255) — document the actual scale used by the planner; today consumers guess.
- `MESH-011` `[obs]` `transport/handlers.rs` does not emit structured spans for inbound requests; every handler should open a span tagged with `peer_id`, `endpoint`, `idempotency_key`.
- `MESH-012` `[resil]` Single JSON-file store has no concurrent-writer story documented; either lock or migrate to a real backend (covered by S1 spec).
- `MESH-013` `[contract]` `A2AStoredMessage` lacks a `schema_version` field. Add it now to make future migrations cheap.
- `MESH-014` `[sec]` Bearer auth header name is hard-coded; document and centralize.
- `MESH-015` `[sec]` No rate limit on `/v1/a2a/deliver`; a paired-but-malicious peer can flood the inbox.
- `MESH-016` `[sec]` No size limit on `payload` field beyond what axum defaults give; explicit per-route body limit.
- `MESH-017` `[obs]` Every 4xx/5xx response should include a `request_id` echoed in an OTel event.
- `MESH-018` `[health]` `transport/mod.rs` carries both type definitions and module wiring (855 LOC); split types into `transport/types.rs`.
- `MESH-019` `[contract]` JWE field naming inconsistent: `jwe_payload` vs the `gen_ai.*` semconv direction. Decide on a project-wide JWE attribute name.
- `MESH-020` `[bug]` `A2AStoredMessage::idempotency_dedupe_key` set to `Some(dedupe)` in one branch and `None` in another (`handlers.rs:766`, `:804`). Audit which branch is correct.
- `MESH-021` `[test]` `transport/router.rs` has only 1 `#[test]`; cover all route paths with at least a smoke request.
- `MESH-022` `[docs]` `populi.md` describes the HTTP contract but doesn't link directly to handler functions. Cross-reference for navigability.
- `MESH-023` `[obs]` Track inbox depth per peer as a metric.
- `MESH-024` `[obs]` Track lease grant/revoke counts; today there's no way to know if leases are being granted at all.
- `MESH-025` `[contract]` `task_kind` is a free-form `String` field on `A2ADeliverRequest`; align with the orchestrator's known task kinds.

## Part B — Node registry & identity

- `MESH-026` `[test]` `node_registry.rs` has no inline tests despite being the join/heartbeat heart.
- `MESH-027` `[test]` `node_record_for_current_process()` integration test against a fixture probe set (independent of real hardware).
- `MESH-028` `[contract]` `NodeRecord` field documentation is split between Rust doc-comments and `populi.md`; pick one as canonical.
- `MESH-029` `[bug]` `lib.rs:313` TODO: "cuda_driver_version from precision layer if needed" — close out (either implement or delete).
- `MESH-030` `[health]` `dummy.rs:3` `unimplemented!()` macro on `CudaDevice` — confirm this file's purpose; if dead, delete.
- `MESH-031` `[contract]` Heartbeat cadence and TTL not documented in the type definition; add doc-comments.
- `MESH-032` `[resil]` No exponential backoff on heartbeat retry after transient failure (verify; if absent, add).
- `MESH-033` `[obs]` Node-leave events are silent; emit a structured event so the orchestrator can react proactively instead of waiting for TTL.
- `MESH-034` `[sec]` Ed25519 challenge/response for join is documented but the actual token rotation flow is not; spec it.
- `MESH-035` `[contract]` `WorkerDonationPolicy` semantics across nodes: which node's policy wins on a paired pair? Document.
- `MESH-036` `[test]` Round-trip serialization for every variant of `TaskCapabilityHints`; current sample roundtrip covers happy path only.
- `MESH-037` `[health]` Move all type definitions in `vox-populi` that should be cross-crate to `vox-mesh-types`; today there's drift.

## Part C — Hardware probes

- `MESH-038` `[test]` `mens/hardware/nvml.rs` has zero tests; introduce trait+mock.
- `MESH-039` `[test]` `mens/hardware/wgpu_probe.rs` has zero tests; same trait+mock pattern.
- `MESH-040` `[test]` `mens/hardware/linux_drm.rs` has zero tests.
- `MESH-041` `[test]` `mens/hardware/win_dxgi.rs` has zero tests.
- `MESH-042` `[test]` `mens/hardware/macos_metal.rs` has zero tests.
- `MESH-043` `[contract]` `hardware/types.rs` enums for vendor/family — document the canonical strings the rest of the codebase expects.
- `MESH-044` `[bug]` Probes that fail (e.g., NVML library missing) currently silently degrade to no-GPU; surface a structured warning.
- `MESH-045` `[obs]` Emit a one-shot startup event with the probe summary so logs show "what does this node think it has".
- `MESH-046` `[ux]` `vox doctor` (or equivalent) should run all probes and report any inconsistency between probe output and `VOX_MESH_ADVERTISE_*` flags.
- `MESH-047` `[resil]` NVML probe should not hold the library handle for the process lifetime; reopen on each probe to handle driver restart.
- `MESH-048` `[perf]` Probe cadence: today probes run on every node-record build; cache with a TTL (configurable via `Vox.toml [mesh]`).
- `MESH-049` `[contract]` Capacity calculation rule (Layer B per ADR-018) is not documented anywhere outside the ADR; add a how-to.
- `MESH-050` `[health]` `mens/hardware/mod.rs` 107 LOC dispatcher: extract per-platform `HardwareProbe` trait.
- `MESH-051` `[ux]` Multi-GPU machines: probe should label devices in a way that survives reboot; today device IDs may shift.
- `MESH-052` `[test]` Real-hardware test gated behind a feature flag (`hw-probe-live-test`) so dev-machine runs can validate.
- `MESH-053` `[docs]` Document the matrix of which probes run on which OS.

## Part D — Cloud providers

- `MESH-054` `[test]` `mens/cloud/runpod_provider.rs` has zero tests; record fixtures from a manual run.
- `MESH-055` `[test]` `mens/cloud/vast.rs` has zero tests; same.
- `MESH-056` `[test]` `mens/cloud/budget.rs` has zero inline tests; cover budget threshold transitions.
- `MESH-057` `[test]` `mens/cloud/resolver.rs` (464 LOC) has zero tests; this is the routing brain — not testing it is a major gap.
- `MESH-058` `[test]` `mens/cloud/watchdog.rs` has zero tests.
- `MESH-059` `[test]` `mens/cloud/local_provider.rs` has zero tests despite being the default.
- `MESH-060` `[test]` `mens/cloud/part_jobs.rs` has zero tests.
- `MESH-061` `[health]` `mens/cloud/mod.rs` (448 LOC) is a kitchen-sink module; split orchestration vs. provider-trait.
- `MESH-062` `[sec]` Provider API keys: confirm none are read directly from env outside vox-secrets (cross-reference [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md)).
- `MESH-063` `[ux]` Cost estimator should print the assumptions used (region, tier) so users can sanity-check.
- `MESH-064` `[resil]` Provider-side rate-limit responses (429) handling: backoff with jitter, surface to caller.
- `MESH-065` `[obs]` Spend tracking: emit a span on every provider call with `vox.cloud.provider`, `vox.cloud.cost_usd_estimated`.
- `MESH-066` `[contract]` Per-provider budget error type is provider-specific; lift to a common shape.
- `MESH-067` `[bug]` Watchdog kill semantics: confirm a killed cloud job releases the budget reservation.
- `MESH-068` `[ux]` `vox populi cloud status` (verb does not exist): show in-flight launches, total burn, budget remaining.

## Part E — Mens training

- `MESH-069` `[test]` `mens/tensor/candle_inference_serve.rs` parity test with serving in production.
- `MESH-070` `[bug]` "QLoRA proxy-max-layers=0 unsupported" — currently silent fail or panic? Trace and surface a clean error.
- `MESH-071` `[bug]` Partial LoRA-head training: "not implemented" markers in tensor code; close out or document as a non-feature.
- `MESH-072` `[test]` Training preflight schema validator: cover all known invalid inputs, not just the canonical happy/sad paths.
- `MESH-073` `[obs]` Training emits `model_pricing_catalog` rows but trace correlation with the originating training job is missing.
- `MESH-074` `[ux]` `vox mens` subcommand error messages assume the user knows the LoRA terminology; soften.
- `MESH-075` `[health]` Tensor module split is internally consistent but not surfaced in `populi.md`; document the layer boundary.
- `MESH-076` `[contract]` Adapter schema v3 (`adapter_schema_v3.rs`) — document v2 → v3 migration in `docs/src/operations/`.
- `MESH-077` `[test]` Hub (`mens/hub.rs`) integration with HF cache: test against a recorded directory.
- `MESH-078` `[mig]` Mens checkpoints carry no provenance metadata that survives mesh transit; spec a sidecar manifest.

## Part F — Orchestrator dispatch (mesh paths)

- `MESH-079` `[test]` `vox-orchestrator/src/a2a/dispatch/mesh.rs` JWE encrypt happy path: verify ciphertext is decryptable by the matching key.
- `MESH-080` `[test]` Same file: test that dispatch falls back gracefully when no recipient pubkey is known.
- `MESH-081` `[bug]` `vox-orchestrator/src/a2a/jwe.rs` has `decrypt_jwe_compact` defined but unused; either wire it in or feature-gate.
- `MESH-082` `[test]` Lease grant integration test exists in `populi_single_owner.rs`; expand to multi-owner contention scenarios.
- `MESH-083` `[obs]` Mesh dispatch path emits no spans tagged `vox.mesh.dispatch_kind`; add `local|remote|fallback`.
- `MESH-084` `[contract]` `remote_worker.rs` constructs A2ADeliverRequest with `jwe_payload: None` (`:289`) — when is that legitimate vs. a bug?
- `MESH-085` `[health]` `dispatch/mesh.rs` JWE-population logic is buried in a `if let Ok(reqs)` chain; lift to a named function.
- `MESH-086` `[test]` Idempotency: a duplicate dispatch should produce the same `message_id`; verify.
- `MESH-087` `[obs]` Track time-from-dispatch-to-receipt as a histogram per peer.
- `MESH-088` `[bug]` Race between dispatch and lease revoke: document and test.
- `MESH-089` `[test]` Cross-encoding test: serialize on orchestrator, deserialize on populi receiver, every field round-trips.

## Part G — Documentation

- `MESH-090` `[docs]` `populi.md` is monolithic; split into "join/heartbeat", "A2A inbox", "leases", "models" sub-pages with `populi.md` as index.
- `MESH-091` `[docs]` ADR-008 addendum's "experimental orchestrator routing in-process only" wording is confusing; rephrase after S2 lands.
- `MESH-092` `[docs]` ADR-017 status should change from "Accepted (design intent)" to "Accepted (implemented)" when S2 ships; add the reminder to the slice spec.
- `MESH-093` `[docs]` ADR-018 status: same.
- `MESH-094` `[docs]` `populi-coordination.md` overlaps with `populi.md`; merge or clearly delineate.
- `MESH-095` `[docs]` `populi-work-type-placement-matrix.md` should link to the orchestrator's planner code.
- `MESH-096` `[docs]` `model-orchestration-ssot-audit-2026.md` claim "no X25519 KEM in vox-crypto" is stale; update or supersede.
- `MESH-097` `[docs]` Same audit's claim "jwe_payload plumbed but never populated" is stale; update.
- `MESH-098` `[docs]` Add a glossary: lease, peer, pair, donation, claimer, trust class.
- `MESH-099` `[docs]` Each ADR should link forward to the spec(s) implementing it.
- `MESH-100` `[docs]` `how-to/populi-quickstart.md` does not exist; create as part of S1.
- `MESH-101` `[docs]` `operations/populi-disaster-recovery.md` does not exist; create when durable backend lands.
- `MESH-102` `[docs]` Mens training docs reference defunct or renamed feature flags; audit and update.
- `MESH-103` `[docs]` Add an architecture diagram (mermaid is fine) showing crate boundaries: vox-mesh-types, vox-populi, vox-orchestrator, vox-secrets, vox-crypto.
- `MESH-104` `[docs]` Document the actual default mesh ports (today's defaults vs operator overrides).

## Part H — Operator UX

- `MESH-105` `[ux]` `vox populi join` does not exist as a verb; today operators run `vox populi serve` and edit env vars.
- `MESH-106` `[ux]` `vox populi pair` does not exist.
- `MESH-107` `[ux]` `vox populi status` exists but output is verbose and unstructured; add `--json` and a TTY-friendly default.
- `MESH-108` `[ux]` `vox populi inventory` does not exist.
- `MESH-109` `[ux]` Admin commands (`maintenance`, `quarantine`, `exec-lease-revoke`) are gated behind the `populi` feature flag — drop the gate.
- `MESH-110` `[cli]` Error from `vox populi serve` when port is in use is a generic Rust error; wrap with "is another mesh process running?".
- `MESH-111` `[cli]` `vox populi --help` should describe the mesh model in 3 sentences at the top.
- `MESH-112` `[ux]` Failed bearer auth returns 401 with no body; include "set VOX_MESH_TOKEN or [mesh] token in Vox.toml".
- `MESH-113` `[ux]` First-run setup: `vox populi init` to write a starter `Vox.toml [mesh]` block.
- `MESH-114` `[ux]` `vox populi peers` to list known peers with last-seen, lease counts, version.
- `MESH-115` `[ux]` `vox populi unpair` for removing a paired peer cleanly.
- `MESH-116` `[a11y]` `vox populi status` colors: respect `NO_COLOR` env.
- `MESH-117` `[a11y]` Long node-record dumps should paginate or `--no-pager`.
- `MESH-118` `[ux]` Time fields in operator output should be relative ("3m ago") not ISO timestamps by default.
- `MESH-119` `[ux]` `vox doctor` should include a "mesh" section.
- `MESH-120` `[ux]` Default config should pick a free local port instead of failing if 9000-equivalent is busy.

## Part I — Observability & telemetry

- `MESH-121` `[obs]` No `vox.mesh.*` span attributes anywhere; define the namespace.
- `MESH-122` `[obs]` Trace propagation field on `A2ADeliverRequest` is missing; add `traceparent: Option<String>`.
- `MESH-123` `[obs]` GenAI semconv attributes (`gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.response.id`) on remote tasks: align with [`telemetry-driven-cost-accounting-research-2026.md`](telemetry-driven-cost-accounting-research-2026.md).
- `MESH-124` `[obs]` Lease grant/renew/expire/revoke as separate event types in telemetry.
- `MESH-125` `[obs]` Pairing events (pair/unpair/rotate) as audit-log entries.
- `MESH-126` `[obs]` Heartbeat misses logged at INFO with peer-id; today they may be silent.
- `MESH-127` `[obs]` "Why was this task routed here?" — add a span event capturing the planner's decision factors at dispatch.
- `MESH-128` `[obs]` Cloud provider call: span with cost_usd_estimated, latency_ms, hit_rate_limit.
- `MESH-129` `[obs]` Probe failures: span event with which probe and why.
- `MESH-130` `[obs]` Add a `vox populi events` follow-mode (like `tail -f`) for live event stream.

## Part J — Security

- `MESH-131` `[sec]` Audit every `unwrap()` / `expect(` in transport for DoS potential on malformed input.
- `MESH-132` `[sec]` JWE recipient pubkey: define rotation policy (max age, force-rotate command).
- `MESH-133` `[sec]` Bearer token: support multiple tokens with role labels (read-only, dispatch, admin).
- `MESH-134` `[sec]` Add an explicit "trust class" check on every A2A handler matching the request's `privacy_class`.
- `MESH-135` `[sec]` `vox-crypto` X25519 sealed-box: add a constant-time test for the receiver-side key check.
- `MESH-136` `[sec]` Document the threat model for paired peers (what a compromised peer can do).
- `MESH-137` `[sec]` Pairing handshake: rate-limit per source IP / peer-id to prevent enumeration.
- `MESH-138` `[sec]` Lease IDs should be unguessable (use UUIDv7 or random bytes); confirm.
- `MESH-139` `[sec]` Audit log: tamper-evident chain (each entry hashes prev) for security-sensitive events.
- `MESH-140` `[sec]` Confirm no peer-controlled string is interpolated into a log message format string.

## Part K — Configuration

- `MESH-141` `[config]` `Vox.toml [mesh]` block: design schema (north-star spec defers, S1 spec decides).
- `MESH-142` `[config]` Env-var equivalents for every `[mesh]` field; precedence: env > file > default.
- `MESH-143` `[config]` `[mesh.peers]` for declaring known peers in config (alternative to runtime `pair`).
- `MESH-144` `[config]` `[mesh.donation]` for worker donation policy in declarative form.
- `MESH-145` `[config]` `[mesh.budget]` for cloud-provider spend caps.
- `MESH-146` `[config]` `[mesh.observability.endpoint]` for OTel collector.
- `MESH-147` `[config]` Config validation: `vox config check` should run mesh-specific lints.
- `MESH-148` `[config]` Deprecation warnings for env vars after `[mesh]` parity ships.
- `MESH-149` `[config]` `[mesh.tls]` block for cert paths (file-based PKI, not just bearer).
- `MESH-150` `[config]` Per-peer overrides (a paired peer's bearer or pubkey) live in config not state.

## Part L — Persistence & A2A storage

- `MESH-151` `[resil]` Single JSON-file store has no fsync discipline documented; verify.
- `MESH-152` `[resil]` Recover from a corrupt store file: `vox populi store check`, `vox populi store repair`.
- `MESH-153` `[perf]` Store grows unbounded for completed messages; add retention with TTL.
- `MESH-154` `[contract]` Schema-version field on the store envelope (separate from `MESH-013`'s message-level version).
- `MESH-155` `[mig]` Migration tool to move from JSON store to durable backend (S1 spec output).
- `MESH-156` `[obs]` Store size metric; alert if > N MB.
- `MESH-157` `[test]` Concurrency stress test against the store with N concurrent writers.
- `MESH-158` `[bug]` Verify atomicity on multi-field updates; today it's "rewrite the file" — confirm tmp-file-rename.

## Part M — Lease lifecycle (preparation for S2)

- `MESH-159` `[contract]` Document state machine: `granted → renewed → expired | revoked | completed`.
- `MESH-160` `[test]` State-machine coverage test: every legal transition + every illegal transition rejected.
- `MESH-161` `[obs]` Lease-state changes emit events.
- `MESH-162` `[contract]` Renewal protocol: who initiates, what happens on no-renew?
- `MESH-163` `[ux]` `vox populi leases` to list active leases with TTL.
- `MESH-164` `[sec]` Revocation requires admin scope; bearer with read-only scope must be rejected.
- `MESH-165` `[resil]` Lease holder crashes mid-task: what triggers cleanup?
- `MESH-166` `[contract]` Document collision policy if two nodes both think they hold the lease (should be impossible by design — verify).
- `MESH-167` `[perf]` Lease index lookup: O(1) by `lease_id`, O(log n) by `task_id`; verify or improve.

## Part N — Model discovery (preparation for S3)

- `MESH-168` `[contract]` Per-node inventory schema: what fields, what versioning.
- `MESH-169` `[obs]` Inventory poll: emit a span per poll cycle with peer count, model count, errors.
- `MESH-170` `[ux]` `vox populi inventory --filter <model-id>` to find which node has a model.
- `MESH-171` `[perf]` Inventory poll cadence: tunable, with adaptive backoff under failure.
- `MESH-172` `[contract]` Stale inventory: if a peer hasn't refreshed in N cycles, mark its inventory as stale (not gone).
- `MESH-173` `[contract]` Inventory diff event: which models appeared/disappeared since last poll.
- `MESH-174` `[bug]` LoRA adapters: an inventory entry needs base-model + adapter id, not just model id.
- `MESH-175` `[contract]` HF cache scan strategy: scan once at startup vs. live? Document.

## Part O — Cross-crate cleanup

- `MESH-176` `[health]` Some types in `vox-populi` should be in `vox-mesh-types`; audit and migrate.
- `MESH-177` `[health]` `vox-mesh-types` has no tests; even pure data types deserve serde round-trip tests.
- `MESH-178` `[health]` Skill descriptions in `vox-skills` (`populi.skill.md`, `orchestrator.skill.md`) duplicate text from `populi.md`; reference instead.
- `MESH-179` `[health]` Feature flags: `mens-gpu` is the default; document the implications for non-GPU contributors building from source.
- `MESH-180` `[health]` `Cargo.toml` features matrix: collapse synonyms (e.g., `mens-train` and `mens-cloud`) where they're no longer independent.
- `MESH-181` `[health]` Public API surface review on `vox-mesh-types` — tighten `pub` to what's actually consumed.

## Part P — Performance

- `MESH-182` `[perf]` HTTP request handling: confirm axum body deserialization is bounded and zero-copy where possible.
- `MESH-183` `[perf]` Probe cache (per `MESH-048`) — measure overhead.
- `MESH-184` `[perf]` JWE encryption is per-task — measure; consider per-peer session-key caching.
- `MESH-185` `[perf]` JSON-store fsync per write is a latency floor; offer a "buffered" mode (named risk: data loss on crash).
- `MESH-186` `[perf]` Heartbeat sender holds no allocations across heartbeats; verify (avoid hot-path GC pressure).
- `MESH-187` `[perf]` `node_record_for_current_process()` is called on every heartbeat; profile.

## Part Q — Resilience

- `MESH-188` `[resil]` Network partition: a peer reachable in one direction only — what's the documented behavior?
- `MESH-189` `[resil]` Clock skew between peers: lease expiry is wall-clock based; document tolerance.
- `MESH-190` `[resil]` Node restart: in-flight messages on the receiver — recovered from store, or lost?
- `MESH-191` `[resil]` Disk full on receiver: A2A handler should return a structured 507 with retry guidance.
- `MESH-192` `[resil]` Probe device disappears mid-task (GPU reset): graceful failover.
- `MESH-193` `[resil]` HTTP timeouts: document defaults and per-route overrides.
- `MESH-194` `[resil]` Backpressure: when inbox is full, what happens? Return 429 with structured retry hint.
- `MESH-195` `[resil]` Cascading lease-revoke: revoking on origin should not flap on the receiver if it's already started executing.

## Part R — Migration & versioning

- `MESH-196` `[mig]` `vox-mesh-types` versioning policy: when does a field change require a major version?
- `MESH-197` `[mig]` Version-skew tolerance test: orchestrator at version X talks to populi at version X-1.
- `MESH-198` `[mig]` Document the deprecation timeline for env vars after `[mesh]` parity.
- `MESH-199` `[mig]` Store schema migration: forward-only with versioned envelope.
- `MESH-200` `[mig]` ADR-008 → ADR-020 transport reconciliation: as the transport layer evolves (gossip-as-hint, optional QUIC), how do mixed-version meshes interop?

## Part S — Skill surface (vox-skills)

- `MESH-201` `[docs]` `populi.skill.md` says "no remote execute" — update when S2 ships.
- `MESH-202` `[docs]` `orchestrator.skill.md` label-alignment guidance should reference the planner's code path.
- `MESH-203` `[ux]` `vox_populi_local_status` tool should include a "this node is paired with N peers" summary.
- `MESH-204` `[ux]` `vox_submit_task` tool: include a hint about which node it's likely to land on, given current routing.
- `MESH-205` `[contract]` Skill tools should accept and return the same `traceparent` so an agent can correlate a chain of submit→wait→read.

## Part T — Misc / cross-cutting

- `MESH-206` `[health]` `crates/vox-populi/src/dummy.rs` — confirm purpose; if dead, delete.
- `MESH-207` `[docs]` README.md mesh section is thin; expand with one paragraph per ADR.
- `MESH-208` `[docs]` `CHANGELOG.md` mesh entries: standardize prefix (`mesh:` vs `populi:`) — pick one.
- `MESH-209` `[ux]` `vox version` should print the mesh wire-version separately from the binary version.
- `MESH-210` `[health]` Unify error types in `vox-populi` — many handler functions return `Box<dyn Error>` style; lift to a named enum.

---

## Notes on completeness

This list is **representative, not exhaustive.** It is the set of items I could justify on a one-pass review of file shapes, ADR claims, and the existing audit doc. A second pass — file-by-file in `vox-populi/src/transport/handlers.rs`, `mens/cloud/resolver.rs`, and `mens/tensor/`, in particular — will surface dozens more in the [bug], [contract], and [perf] tags. New items should be appended with the next available `MESH-NNN` and tagged consistently.

A `[done]` tag and a date field can be added in a future revision once items start being closed; the current list is all-pending.

## Revision history

- **2026-05-01.** Initial backlog of 210 items spanning transport, registry, probes, cloud, mens, dispatch, docs, UX, observability, security, config, persistence, leases, discovery, cross-crate, perf, resilience, migration, skills.
