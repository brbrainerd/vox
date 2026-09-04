---
title: "Populi Mesh: iroh Transport, Capability Scheduling, and Transit-Cost Routing"
description: "Design replacing the hand-rolled Populi mesh transport with iroh, reducing the subsystem to the capability scheduler that is vox's actual value-add, and adding a measured transit-cost model that decides per-machine whether shipping a job is worth it."
category: "Architecture SSOTs"
status: "current"
---

# Populi Mesh: iroh Transport, Capability Scheduling, and Transit-Cost Routing

**Status.** Revision 3, 2026-09-04. Revisions 0–2 are recorded below because each
was rejected for a reason worth keeping.

## Goal

When a user works in the CLI or in Axis, vox distributes CPU and GPU work across
the machines they own, gets out of the way, and shows honestly what that bought
or cost. The mesh should be something the user never has to think about.

## Requirements

Binding, and they order every decision. Unchanged since revision 1.

1. **Works out of the box.** Two machines with vox installed and no
   configuration can pair and share work.
2. **No central server.** Fully functional with Vox infrastructure switched off
   permanently.
3. **Works without internet.** LAN-only and air-gapped are supported.
4. **No account required.** No sign-up, no third-party identity provider.
5. **Secure by default.** The safe configuration is the one reached without
   reading documentation.
6. **Confirmed in the GUI.** Not done until visible and operable in Axis.

## Revision history

| Rev | Approach | Why rejected |
|---|---|---|
| 0 | Derive mesh identity from a Tailscale tailnet | Requires an account, an internet round-trip to a third-party coordinator, and a third-party install. Fails Requirements 2–4. |
| 1 | Hand-rolled mDNS discovery as the default | An eight-track audit found 24 defects. mDNS also cannot reach managed Windows (non-admin users cannot accept the firewall prompt), guest Wi-Fi with client isolation, or anything across a VLAN. |
| 2 | Pasteable `vox-mesh://` ticket first, mDNS as convenience, over the existing HTTP control plane | Correct in shape, but hand-rolls identity, tickets, discovery, liveness, and auth — each of which the audit found defective, and each of which a library provides tested. |
| **3** | **iroh for transport and identity; vox keeps capability scheduling** | — |

Revision 2's rejection of iroh was factually wrong and is corrected below.

---

## Part 1 — Why iroh

### The decisive detail

**An iroh `EndpointId` is an ed25519 public key**, and `Connection::remote_id()`
returns the peer's `EndpointId` derived from their TLS certificate — a
cryptographically bound identity, not a self-asserted string in a header or a
TXT record.

`TrustedNodeRegistry` is already keyed by pubkey. So the entire trust decision
becomes one check at accept time:

```rust
let remote = conn.remote_id()?;
if !trust.contains(&remote) { return; }   // drop the connection
```

That single check replaces the `FullAccess` / bearer / JWT / role matrix in
`transport/auth.rs`. **The unauthenticated-execute hole the audit found does not
exist in this model** — not because a guard was added, but because there is no
code path that accepts an unidentified peer. Revision 2 needed a blocking
security phase to patch that hole; revision 3 deletes the surface it lived on.

### What revision 2 got wrong about iroh

> "Both supply authenticated streams between opaque NodeIds rather than IP
> addresses, so adopting either means porting the axum router and every
> `http://host:port` reference onto their stream abstraction — a rewrite of a
> working transport."

Two errors. The transport was not working (`vox populi up` has never started a
server). And the streams are not an obstacle: `SendStream`/`RecvStream` implement
`AsyncWrite`/`AsyncRead`, so an axum router can be served over one via
`hyper_util::TokioIo` in a few dozen lines — though this design does not need to,
because it replaces the routes with a typed protocol instead.

### The correct reasons to have hesitated

Neither appeared in revision 2:

- iroh's own **local discovery is mDNS** (`iroh-mdns-address-lookup`), so
  adopting iroh does not avoid mDNS's network limitations — it inherits them.
  The ticket path from revision 2 remains necessary and remains the default.
- iroh's **`N0` preset contacts n0-operated relay and discovery infrastructure**,
  which Requirement 2 forbids. The `Minimal` preset is mandatory here, and that
  choice must be enforced, not merely documented.

---

## Part 2 — What iroh replaces

| Hand-rolled today | Replacement | Lines |
|---|---|---|
| axum control plane, ~25 routes, in **two near-identical copies** (`vox-populi/src/transport`, `vox-plugin-populi-mesh/src/transport`) | `Endpoint` + ALPN + typed protocol | **9,185** |
| bearer / JWT / `FullAccess` auth layer | `Connection::remote_id()` against an allowlist | + the RCE |
| `vox-mesh://` ticket (revision 2's design) | `iroh_tickets::EndpointTicket` | ~200, unwritten |
| bespoke mDNS module (revision 1's design) | `iroh-mdns-address-lookup` | ~400, unwritten |
| federation directory + announce | direct connection, or `iroh-gossip` later | ~400 |
| `bundle_fetch`, `op_fragment` | `iroh-blobs` (BLAKE3-verified, resumable, dedup) | ~600 |
| `tls.rs` (dead — no caller in `serve()`) | QUIC is encrypted by construction | ~150 |
| NAT traversal (does not exist) | provided | — |

Verified API surface (iroh 1.1.0):

- `Endpoint::bind(presets::Minimal)`, `Endpoint::builder(preset)`,
  `Builder::alpns(Vec<Vec<u8>>)`, `Endpoint::{connect, accept, addr, id, close}`
- `Connection::{open_bi, accept_bi, closed, remote_id, stats, rtt}`
- `presets::{Empty, Minimal, N0, N0DisableRelay}` — **`Minimal` is required**
- `MdnsAddressLookup::builder().build(endpoint.id())`, attached via
  `endpoint.address_lookup()?.add(mdns)`; `subscribe()` yields `DiscoveryEvent`
- `iroh_tickets::endpoint::EndpointTicket::new(endpoint.addr())`

### Deleting the lease system

Exec leases exist to prevent duplicate execution. With one originator per task,
**the QUIC connection is the lease**: if it drops, the job is orphaned and the
originator retries. `Connection::closed()` is the expiry signal.

This deletes grant / renew / expire / revoke and its persistence — roughly 1,500
lines — and removes a whole class of distributed-state bugs. A personal mesh of
2–10 machines the user owns does not need distributed consensus; it needs
reachability, a capability probe, and retry.

ADR-017 is superseded for the personal-mesh case. If a future multi-originator
tier appears, leases return with it.

---

## Part 3 — What survives, and why it matters

The parts worth keeping are the parts no library provides:

- **`NodeRecord` capability data** — probe-backed GPU truth, VRAM, CPU load,
  `host_triple`. iroh gives you a pipe between machines; nothing else knows which
  of your machines has 12 GB of VRAM free.
- **`select_best_node`** and the model-registry scoring.
- **`TrustedNodeRegistry`**, reduced to an allowlist of `EndpointId`s.
- **Local job queue and journal** for retry.

That is the actual product. Today it is buried under nine thousand lines of
transport plumbing that a library does better.

---

## Part 4 — Transit-cost routing

The scheduler's question is not "which machine is fastest" but "is shipping this
worth it at all." Because iroh exposes `Connection::stats()` and
`Connection::rtt()`, the inputs are **measured rather than configured**:

```rust
/// Should this job go to `peer` instead of running locally?
fn worth_shipping(job: &Job, peer: &PeerStats, local: &LocalStats) -> bool {
    let bytes = job.payload_bytes + job.expected_result_bytes;
    let transit_ms = (bytes as f64 / peer.throughput_bps) * 1000.0 + peer.rtt_ms * 2.0;
    let remote_ms = transit_ms + peer.median_ms_for(job.kind);
    remote_ms < local.median_ms_for(job.kind) * SHIP_MARGIN
}
```

`median_ms_for` is a rolling median of observed durations keyed by
`(job_kind, endpoint_id)`. No model, no configuration, no cold-start guessing:
**with no history, the job runs locally.** The estimator self-calibrates from
real outcomes.

`SHIP_MARGIN` is 0.8 — a job must be at least 20 % faster remotely to be worth
shipping. Marginal wins are not worth the failure modes.

// ponytail: rolling median over the last 16 samples per (kind, endpoint);
// a real latency model if the median proves too coarse for heterogeneous GPUs.

**Surfacing gains and losses (Requirement 6).** Axis shows, per job: where it
ran, estimate versus actual, and cumulative time saved or lost. This is the
feedback loop that makes the estimator trustworthy — and auditable when it is
wrong.

---

## Part 5 — Pairing and trust

### Tiers, ordered by coverage

| Tier | Mechanism | Coverage | Status |
|---|---|---|---|
| **T0** | loopback | one machine | always |
| **T1** | `EndpointTicket` paste | **every network** | **default** |
| **T2** | `iroh-mdns-address-lookup` | most home/office LANs | convenience over T1 |
| **T3** | self-hosted iroh relay | cross-site | opt-in |
| **T4** | vox-operated rendezvous | — | **prohibited** |

T1 stays the default for the reason revision 2 established: no multicast
mechanism reaches managed Windows, isolated guest Wi-Fi, or across a VLAN, and
the blocker is the firewall and the access point, not the wire format. iroh
inherits that limitation because its local discovery *is* mDNS.

The ticket adds no friction the trust model did not already require — an
out-of-band step is unavoidable (see below), and `EndpointTicket` collapses
discovery and identity into one pasteable string.

**T3 is compatible with the T4 prohibition** because the user opts into a relay
they run. n0's relays are not used: the `Minimal` preset contacts no third party,
and a lint must enforce that the `N0` preset never appears in vox code.

### The constraint, stated plainly

Automatic, secure, and serverless cannot all hold at once. Discovering a peer
does not prove it is yours. Something must be exchanged out of band exactly once.
Designs that appear to avoid this have hidden a bearer secret or a trusted third
party.

### Flow

1. B prints its ticket (`vox mesh ticket`, or Axis displays it). It carries B's
   `EndpointId` — a public key — and its addresses.
2. A consumes it: paste into Axis, or `vox auth trust <ticket>`.
3. A adds B's `EndpointId` to the allowlist. B does the same for A.
4. Every later connection is authenticated by TLS to that key. An unknown
   `remote_id()` is dropped before any protocol handler runs.

T2 inserts one optional step: a discovered peer appears with its `EndpointId`,
and approving it performs step 3 without a paste. **The displayed identity is the
key itself**, so the comparison the user performs is the thing the system later
enforces — unlike revision 1, where a fingerprint was displayed and a node_id was
stored.

### Security properties

- A hostile peer on the LAN can be discovered and can do nothing else: no
  protocol handler runs before the allowlist check.
- Trust binds to the key, so it survives address changes and cannot be claimed by
  announcing someone else's identifier.
- Revocation is local, immediate, and drops live connections.
- Ticket contents are public; a ticket grants the holder nothing until the far
  side reciprocates.

**Rejected: pairing codes as the default.** A short code is a bearer secret;
anyone who observes it joins silently, failing Requirement 5.

---

## Part 6 — Component design

### Process topology

Three processes, and this constrains everything:

1. **Axis** (`vox-gui`) — no in-process MCP host; it was deliberately removed.
2. **`vox-orchestrator-d`** — a child binary over TCP `127.0.0.1:9745`. **This is
   where `vox_mesh_nodes` executes.**
3. **`vox populi serve`** — a separate child process.

Revision 1 stored discovery in a process-global read from a different process. In
this design **the iroh `Endpoint` lives in `vox-orchestrator-d`**, which is the
process that both serves the GUI's tool calls and dispatches work. `vox populi
serve` is retired along with the HTTP plane.

### New crate: `vox-mesh`

Roughly 800 lines, replacing ~10,000. One crate so an iroh 2.0 is one file:

```text
identity.rs   persisted SecretKey; EndpointId = its public key
endpoint.rs   Endpoint::bind(presets::Minimal) + MdnsAddressLookup + StaticProvider
trust.rs      EndpointId allowlist, wrapping TrustedNodeRegistry
protocol.rs   ALPN b"vox/job/1"; Probe / Run / Cancel over one bi-stream
cost.rs       worth_shipping(), rolling medians, PeerStats from Connection::stats()
ticket.rs     thin wrapper over iroh_tickets::EndpointTicket
```

The job protocol is deliberately small:

```rust
enum JobRequest {
    Probe,                          // -> Capabilities (GPU, VRAM, load, host_triple)
    Run { kind: JobKind, blob: Hash },
    Cancel { job_id: JobId },
}
```

Payloads travel as `iroh-blobs` hashes rather than inline bytes: content-addressed,
resumable, deduplicated across jobs, and BLAKE3-verified on arrival.

### Crate edges

`vox-mesh` is a new L2 crate depending on `vox-identity` (L1), `vox-mesh-types`
(L1), and iroh. Consumers are `vox-orchestrator-mcp` and `vox-ml-cli` (both L4).
These are new workspace edges and **require user authorization** per `AGENTS.md`
§Dependency Discipline. They are requested explicitly in the plan, not assumed.

---

## Part 7 — Deletion inventory

Confirmed by symbol search to have zero consumers outside their own crate:

| Item | Lines |
|---|---|
| `vox-plugin-populi-mesh/src/transport` (dormant — `MeshDriver::start_transport` has no non-test caller) | 3,702 |
| `vox-populi/src/transport` (live, replaced) | 5,483 |
| `pairing/{device_flow,github_attestation}` | 422 |
| `quota/` (no external consumers) | 267 |
| `vox-mesh-types`: `quorum`, `tee_attestation`, `secret_sync`, `model_inventory`, `op_fragment`, `kudos` | ~600 |
| `tls.rs` | ~150 |
| exec-lease machinery | ~1,500 |

**~12,000 lines deleted, ~1,000 written**, subject to a real accounting during
implementation. `A2ADeliverRequest` (16 consumers) and the capability types stay.

---

## Part 8 — Reconciliation with the ratified mesh SSOT

[`mesh-and-language-distribution-ssot-2026.md`](../../src/architecture/mesh-and-language-distribution-ssot-2026.md)
line 101 states a binding non-goal (council decision D16): *"Paired peers +
GitHub attestation are the binary gates."* Requirement 4 is incompatible with an
attestation gate that requires a third-party identity provider and an internet
round-trip.

The conflict is smaller than it looks: the gate was never wired.
`fetch_and_verify`, `device_flow`, `PublicAttestationManifest`, and
`verify_against_trust` all have zero non-test callers; `pairing_e2e.rs` is a
one-line placeholder; and `vox populi join` writes a config key nothing reads.

This design supersedes the enrollment half of Phase 5/6. **An amendment must be
filed with the first implementation commit** — a ratified SSOT is not amended
silently.

---

## Part 9 — Risks

| Risk | Mitigation |
|---|---|
| iroh 1.0 is three months old (June 2026; 1.1 August, after 65 pre-releases) | Confined to `vox-mesh`. Pin exactly. A 2.0 touches one crate. |
| Losing HTTP means losing `curl` debugging | Keep a loopback-only HTTP admin surface, plus `vox mesh call` for protocol probes. |
| It is a rewrite | Most of what is rewritten is dead or broken. `vox populi up` has never started a server. |
| `AGENTS.md` bans `ring` | iroh's active provider is `aws-lc-rs`. `ring`, `aws-lc-rs`, `quinn`, and `rustls` are **already in `Cargo.lock`**, so this does not worsen the policy — but it does make its aspirational status hard to ignore. Decide whether the rule is real. |
| Accidentally shipping the `N0` preset | A `vox-code-audit` detector failing on `presets::N0` in vox code. Requirement 2 needs a gate, not a comment. |

---

## Part 10 — Future direction

Each of these is a new ALPN or an existing iroh module, not a transport change:

- **`iroh-blobs` for model and checkpoint distribution** — content-addressed,
  resumable, deduplicated. This is workstream W4 (mesh model inventory) with the
  hard part already solved, and it matters most for MENS checkpoints.
- **`iroh-gossip`** for mesh-wide state without a directory.
- **Mobile** — iroh runs on iOS and Android, so the Tauri mobile targets inherit
  the mesh.
- **New protocols** are new ALPNs; the transport is never touched again.
- **Cross-site** via a user-run relay, never a vox-run one.

---

## Part 11 — Out of scope

- Internet-peer attestation (Part 8) — neither implemented nor forbidden.
- `iroh-gossip`, `iroh-docs`, mobile — named as direction, not built.
- Cross-node secret pairing (W3) and trace propagation (W5).
- Multi-originator leases — deleted, and returning only with a tier that needs them.
- Any vox-operated coordination service. Rejected, and Part 9 asks for a detector.
