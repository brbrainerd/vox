---
title: "Populi Mesh: iroh Transport, Sandboxed Execution, and Measured Placement"
description: "Design replacing the hand-rolled Populi mesh transport with iroh, sandboxing mesh-received work by default, porting the four capabilities the HTTP plane still serves, and adding a measured placement model that decides per-machine whether shipping a job is worth it."
category: "Architecture SSOTs"
status: "current"
---

# Populi Mesh: iroh Transport, Sandboxed Execution, and Measured Placement

**Revision 4**, 2026-09-04. Revisions 0–3 and the reason each was superseded are
in §12. Revision 3 was audited across eight tracks; this revision incorporates
those findings, several of which invalidated its headline claims.

## Goal

When a user works in the CLI or in Axis, vox distributes CPU and GPU work across
the machines they own, gets out of the way, and shows honestly what that bought
or cost.

## Requirements

1. **Minimal setup.** Two machines with vox installed pair with **one
   out-of-band step** and no network configuration.
   *(Revision 3 said "no configuration." Pasting a ticket is configuration; §5
   explains why one such step is unavoidable, and the requirement now says so.)*
2. **No central server.** Fully functional with Vox infrastructure switched off.
3. **Works without internet.** LAN-only and air-gapped are supported.
4. **No account required.** No sign-up, no third-party identity provider.
5. **Secure by default.** The safe configuration is the one reached without
   reading documentation. **Pairing a machine does not grant it native code
   execution** (§4).
6. **Confirmed in Axis.** Not done until visible and operable in the GUI.

---

## Part 1 — Why iroh

**An iroh `EndpointId` is an ed25519 public key**, and `Connection::remote_id()`
returns the peer's id derived from their TLS certificate. Awaiting `Incoming`
completes the handshake, so identity is proven before any application byte is
processed. 0-RTT — the only state where identity is unproven — is opt-in on both
sides and this design never enables it (§4.1).

Authorization is therefore one allowlist check at accept time, replacing the
`FullAccess` / bearer / JWT / role matrix in `transport/auth.rs` and the
unauthenticated-execute path it guarded.

### Corrections to revision 3's account of iroh

Verified against iroh 1.1.0 source, not doc summaries:

| Rev 3 claim | Correction |
|---|---|
| "quinn" throughout | **iroh 1.1 does not use quinn.** It uses `noq` 1.2, n0's own QUIC stack. |
| "`Connection::stats()` gives throughput, so inputs are measured" | **`ConnectionStats` has no throughput, bandwidth, or congestion window** — only `udp_tx.bytes`, `udp_rx.bytes`, `lost_packets`, `lost_bytes`. RTT comes from `rtt(PathId)`. Throughput must be **derived by vox** by differentiating byte counters across a transfer. Still measured, but by us. |
| "`TrustedNodeRegistry` is already keyed by pubkey" | **False.** It is keyed by `node_id`; `pubkey_hex` is a separate field that can be empty. The mesh needs its own store (§4.4). |
| `presets::Minimal` assumed n0-free | **Confirmed** from source: `Builder::empty()` plus a crypto provider. No relay, no DNS, no pkarr. `N0DisableRelay` still publishes to n0 pkarr and DNS — ban it too. |

---

## Part 2 — What iroh replaces, and what must be ported first

### Replaced

| Hand-rolled | Replacement | Lines |
|---|---|---|
| axum control plane, ~25 routes, in two near-identical copies | `Endpoint` + ALPN + typed protocol | 9,185 (largely duplicate) |
| bearer / JWT / `FullAccess` auth | `Connection::remote_id()` against an allowlist | + the RCE |
| bespoke ticket and mDNS designs | `iroh_tickets::EndpointTicket`, `iroh-mdns-address-lookup` | ~600 unwritten |
| `tls.rs` (no caller in `serve()`) | QUIC is encrypted by construction | 119 |
| exec-lease **grant protocol** | `Connection::closed()` | see below |

### Must be ported before anything is deleted

Revision 3 listed these as deleted. They are live, and three of them are load-bearing for goals stated earlier in this project:

| Capability | Live consumer | Why it can't just go |
|---|---|---|
| **A2A mailbox** (`deliver`/`inbox`/`ack`) | `vox-orchestrator/src/a2a/remote_worker.rs` (1,325 lines) | Store-and-forward delivery to an **offline** peer. A request/response RPC is not a substitute. |
| **Federation directory** | `models/registry.rs:379`, `catalog.rs:535`, `task_submit.rs:700` | Feeds mesh models to the **model selector**. All three call sites are `if let Ok(…)`, so deletion is **silent**: mesh models vanish with no error. |
| **Queue stats** | `populi_tools.rs:178` → `MeshView.tsx:93` | **Axis calls it today.** |
| **`PopuliHttpOp`** | `vox-workflow-runtime/src/workflow/populi.rs` | A Vox **`activity` language surface**. Removing it is a language-level breaking change, not a refactor. |

### Deletion, honestly accounted

Revision 3 claimed ~12,000 deleted / ~1,000 written. Verified by symbol search:

- `vox-mesh-types` dead modules: **37 lines**, not ~600. `secret_sync` is live in `vox secrets`; `kudos` in `vox-gamify` and `vox-db`; `op_fragment` in the hopper mesh adapter. Only `quorum` and `model_inventory` are dead.
- `PublicAttestationManifest` has **six live call sites** in `vox-ml-cli`. Revision 3's SSOT amendment text claimed zero — it would have filed a false statement against a ratified SSOT.
- `http_client.rs` (649), `http_auth.rs` (112), `http_lifecycle.rs` (198) are `use crate::transport::…` and **cannot survive** the transport deletion. Not previously counted.
- The exec-lease **table, `vox-db/src/mesh_exec_leases.rs`, and `lease_gate.rs` stay.** Connection-as-lease replaces the *grant protocol*; the ADR-017 duplicate-execution guard is a **database** check and is transport-independent. `known-tables.txt:112` also pins the table to a drift fixture.

**Revised: ~7,000 lines deleted, ~2,500 rewritten, ~1,500 written.** The bet is
still good — most of the deletion is a duplicated transport nobody reaches — but
it is a rewrite with a deletion in it, not a deletion with a rewrite in it.

---

## Part 3 — What survives, and why it matters

`NodeRecord` capability data (probe-backed GPU truth, VRAM, `host_triple`),
`select_best_node`'s **admission** logic, and the local job queue. iroh gives a
pipe between machines; nothing else knows which machine has 12 GB of VRAM free.

---

## Part 4 — Security

Revision 3 argued the RCE "stops existing." That is true of the *transport*. It
was false of the *executor*, which revision 3 left undefined while Phase 3
deleted the only gate that existed.

### 4.1 Identity and the 0-RTT invariant

`remote_id()` is infallible on a handshake-completed connection **because** the
handshake authenticated the peer. In 0-RTT states it returns `Result`, precisely
because identity is not yet proven. **This design never calls `into_0rtt()`**,
and a detector enforces that alongside the `presets::N0` ban — otherwise a future
`?` added to satisfy the compiler silently makes every check advisory.

Trust is re-checked **per request**, not only at accept, so revocation bounds
exposure to one in-flight job.

### 4.2 Execution: sandboxed by default

The existing worker writes posted bytes to the temp dir, `chmod 0755`, and
executes them natively. Default policy is `"permissive"`. There is **no timeout**
— `Command::output()` blocks forever — and `max_job_duration_secs` /
`max_concurrent` in `donation_policy` have **no readers anywhere**. They are
decorative.

Therefore:

- **The receiver chooses the sandbox, never the sender.** `Isolation::Wasm` is
  the default for mesh-received work.
- **Native execution is per-peer opt-in**, recorded as a `TrustLevel` in the
  trust store and set by an explicit GUI action — never a global secret. A global
  permissive flag is exactly the control that failed.
- **`JobLimits` is part of the executor contract**: `wall_clock` (default 300 s,
  hard kill), `max_output_bytes` (10 MiB, carried forward), `max_payload_bytes`.
- **`MAX_CONCURRENT_JOBS_PER_PEER = 4`.** The allowlist protects against
  strangers, not against a trusted machine that has gone bad.

### 4.3 Bounded pre-authorization work

An unauthenticated peer can force a TLS handshake — an X25519 exchange plus a
signature verification, ~100 µs — and iroh has no built-in connection cap. The
accept loop bounds in-flight handshakes with a semaphore, applies a handshake
timeout, and calls `Incoming::retry()` above a threshold to make spoofed sources
prove reachability.

### 4.4 Trust store

A **separate `~/.vox/mesh_trust.json` keyed by `EndpointId`**, holding
`{endpoint_id, label, trust_level, added_at}`. Not `trusted_nodes.json`: that is
keyed by `node_id` from a different keyspace, its `pubkey_hex` can be empty, and
overloading it would make `untrust` ambiguous by construction. Writes are
atomic (temp + rename); a read failure fails closed **and logs at `error!`** —
failing closed silently is how a security control becomes an undiagnosed outage.

### 4.5 Payload and result size

BLAKE3 verification proves the bytes match the hash **the sender chose**. It is
an integrity property, not a safety one. Sizes are declared in the protocol and
enforced before *and* during transfer, in **both directions** — the result fetch
runs on the originator, which is the user's main machine, and revision 3 never
mentioned it. Per-job caps plus a store budget, with blobs GC'd on completion.

### 4.6 Consuming a ticket is the highest-privilege action in the system

Revision 3 said *"a ticket grants the holder nothing until the far side
reciprocates."* True of **publishing** one. Backwards for **consuming** one:
`vox auth trust <ticket>` moves an attacker-chosen key onto the allowlist.

The strongest attack on this design is social — "paste this to pair" in a chat —
and revision 3's framing actively encouraged treating the paste as low-stakes.
So: the CLI prints the `EndpointId` and requires confirmation; Axis decodes and
displays it before the Trust button is live; and §4.2's sandboxed default is what
makes a mistaken paste survivable rather than fatal.

### 4.7 mDNS supplies addresses, never identity

mDNS is unauthenticated: anyone on the LAN can announce any `(EndpointId,
addresses)` pair. TLS means they cannot impersonate a peer, but they *can*
blackhole one by announcing dead addresses, or use the node as a reflector.
Cap addresses per peer, prefer ticket-learned and previously-successful
addresses, and refuse discovered addresses outside the local subnet.

### 4.8 The placement model is an attacker-influenced input

A peer reports its own speed by completing jobs quickly. A compromised peer can
drive its median toward zero and attract **every** job — including payloads that
are source code and training data — while Axis displays it as the fastest
machine. Floor each peer's estimate relative to the best honestly-observed time.

Deeper consequence to record now: with placement, the scheduler makes a **data
placement** decision. Anything that should not leave a machine needs a per-job
bit the cost model cannot override.

---

## Part 5 — Pairing

| Tier | Mechanism | Coverage | Status |
|---|---|---|---|
| **T0** | loopback | one machine | always |
| **T1** | `EndpointTicket` paste | **every network** | default |
| **T2** | `iroh-mdns-address-lookup` | most home/office LANs | convenience over T1 |
| **T3** | self-hosted iroh relay | cross-site | opt-in |
| **T4** | vox-operated rendezvous | — | **prohibited** |

T1 remains the default: no multicast mechanism reaches managed Windows (a
non-admin user **cannot** accept the firewall prompt), guest Wi-Fi with client
isolation, or across a VLAN. iroh inherits that limit because its local discovery
*is* mDNS.

**The constraint, stated plainly.** Automatic, secure, and serverless cannot all
hold at once. Discovering a peer does not prove it is yours; something must be
exchanged out of band exactly once. Designs that appear to avoid this have hidden
a bearer secret or a trusted third party.

---

## Part 6 — Placement

The question is not "which machine is fastest" but "is shipping this worth it."

```rust
fn worth_shipping(job: &JobEstimate, peer: &PeerStats, local: &LocalStats) -> bool {
    let Some(peer_ms)    = peer.median_ms_for(job.kind)          else { return false };
    let Some(result_len) = peer.median_result_bytes_for(job.kind) else { return false };
    let Some(throughput) = peer.throughput_bps                    else { return false };
    let Some(rtt_ms)     = peer.rtt_ms                            else { return false };
    let bytes = (job.payload_bytes + result_len) as f64;
    let transit_ms = (bytes / throughput.max(1.0)) * 1000.0 + rtt_ms * 2.0;
    (transit_ms + peer_ms) < local.median_ms_for(job.kind) * SHIP_MARGIN
}
```

Every input is measured; **absent any measurement, the job stays local.**
Revision 3 had three fields with no source: `expected_result_bytes` is
unknowable before running (it is now observed alongside duration), and
throughput and RTT come from vox's own differentiation of iroh's byte counters.

**Admission is separate from cost.** VRAM and `host_triple` are hard gates — a
16 GB model cannot run on a 6 GB GPU at any speed, and an `x86_64-pc-windows-msvc`
artifact cannot run on `aarch64-apple-darwin`. Revision 3 had no admission stage
at all. (Note the existing `select_best_node` admits on *total* VRAM, not free;
the `Probe` response must report free.)

**One scheduler, not two.** `models/registry.rs` and `scoring.rs` already rank
mesh peers with cost, latency, and reputation terms. Machine placement moves to
`vox-mesh-policy::cost`; `ProviderType::PopuliMesh` survives as "run this via the
mesh," but *which machine* stops being the model scorer's business.

**Build the data before the model.** Ship placement records and the Axis table
first, routing on hard admission plus one honest rule — *ship if the peer has a
GPU and this machine does not and the payload is small*. Record what the medians
*would* have predicted without acting on them. Requirement 6 is satisfied on day
one, the GPU case works immediately, and the model switches on only when the
recorded data shows it would have done better. If it wouldn't have, we deleted a
model instead of shipping one.

### Honest telemetry

The counterfactual is never observed: for a shipped job, "saved" is an *estimate*
minus a *measurement*. So the Axis panel shows estimated net saving **beside
median calibration error**, counts jobs that stayed home when the estimate said
ship, books failed ships at their full cost, and tags exploratory dispatches so
they are excluded from calibration.

---

## Part 7 — VoxScript and automatic distribution

**Question:** can all vox scripts be scalable, parallelizable, and grid-computable
with no new decorators?

**Answer: no for arbitrary functions, and the research is unambiguous.** No
system has achieved it. Unison makes `Location` the *first argument* to `forkAt`
and its design doc forbids implicit node contact. Cloud Haskell's `Closure` /
`static` is a **type-level tax paid because transparency is impossible**. Erlang
takes `spawn(Node, M, F, A)`. Chapel and X10 both deliberately made the
local/remote boundary syntactically visible, on the grounds that invisible
communication destroys performance predictability — that is prior art *against*
hiding it.

**What the enabling condition actually is** — and revision 3's instinct was only
half right. It is not purity, and it is definitely not element cost (Spark's CBO
estimates SQL cardinality; Dask has a flat overhead constant; Ray has nothing).
It is **re-executability**: deterministic, idempotent, serializable, and
appropriately sized. Spark models it as a three-valued `DeterministicLevel`
lattice; Ray states "tasks are assumed to be deterministic and idempotent"; Dask
makes it the literal `pure=` parameter.

**Where vox could be better positioned — and is not yet.** Those systems assert
purity because they cannot prove it, and the barrier is not Python's dynamism:
Spark's `ClosureCleaner` does JVM bytecode analysis in statically-typed Scala and
still only *approximates* capture.

An earlier draft of this section claimed "vox can check" the assertion those
systems must ask for. **That was false, and it was the premise the whole section
turned on.** Verified in `crates/vox-compiler/src/typeck/effect_check.rs`:

- `infer_expr_effects`'s `Call` arm (L413) recurses into the callee *expression*
  — an `Ident`, a leaf — and the arguments. It never consults the callee's
  `cap_map` entry or body. **Effects propagate exactly one hop.** A `@pure fn`
  calling an undeclared helper that calls `http.get` passes with zero violations.
- Effects are detected only by matching a method call whose receiver is in an
  8-entry hardcoded allowlist (L497).
- Inline lambdas passed to `map` *are* checked; a **named function reference is
  not** — `xs.map(fetch_one)` is entirely ungoverned, and `vox-codegen`'s
  `is_closure_arg` accepts it.

So today vox checks *less* than `ClosureCleaner`. Bottom-up inference is planned
as P1-T6 and has not landed; `effect_inference.rs`, `serializable.rs`, and
`workflow_determinism.rs` do not exist. The fix is small — `placement.rs::infer`
is already a real `while changed` fixpoint over call edges in the adjacent pass,
and copying it is a day's work, not research.

**And the corpus has no auto-distributable surface today.** Measured:

| | |
|---|---|
| `.vox` files under `scripts/` | 84 |
| …touching `fs.` / `process.` / `http.` | **74 (88%)** |
| `.map(` / `.filter(` / `.fold(` in `scripts/` | **0** |
| `for … in` loops in `scripts/` | **143** |
| files declaring `@pure` in `scripts/` + `examples/` | **2** |

Auto-parallelization attaches to combinators. This corpus is imperative loops
over I/O-bound glue **by policy** — VoxScript-First is explicitly about CI prep,
install helpers, and data migrations. A perfect implementation would light up
**zero of the 84 scripts**, and that is not a defect to engineer around; it is
what those scripts are for.

**So the honest sequencing is three steps, and only the first is unconditional:**

1. **Make purity real** (P1-T6). Replace the one-hop walk with the fixpoint
   `placement.rs` already implements, and attribute effects to `Ident` arguments
   in call position. **This pays for itself with no distribution story at all** —
   it fixes `@pure`, `@place`, and workflow determinism, all of which are
   currently unsound at two hops.
2. **Intra-node parallelism.** `vox-codegen` emits `map` as
   `.into_iter().map(f).collect()`; with a proven-pure element closure and a
   `Send` element type, emit `.into_par_iter()`. Rayon is already a workspace
   dependency. Roughly ten lines in one emitter — and it is *parallelization, not
   distribution*, so it dodges partial failure, latency, and serialization
   entirely. That is why it is cheap, and why it should ship first.
3. **Distribution.** Only after the accounting in Part 6 shows shipping pays.

**Two corrections to the earlier framing, both load-bearing:**

- **Purity is not the mobility blocker; code identity is.** Haskell has the
  strongest purity guarantee of any language here and still needed *more* syntax
  — `static` / `mkClosure` / a static pointer table — because a closure is a code
  address meaningless in another process. Unison solves it with content-addressed
  hashes. Either answer is structural and expensive. Vox would need one.
- **Waldo's impossibility argument is narrower than usually quoted.** §4 ranks
  the four differences and calls latency "the least fundamental"; the
  logical-impossibility claim is scoped to partial failure and concurrency, and
  even there reads "does not even *seem* to be logically possible." §8 explicitly
  carves out objects in different address spaces **under a single resource
  manager**: "partial failure and the indeterminacy that it brings can be
  avoided." A personal mesh with one originator per task and a single
  authoritative failure detector sits inside that carve-out — which is also why
  Spark, Dask, and Ray can offer near-transparent distribution where CORBA could
  not. Cite §8, not a slogan.

**What is not attainable, and should not be promised:** anything touching
non-serializable state (file handles, sockets, GPU contexts, actor refs),
anything whose captured environment is large enough that shipping costs more than
computing, and anything non-deterministic — where the failure mode is *silently
wrong answers*, not errors. Spark shipped exactly that bug for years
(SPARK-23207: `repartition` returning 931,532 of 1,000,000 rows after an executor
was killed).

**The bottleneck will be economics, not analysis.** Cilk's `cilk_spawn` is
permission rather than command and costs 2–6× a function call, so being wrong is
free. A network hop is 3–5 orders of magnitude more expensive, so the same design
needs a *predictive* model where Cilk needs none. The field's best answer to
granularity — heartbeat scheduling — is **ex post**, and cannot port to an
**ex ante** offload decision. Auto-parallelizing compilers of the 1990s
frequently made correctly-parallelized code *slower*.

**Therefore: build the accounting first.** GHC reports sparks as
converted / fizzled / GC'd / overflowed — a runtime telling you how often it
declined your advice. Without per-decision accounting of "shipped it and lost,"
a runtime cost model is unfalsifiable folklore. That is the same conclusion Part
6 reaches from the operational side, and it is why the telemetry is the first
deliverable rather than the last.

Worth noting as a caution rather than a precedent: GHC's `par` has exactly the
property this design wants — purity guaranteed by the type system, an advisory
annotation, and a runtime free to decline — and it still proved hard to use well,
because programmers could not predict granularity. The community moved to
`Strategies` and then to explicit dataflow. **Purity was necessary and nowhere
near sufficient.**

**Scope for this document: none of it.** Part 7 records the finding so the
question is settled with evidence. Language-level distribution is a separate
spec that should not start until the mesh moves bytes.

---

## Part 8 — Cryptography policy

The `ring` ban's goals were (a) unify crypto under `vox-crypto`, (b) improve
security, (c) reduce dependencies. Scored honestly:

- **(a) is already failing by two providers.** Root `Cargo.toml:251` pins
  `rustls` to `ring` deliberately; `workspace-hack` enables `rustls/aws-lc-rs` on
  all four target sections, via reqwest's `rustls-tls`. Both ship today, and
  `ring` is on the SSOT's own banned list — **the root manifest contradicts the
  SSOT at line 251.**
- **The gate has never run.** `scanner.rs:132` drops every file whose language is
  `Unknown`, and `from_extension("toml")` is `Unknown`, so `crypto_ban.rs`'s
  manifest branch is unreachable. Its tests pass by constructing `SourceFile`
  directly, **bypassing the scanner that would have excluded them.**
- **The cmake/nasm fear is empirically false.** `aws-lc-sys` is built in this
  tree, on Windows, with neither tool on PATH — its own build log says
  `NASM command not found` and it succeeded via prebuilt objects.
- **(b) iroh helps most.** We currently hand-roll an ed25519 envelope and a JWT
  auth matrix outside `vox-crypto`. That is where the RCE was.
- **(c) iroh makes the literal metric worse** — a QUIC + NAT-traversal stack has
  more transitive crates than an axum router — and the real metric better, by
  trading ~7,000 first-party lines for vendored, tested ones.

**Revision: split the policy in two.**

1. **Application crypto** — anything vox performs on vox's data — MUST go through
   `vox-crypto`. Enforceable by source scan; keep it.
2. **Transport crypto** lives inside vetted libraries, is **allowlisted, pinned,
   and ledgered**, and is checked against the **resolved lockfile**, not manifest
   text. Feature unification is per-package, so a single crate enabling a
   provider reintroduces it workspace-wide — which is exactly what happened.
3. **The build-toolchain invariant** — a clean clone builds with only the Rust
   toolchain, the platform C compiler, and Node/pnpm — is a property of the
   *build*, verified by CI on an image without cmake/nasm/Go/perl/libclang, not
   by blacklisting crate names.

**Open decision (yours):** standardize on **`ring`** (already the deliberate pin,
iroh's default; requires amending the SSOT's ban) or on **`aws-lc-rs`** (rustls's
default, FIPS-capable, empirically toolchain-free on Windows; requires changing
line 251 and the reqwest features). Either way, fix the reqwest feature selection
and replace the dead textual detector with a lockfile gate.

**Identity:** `vox-crypto` pins `ed25519-dalek` 2.x, iroh pins 3.0-rc, so they
cannot share a Rust type — but both round-trip through 32 bytes. Do **not**
unify: the mesh key must start headless while `NodeIdentity` is password-sealed,
so fusing them either breaks unattended start or unseals the signing key.

---

## Part 9 — Process topology

Three processes: **Axis** → TCP `127.0.0.1:9745` → **`vox-orchestrator-d`**
(where `vox_mesh_nodes` executes and where the iroh `Endpoint` lives) → and
today a separate `vox populi serve`, being retired.

**Unresolved in revision 3 and resolved here:** `vox mesh` CLI verbs run in a
*different process* from the daemon. They connect to the daemon over its existing
IPC and, if it is not running, start it. The user must never learn the word
`vox-orchestrator-d`, and a ticket must never be minted by a process whose
addresses die when it exits.

---

## Part 10 — Risks

| Risk | Mitigation |
|---|---|
| iroh 1.1 is three months old | Confined to one crate. Pin exactly. **Its satellites are pre-1.0** (`iroh-blobs` 0.103, `iroh-mdns-address-lookup` 0.5) and can break on every minor — pin those with `=`. |
| Losing HTTP loses `curl` debugging | Loopback-only admin surface plus `vox mesh explain`. |
| The deletion is larger than the replacement | Deletion happens **last**, after the replacement is proven across two machines. |
| Placement model is wrong or gamed | Data before model (§6); estimate floor (§4.8); calibration error shown beside every claim of savings. |
| Version skew between machines | A frozen `Hello { proto, vox }` frame. Without it the likely week-one failure is a hang. |

---

## Part 11 — Reconciliation with ratified decisions

**ADR-020 §Decision.1 states the default remains HTTP Populi "until a future ADR
explicitly replaces ADR-008 as the default transport."** This design does exactly
that and therefore **requires a new ADR** — filed with the first implementation
commit, superseding ADR-008 and ADR-020, partially superseding ADR-017 (the
grant protocol, not the duplicate-execution guard), and **upholding ADR-018**
(probe-backed GPU truth is Layer A).

ADR-020 also *pre-authorizes* this move by naming QUIC as the future option — a
supporting argument revision 3 ignored while being in breach of the same ADR.

The mesh SSOT's non-goal that "paired peers + GitHub attestation are the binary
gates" is superseded for local peers. The attestation subsystem's *pairing* half
has no non-test callers; `PublicAttestationManifest` itself does, and the
amendment must say so.

---

## Part 12 — Revision history

| Rev | Approach | Superseded because |
|---|---|---|
| 0 | Tailscale-derived identity | Requires an account, a third-party coordinator, an install. |
| 1 | Hand-rolled mDNS as the default | 24 defects; mDNS unreachable on managed Windows, isolated guest Wi-Fi, across VLANs. |
| 2 | Ticket-first over the existing HTTP plane | Hand-rolled identity, tickets, discovery, liveness, and auth — each defective, each provided tested by a library. |
| 3 | iroh transport | Right direction; overstated the deletion, omitted four live capabilities, left the executor unsandboxed, and had six compile errors plus three cost-model fields with no data source. |
| **4** | **iroh + sandboxed execution + measured placement, deletion last** | — |

---

## Part 13 — Out of scope

- Language-level distribution (Part 7) — evidence recorded, work deferred.
- `iroh-blobs` for job payloads — **cut from v1**. A bi-stream already gives
  ordered, encrypted, flow-controlled bytes; blobs buys resume and dedup that a
  10 MB payload on a 3 ms LAN does not need, at the cost of a store, a GC policy,
  and a disk-full failure mode. It earns its place for **model and checkpoint
  distribution**, where files are gigabytes — introduce it there, as its own ALPN.
- `irpc` — cut. Three request variants over one bi-stream is four lines of
  postcard.
- `iroh-gossip`, mobile — direction, not work.
- Multi-originator leases — the DB guard stays; the grant protocol goes.
- Any vox-operated coordination service. Rejected, with a detector.
