# Populi Mesh on iroh — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vox distributes CPU and GPU work across a user's own machines, decides per-machine whether shipping a job is worth it using measured transport statistics, and shows honestly what that bought — with no account, no internet, and no central server.

**Architecture:** iroh provides transport, identity, NAT traversal, and discovery. Vox keeps the capability scheduler, which is the part no library provides. ~12,000 lines of hand-rolled transport, auth, and lease machinery are deleted.

**Tech Stack:** Rust 1.96, `iroh 1.1`, `iroh-mdns-address-lookup`, `iroh-tickets`, `iroh-blobs`, `irpc`, Tauri 2 + React (Axis).

**Spec:** [`docs/superpowers/specs/2026-09-04-populi-mesh-iroh-transport-design.md`](../specs/2026-09-04-populi-mesh-iroh-transport-design.md) — revision 3.

**Provenance:** Revision 3. Revision 1 was audited across eight tracks and found to have 24 defects; revision 2 fixed them by hand; revision 3 deletes the surface most of them lived on. Findings verified *correct* by that audit are in the appendix — do not relitigate them.

## Global Constraints

- **Two new crate edges require your authorization before Phase 1 starts.** `vox-orchestrator-mcp → vox-mesh` and `vox-ml-cli → vox-mesh`. `AGENTS.md` §Dependency Discipline: *"exceptions entries are USER-AUTHORIZED-ONLY."* Task 0.2 requests them; **do not write the ledger entry yourself.**
- **`presets::Minimal` only.** `presets::N0` and `N0DisableRelay` contact n0-operated infrastructure and violate Requirement 2. Task 1.2 adds a detector.
- **Trust is checked before any protocol handler runs.** `conn.remote_id()` against the allowlist, then drop. No exceptions, no "just for local".
- **Three processes.** Axis → TCP `127.0.0.1:9745` → `vox-orchestrator-d` (where `vox_mesh_nodes` executes and where the iroh `Endpoint` lives) → `vox populi serve` (being retired). Verify which process your code runs in before storing state in one.
- **Test-first** (`AGENTS.md`): the detector is file-granular — every file with a `pub fn` needs at least one `#[test]` in it.
- **Formatting:** `vox run scripts/fmt.vox`. **Never** `cargo fmt --all` (Windows `os error 206`).
- **Pre-push:** `vox ci pre-push --complete`; the fast tier runs no clippy.
- Deletion happens in Phase 3, *after* the replacement works. Do not delete and rebuild in the same phase.

---

## Phase 0 — Spike and authorization

### Task 0.1: Throwaway two-machine spike

**Output is an answer, not code you keep.** Label everything throwaway. If this fails, the rest of the plan is void and we return to revision 2.

- [ ] **Step 1: Scratch binary**

Outside the workspace (`$SCRATCH/iroh-spike/`), a two-mode binary:

```rust
// throwaway spike — not for merge
use iroh::{Endpoint, endpoint::presets};
use iroh_tickets::endpoint::EndpointTicket;

const ALPN: &[u8] = b"vox/spike/0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ep = Endpoint::bind(presets::Minimal).await?;
    match std::env::args().nth(1).as_deref() {
        Some("serve") => {
            println!("ticket: {}", EndpointTicket::new(ep.addr()));
            while let Some(incoming) = ep.accept().await {
                let conn = incoming.await?;
                println!("peer: {}", conn.remote_id()?);
                let (mut send, mut recv) = conn.accept_bi().await?;
                let msg = recv.read_to_end(64 * 1024).await?;
                send.write_all(&msg).await?;
                send.finish()?;
            }
        }
        Some("dial") => {
            let ticket: EndpointTicket = std::env::args().nth(2).unwrap().parse()?;
            let conn = ep.connect(ticket.addr().clone(), ALPN).await?;
            let (mut send, mut recv) = conn.open_bi().await?;
            send.write_all(b"ping").await?;
            send.finish()?;
            println!("echo: {:?}", recv.read_to_end(1024).await?);
            println!("rtt: {:?}", conn.rtt(Default::default()));
            println!("stats: {:?}", conn.stats());
        }
        _ => anyhow::bail!("serve | dial <ticket>"),
    }
    Ok(())
}
```

The builder needs `.alpns(vec![ALPN.to_vec()])` on the serve side — `Endpoint::bind(preset)` takes no ALPNs, so use `Endpoint::builder(presets::Minimal).alpns(...)`. **Confirm the exact builder chain against the real docs**; iroh 1.1 is three months old and doc summaries are lossy.

- [ ] **Step 2: Answer five questions and write them down**

1. Does `presets::Minimal` connect with **zero** third-party contact? Verify with a packet capture or by running both machines with the internet disconnected.
2. Does LAN connection succeed with no relay?
3. What do `conn.stats()` and `conn.rtt()` actually expose — is there a usable throughput figure, or only RTT and byte counters? **The cost model in Phase 2 depends on this.** If throughput is not available, the model must derive it from bytes-over-time itself.
4. Does `MdnsAddressLookup` find the peer on your LAN, and does it survive the Windows firewall prompt?
5. What is the real dependency-tree weight and build-time delta?

- [ ] **Step 3: Report and decide**

Write findings to `docs/src/architecture/iroh-spike-findings-2026.md` with frontmatter. If question 1 or 3 fails, **stop and reassess** — those are the two load-bearing assumptions.

- [ ] **Step 4: Delete the spike.** Its output is the findings doc.

### Task 0.2: Request crate-edge authorization

- [ ] **Step 1: Present the request**

New crate `vox-mesh` (L2), consumed by `vox-orchestrator-mcp` (L4) and `vox-ml-cli` (L4). Both edges point downward and are layer-legal, but the exact-edge ratchet still requires an `exceptions` entry in `contracts/ci/crate-edges.allow.v1.json`.

Also required: a layer assignment for `vox-mesh` in `contracts/ci/crate-layers.v1.json`, and a row in `docs/src/architecture/where-things-live.md`.

- [ ] **Step 2: STOP. Wait for explicit authorization.** Do not write the ledger entry. Do not regenerate the baseline. If authorization is declined, the alternative is to place `vox-mesh` inside `vox-populi` (which already has the consumers' edges) at the cost of keeping a 26,000-line crate alive.

---

## Phase 1 — The `vox-mesh` crate

### Task 1.1: Identity and endpoint

**Files:** Create `crates/vox-mesh/{Cargo.toml,src/lib.rs,src/identity.rs,src/endpoint.rs}`

- [ ] **Step 1: Write the failing test**

```rust
//! Persisted mesh identity. The `EndpointId` IS the ed25519 public key, so the
//! trust registry and the transport share one identifier with no mapping layer.

use iroh::SecretKey;
use std::path::Path;

/// Load the mesh secret key, generating and persisting one on first run.
///
/// Separate from `vox_identity`'s node identity: that one is password-sealed and
/// may be locked, which must never prevent the mesh from starting.
pub fn load_or_create(path: &Path) -> anyhow::Result<SecretKey> { /* … */ }

/// Hex of the public key — the stable, shareable node identifier.
pub fn public_hex(sk: &SecretKey) -> String {
    hex::encode(sk.public().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_key_is_stable_across_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        let a = public_hex(&load_or_create(&p).unwrap());
        let b = public_hex(&load_or_create(&p).unwrap());
        assert_eq!(a, b, "identity must survive restart or every pairing breaks");
    }

    #[test]
    fn the_public_hex_is_64_chars_matching_the_trust_registry_format() {
        let dir = tempfile::tempdir().unwrap();
        let h = public_hex(&load_or_create(&dir.path().join("k")).unwrap());
        assert_eq!(h.len(), 64);
        assert!(h.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn a_corrupt_key_file_is_an_error_not_a_silent_new_identity() {
        // Silently regenerating would orphan every peer that trusted the old key.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        std::fs::write(&p, b"not a key").unwrap();
        assert!(load_or_create(&p).is_err());
    }
}
```

- [ ] **Step 2: Run to verify it fails, implement, verify PASS**

`cargo test -p vox-mesh identity`

- [ ] **Step 3: Endpoint construction**

```rust
/// Bind the mesh endpoint.
///
/// `presets::Minimal` is mandatory: `N0` and `N0DisableRelay` contact
/// n0-operated relay and discovery infrastructure, which Requirement 2 forbids.
pub async fn bind(sk: SecretKey) -> anyhow::Result<Endpoint> {
    let ep = Endpoint::builder(presets::Minimal)
        .secret_key(sk)
        .alpns(vec![crate::protocol::ALPN.to_vec()])
        .bind()
        .await?;
    let mdns = MdnsAddressLookup::builder().build(ep.id())?;
    ep.address_lookup()
        .ok_or_else(|| anyhow::anyhow!("address lookup unavailable"))?
        .add(mdns);
    Ok(ep)
}
```

Adapt the builder chain to the real 1.1 API confirmed in Task 0.1 — `bind()` versus `Endpoint::bind(preset)` differ, and `secret_key` may be named differently.

- [ ] **Step 4: Commit**

### Task 1.2: Trust allowlist and the preset detector

**Files:** Create `crates/vox-mesh/src/trust.rs`; add a `vox-code-audit` detector

- [ ] **Step 1: Write the failing test**

```rust
/// Allowlist of trusted `EndpointId`s, backed by the existing
/// `~/.vox/trusted_nodes.json` store.
///
/// The check is the whole authorization model: an unknown `remote_id()` is
/// dropped before any protocol handler runs.
pub struct MeshTrust { /* … */ }

impl MeshTrust {
    pub fn is_trusted(&self, id: &EndpointId) -> bool { /* … */ }
    pub fn trust(&self, id: &EndpointId, label: Option<String>) -> anyhow::Result<()> { /* … */ }
    pub fn untrust(&self, id: &EndpointId) -> anyhow::Result<bool> { /* … */ }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untrusted_endpoint_is_refused() {
        let (_d, t) = temp_trust();
        assert!(!t.is_trusted(&random_id()));
    }

    #[test]
    fn trust_persists_across_handles() {
        let dir = tempfile::tempdir().unwrap();
        let id = random_id();
        MeshTrust::at(dir.path()).trust(&id, None).unwrap();
        assert!(MeshTrust::at(dir.path()).is_trusted(&id), "must survive restart");
    }

    #[test]
    fn untrust_takes_effect_immediately() {
        let (_d, t) = temp_trust();
        let id = random_id();
        t.trust(&id, None).unwrap();
        assert!(t.untrust(&id).unwrap());
        assert!(!t.is_trusted(&id));
    }

    #[test]
    fn a_registry_read_error_fails_closed() {
        // An unreadable allowlist must deny, never allow.
        let t = MeshTrust::at(std::path::Path::new("/nonexistent/dir"));
        assert!(!t.is_trusted(&random_id()));
    }
}
```

- [ ] **Step 2: Run to verify fail, implement, verify PASS**

- [ ] **Step 3: Add the `presets::N0` detector**

Requirement 2 needs a gate, not a comment. Add a `vox-code-audit` detector failing at `Error` severity on `presets::N0` or `N0DisableRelay` in workspace code, with a test asserting it fires.

- [ ] **Step 4: Commit**

### Task 1.3: Job protocol

**Files:** Create `crates/vox-mesh/src/protocol.rs`

- [ ] **Step 1: Write the failing test**

```rust
pub const ALPN: &[u8] = b"vox/job/1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobRequest {
    /// Ask what this peer can do right now.
    Probe,
    /// Run a job whose payload is an iroh-blobs hash.
    Run { kind: JobKind, blob: Hash },
    Cancel { job_id: JobId },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobResponse {
    Capabilities(Box<NodeCapabilities>),
    Accepted { job_id: JobId },
    Completed { job_id: JobId, result: Hash, duration_ms: u64 },
    Failed { job_id: JobId, error: String },
    Refused { reason: RefusalReason },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_through_postcard() {
        let r = JobRequest::Run { kind: JobKind::Compile, blob: Hash::from([7u8; 32]) };
        assert_eq!(postcard::from_bytes::<JobRequest>(&postcard::to_allocvec(&r).unwrap()).unwrap(), r);
    }

    #[test]
    fn the_alpn_is_versioned_so_a_v2_peer_cannot_half_speak_v1() {
        assert!(ALPN.ends_with(b"/1"));
    }

    #[test]
    fn a_failed_response_carries_a_reason_not_just_a_flag() {
        // Opaque remote failures were the top operator complaint in the
        // north-star audit; the type makes an empty reason unrepresentable.
        let f = JobResponse::Failed { job_id: JobId::new(), error: "oom".into() };
        match f { JobResponse::Failed { error, .. } => assert!(!error.is_empty()), _ => panic!() }
    }
}
```

- [ ] **Step 2: Run to verify fail, implement, verify PASS**

- [ ] **Step 3: Accept loop with the trust gate first**

```rust
pub async fn serve(ep: Endpoint, trust: Arc<MeshTrust>, exec: Arc<dyn JobExecutor>) {
    while let Some(incoming) = ep.accept().await {
        let (trust, exec) = (Arc::clone(&trust), Arc::clone(&exec));
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else { return };
            // The entire authorization model. Nothing below runs for a stranger.
            let Ok(remote) = conn.remote_id() else { return };
            if !trust.is_trusted(&remote) {
                tracing::debug!(%remote, "refusing untrusted peer");
                return;
            }
            handle(conn, remote, exec).await;
        });
    }
}
```

- [ ] **Step 4: Integration test — the security property**

```rust
#[tokio::test]
async fn an_untrusted_peer_cannot_reach_the_executor() {
    let (server, _t) = server_trusting_nobody().await;
    let client = Endpoint::bind(presets::Minimal).await.unwrap();
    let conn = client.connect(server.addr(), ALPN).await;
    // Either the connect fails or the stream closes without the executor running.
    assert!(conn.is_err() || probe_times_out(conn.unwrap()).await);
    assert_eq!(executor_invocations(), 0, "a stranger reached the executor");
}
```

This is the test that replaces revision 2's entire Phase 0. It must pass before Phase 2.

- [ ] **Step 5: Commit**

---

## Phase 2 — Scheduling and cost

### Task 2.1: Transit-cost model

**Files:** Create `crates/vox-mesh/src/cost.rs`

- [ ] **Step 1: Write the failing test**

```rust
/// A job must be at least 20% faster remotely to be worth shipping.
/// Marginal wins are not worth the failure modes.
const SHIP_MARGIN: f64 = 0.8;

pub fn worth_shipping(job: &JobEstimate, peer: &PeerStats, local: &LocalStats) -> bool {
    let Some(peer_ms) = peer.median_ms_for(job.kind) else { return false }; // no history: stay local
    let bytes = (job.payload_bytes + job.expected_result_bytes) as f64;
    let transit_ms = (bytes / peer.throughput_bps.max(1.0)) * 1000.0 + peer.rtt_ms * 2.0;
    (transit_ms + peer_ms) < local.median_ms_for(job.kind) * SHIP_MARGIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cold_peer_is_never_chosen() {
        // No observed history means no estimate. Guessing is how you lose time.
        assert!(!worth_shipping(&job(1_000, 1_000), &cold_peer(), &local(10_000.0)));
    }

    #[test]
    fn a_much_faster_peer_wins_despite_transit() {
        let peer = peer_with(1_000.0, 5.0, 1e8); // 1s compute, 5ms rtt, 100Mbps
        assert!(worth_shipping(&job(1_000_000, 1_000), &peer, &local(10_000.0)));
    }

    #[test]
    fn a_marginally_faster_peer_is_not_worth_it() {
        let peer = peer_with(9_000.0, 5.0, 1e8);
        assert!(!worth_shipping(&job(1_000, 1_000), &peer, &local(10_000.0)));
    }

    #[test]
    fn a_huge_payload_over_a_slow_link_stays_local() {
        // The case the model exists for: fast GPU, terrible link.
        let peer = peer_with(100.0, 200.0, 1e6); // 1Mbps
        assert!(!worth_shipping(&job(500_000_000, 1_000), &peer, &local(10_000.0)));
    }

    #[test]
    fn zero_throughput_does_not_divide_by_zero() {
        let peer = peer_with(1.0, 5.0, 0.0);
        let _ = worth_shipping(&job(1_000, 1_000), &peer, &local(10_000.0));
    }
}
```

- [ ] **Step 2: Run to verify fail, implement, verify PASS**

- [ ] **Step 3: Rolling medians**

```rust
/// Rolling median of the last 16 observed durations per (kind, endpoint).
/// Self-calibrating: no configuration, no model, no cold-start guess.
///
// ponytail: 16-sample median; a real latency model if heterogeneous GPUs make
// the median too coarse.
pub struct Observations { /* … */ }
```

with a test that the median tracks a step change within 16 samples and that a single outlier does not move it.

- [ ] **Step 4: Populate `PeerStats` from the live connection**

`rtt_ms` from `Connection::rtt()`. Throughput from `Connection::stats()` if Task 0.1 found a usable figure; otherwise derive it from bytes-over-time on completed transfers. **Do not invent a constant.**

- [ ] **Step 5: Commit**

### Task 2.2: Dispatch integration

**Files:** Modify `crates/vox-orchestrator/src/a2a/dispatch/`

- [ ] **Step 1: Write the failing test**

Dispatch consults `worth_shipping` before selecting a remote node, and falls back to local when no peer clears the margin. Assert local execution happens when every peer is cold — this is the default state on a fresh install and must be correct.

- [ ] **Step 2: Replace the lease gate**

`gate_local_fallback` and the `mesh_exec_leases` table go away. The connection is the lease: `Connection::closed()` is the expiry signal, and the originator retries locally on close without a completion.

```rust
tokio::select! {
    res = run_remote(&conn, job) => res,
    err = conn.closed() => {
        tracing::warn!(%err, "peer dropped mid-job; retrying locally");
        run_local(job).await
    }
}
```

- [ ] **Step 3: Record outcomes**

Every completion feeds `Observations` for both the local and remote path. This is what makes the estimator improve.

- [ ] **Step 4: Commit**

---

## Phase 3 — Deletion

Only after Phases 1–2 work end to end. Each task is its own commit so a revert is surgical.

### Task 3.1: Retire the HTTP control plane

- [ ] **Step 1: Confirm no live callers** of `vox_populi::transport::serve` outside `vox populi serve`.
- [ ] **Step 2: Delete** `crates/vox-populi/src/transport/` (5,483 lines) and `crates/vox-plugin-populi-mesh/src/transport/` (3,702 — already dormant; `MeshDriver::start_transport` has no non-test caller).
- [ ] **Step 3: Delete** `tls.rs` (~150; no caller in `serve()`).
- [ ] **Step 4: Retire** `vox populi serve` and `vox populi up`; the endpoint now lives in `vox-orchestrator-d`. Add retirement markers per `AGENTS.md`.
- [ ] **Step 5:** `cargo run -p vox-cli -- ci command-sync` — removing subcommands drifts the CLI surface SSOT.
- [ ] **Step 6: Commit**

### Task 3.2: Delete confirmed-dead subsystems

Each verified by symbol search to have zero consumers outside its own crate:

- [ ] `pairing/{device_flow,github_attestation}` (422) — and `pairing_e2e.rs`, a one-line placeholder
- [ ] `quota/` (267)
- [ ] `vox-mesh-types`: `quorum`, `tee_attestation`, `secret_sync`, `model_inventory`, `op_fragment`, `kudos` (~600)
- [ ] exec-lease machinery (~1,500)
- [ ] **Keep** `A2ADeliverRequest` (16 consumers) and every capability type
- [ ] **Step: Remove the crate-wide `#![allow(dead_code)]`** from `vox-plugin-populi-mesh/src/lib.rs` and resolve what it was hiding. Prefer deletion.
- [ ] **Step: Commit** each group separately

### Task 3.3: File the SSOT amendment

- [ ] Amend `mesh-and-language-distribution-ssot-2026.md` (line 101 makes GitHub attestation a binding gate) recording that ticket pairing supersedes it for local peers, that P6-T7's invite flow is deliberately replaced, and that the attestation subsystem had no non-test callers. **Do not silently contradict a ratified SSOT.**
- [ ] Add the `vox-mesh` row to `where-things-live.md` and its layer assignment.

---

## Phase 4 — Axis

### Task 4.1: Serve mesh state to the GUI

**Files:** Modify `crates/vox-orchestrator-mcp/src/populi_tools.rs`

- [ ] **Step 1:** `vox_mesh_nodes` returns trusted peers with live status, plus `pending_peers` from `MdnsAddressLookup::subscribe()`, plus `discovery_state` (`disabled` / `failed` / `running`) so a blocked network is distinguishable from an empty one.
- [ ] **Step 2:** Enable the feature in **both** `vox-gui` and `vox-orchestrator-d`. The audit found revision 1 gated its equivalent behind `populi-transport`, which is off in every shipped binary — the field would have been permanently empty. Verify with `cargo tree -e features`.
- [ ] **Step 3:** Add an integration test that the tool returns peers from a live endpoint. Revision 1's unit tests passed under all three of its wiring failures.
- [ ] **Step 4: Commit**

### Task 4.2: Mesh surface

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.tsx` and its test

Real-harness facts confirmed by the audit — do not rediscover them:

- There is **no** `renderMeshView` helper. The suite mocks `@tauri-apps/api/core`'s `invoke` via a module-scope `invokeMock` and must answer **two** tools (`vox_mesh_nodes` and `vox_mesh_queue_stats`). `pushToast` is a required prop.
- The `Toast` type is `{ tone: 'ok'|'warn'|'info'; title: string; body?: string; cause: ToastCause }` — no `kind`, no `message`. Getting this wrong fails `vox ci gui-honesty`, a required job.
- Tauri args are camelCase at the top level. Settled against `SettingsView.tsx:192`.
- The root is a 12-column grid; new sections need `col-span-12` or they render as a 1/12 strip.
- Use design tokens. `docs/agents/gui-honesty-findings/Mesh.json` already flags `statusTone()` at `severity: med` for a hardcoded palette; do not copy that pattern.

- [ ] **Step 1: Write the failing tests** for: a pending peer rendered with its `EndpointId`; Trust invoking with that id; a `discovery_state: 'failed'` message distinct from the empty-LAN message.
- [ ] **Step 2: Ticket UI** — paste box, **this node's own ticket with a copy button**, and Untrust. The ticket is the default pairing path and must be at least as prominent as the pending list.
- [ ] **Step 3: Show the local `EndpointId`** on this surface. Revision 1 sent users to `vox populi identity show`, which prints a *different key* from a *different store* in a different format — they could never match.
- [ ] **Step 4: Keep trusted peers visible** after approval. Filtering them out of `pending_peers` without another home makes an approved peer vanish from the UI.
- [ ] **Step 5:** `pnpm test && pnpm typecheck && cargo run -p vox-cli -- ci gui-honesty`
- [ ] **Step 6: Commit**

### Task 4.3: Surface gains and losses

- [ ] **Step 1:** Per-job row showing where it ran, estimate versus actual, and cumulative time saved or lost. This is Requirement 6's "surfacing performance gains and losses" and the feedback loop that makes the estimator auditable.
- [ ] **Step 2:** `MeshWidget` gains a pending count. Note it takes `{ data }: { data: DashboardData }` and calls **no hook** — adding a count is a real change, not a derivation. Keep the literal label `Mesh Peers` (`Dashboard.test.tsx:355` asserts on it) and replace the third line rather than adding a fourth.
- [ ] **Step 3: Commit**

---

## Phase 5 — Two-node verification

- [ ] **Step 1: Ticket pairing.** B: `vox mesh ticket`. A: paste into Axis. Reciprocate. Expected: mutual trust, no discovery involved.
- [ ] **Step 2: Refusal before trust.** From an untrusted third endpoint, attempt a job. Expected: refused before the executor runs. A success is blocking.
- [ ] **Step 3: Dispatch after trust.** Job runs on B and returns.
- [ ] **Step 4: Cost model.** Run a job with a large payload over a throttled link; assert it stays local. Run a GPU job with a small payload; assert it ships. Compare the estimate against the actual in Axis.
- [ ] **Step 5: Connection-as-lease.** Kill vox on B mid-job. Expected: A retries locally with a clear message, no duplicate execution, no orphaned lease row.
- [ ] **Step 6: mDNS path.** Both on one LAN with no ticket. Expected: mutual discovery within ~60s. If empty, check UDP 5353 — on Windows a **non-admin user cannot accept the firewall prompt**. That is an expected outcome, not a bug: fall back to the ticket and record the network class.
- [ ] **Step 7: No internet.** Disconnect both, keep them on the LAN, repeat Steps 1 and 3. Unchanged behavior. **A failure here is blocking** — it is the requirement distinguishing this design from revision 0.
- [ ] **Step 8: Screenshot** the mesh surface with a real second node. This is the Requirement 6 deliverable; a passing unit test is not.
- [ ] **Step 9: Quickstart** at `docs/src/how-to/populi-two-node-quickstart.md`, frontmatter `category: "How-To Guides"`. Lead with the ticket; present mDNS as the shortcut; state which networks it will not work on.
- [ ] **Step 10:** `vox ci pre-push --complete`

---

## Appendix — verified correct, do not relitigate

From the eight-track audit and this revision's API research:

- **iroh 1.1.0 API** — `Endpoint::{bind, builder, connect, accept, addr, id, close}`; `Builder::alpns(Vec<Vec<u8>>)`; `Connection::{open_bi, accept_bi, closed, remote_id, stats, rtt}`; `presets::{Empty, Minimal, N0, N0DisableRelay}`; `EndpointId` is an ed25519 public key; `remote_id()` derives from the TLS certificate.
- **`MdnsAddressLookup`** — `builder().build(endpoint.id())`, attached via `endpoint.address_lookup()?.add(mdns)`; `subscribe()` yields `DiscoveryEvent`.
- **`iroh_tickets::endpoint::EndpointTicket::new(endpoint.addr())`** — base32 postcard; embeds current addresses.
- **`ring`, `aws-lc-rs`, `quinn`, `rustls` are already in `Cargo.lock`** — iroh's stack is not a new policy exposure, though the `AGENTS.md` `ring` ban is already aspirational.
- **`quic-rpc` is deprecated** in favor of `irpc`.
- **Tauri args are camelCase** at the top level; nested struct fields stay snake_case.
- **The edge ratchet models edges, not features.**
- **Arch-check LoC budgets are `warn`/`off`**; all touched crates are already over budget on `main`.
- **The test-first detector is file-granular**, not per-function.
- **`category: "How-To Guides"`** is in the canonical vocabulary.
- **`A2ADeliverRequest` has 16 consumers** — it looked dead under a module-path grep but is re-exported at the crate root. It stays.
- **No `contracts/` change** is required for new `vox_mesh_nodes` fields; the crate has no MCP output schema.
