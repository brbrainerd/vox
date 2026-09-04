# Populi Mesh on iroh — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vox distributes CPU and GPU work across a user's own machines, sandboxes what it receives, decides per-machine whether shipping is worth it, and shows honestly what that bought.

**Architecture:** iroh provides transport, identity, and NAT traversal. Vox keeps capability scheduling. Work received from a peer runs sandboxed by default. Deletion happens **last**, after the replacement is proven across two machines.

**Tech Stack:** Rust 1.96, `iroh 1.1` (pinned, `noq` QUIC — not quinn), `iroh-tickets`, `iroh-mdns-address-lookup` (pinned `=`, pre-1.0), Tauri 2 + React (Axis). **No `iroh-blobs`, no `irpc`** in v1 — see spec Part 13.

**Spec:** [`2026-09-04-populi-mesh-iroh-transport-design.md`](../specs/2026-09-04-populi-mesh-iroh-transport-design.md) — revision 4.

**Provenance:** Revision 4. Revision 3 was audited across eight tracks; it had six compile errors, three cost-model fields with no data source, an unsandboxed executor, four live capabilities listed as deleted, and a deletion inventory wrong by ~5,000 lines. Findings verified *correct* are in the appendix.

## Global Constraints

- **Deletion is Phase 6 and happens after the two-machine demo passes.** Revision 3's Global Constraints said deletion follows the replacement; its phase numbering did not obey that. This one does.
- **Four capabilities must be ported before anything is deleted:** A2A mailbox, federation directory, queue stats, `PopuliHttpOp`. Spec Part 2.
- **Never `presets::N0`, never `N0DisableRelay`, never `into_0rtt()`.** One detector, three patterns (Task 1.2).
- **Mesh-received work is sandboxed by default.** Pairing does not grant native execution. No global permissive flag.
- **Trust checked at accept *and* per request**, against `~/.vox/mesh_trust.json` keyed by `EndpointId` — **not** `trusted_nodes.json`, which is keyed by `node_id` from a different keyspace.
- **Three processes.** Axis → TCP `127.0.0.1:9745` → `vox-orchestrator-d` (endpoint lives here) → `vox populi serve` (retired in Phase 6).
- **Test-first**, file-granular detector. **Formatting:** `vox run scripts/fmt.vox`, never `cargo fmt --all`.
- **Pre-push:** `vox ci pre-push --complete`.

---

## Phase 0 — Prerequisites and the spike

### Task 0.1: Build vox on the Mac

**Four lines, already documented** in `docs/superpowers/2026-09-04-macbook-clone-handoff.md`, which verified from Windows that the Mac has `rustc 1.98.0`, Xcode CLT, and 759 GB free, and that **vox is not installed there.** Every question in Task 0.2 needs two machines.

- [ ] **Step 1**

```bash
ssh bertrands-macbook-pro 'cd ~/Developer/GitHub && \
  git clone https://github.com/vox-foundation/vox.git && cd vox && \
  git checkout fix-all-ci-failures && rustup target add wasm32-wasip1 && \
  cargo build -p vox-cli'
```

- [ ] **Step 2:** Record the `host_triple` (`aarch64-apple-darwin`) and build time. This is the first cross-platform build verification in the project.

### Task 0.2: Throwaway spike, run between the two machines

**Output is an answer, not code.** If question 1 or 3 fails, stop and reassess.

- [ ] **Step 1: Scratch binary** in `$SCRATCH/iroh-spike/`

Corrected against iroh 1.1.0 — revision 3's version had four errors:

```rust
// throwaway spike — not for merge
use iroh::{Endpoint, endpoint::{presets, PathId}};
use iroh_tickets::endpoint::EndpointTicket;

const ALPN: &[u8] = b"vox/spike/0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("serve") => {
            // `Endpoint::bind(preset)` takes NO alpns — a bind()-created server
            // refuses every connection at ALPN negotiation. Silent, not a compile error.
            let ep = Endpoint::builder(presets::Minimal)
                .alpns(vec![ALPN.to_vec()])
                .bind()
                .await?;
            println!("ticket: {}", EndpointTicket::new(ep.addr()));
            while let Some(incoming) = ep.accept().await {
                let conn = incoming.await?;
                println!("peer: {}", conn.remote_id());   // infallible after handshake
                let (mut send, mut recv) = conn.accept_bi().await?;
                let msg = recv.read_to_end(64 * 1024).await?;
                send.write_all(&msg).await?;
                send.finish()?;
            }
        }
        Some("dial") => {
            let ep = Endpoint::builder(presets::Minimal).bind().await?;
            let ticket: EndpointTicket = std::env::args().nth(2).unwrap().parse()?;
            let conn = ep.connect(ticket.endpoint_addr().clone(), ALPN).await?;
            let (mut send, mut recv) = conn.open_bi().await?;
            send.write_all(b"ping").await?;
            send.finish()?;
            println!("echo: {:?}", recv.read_to_end(1024).await?);
            println!("rtt: {:?}", conn.rtt(PathId::ZERO));
            println!("stats: {:?}", conn.stats());
        }
        _ => anyhow::bail!("serve | dial <ticket>"),
    }
    Ok(())
}
```

- [ ] **Step 2: Answer six questions, and write them down**

1. Does `presets::Minimal` connect with **zero** third-party contact? Verify with both machines' internet disconnected. *(Expect gateway UPnP/PCP traffic from `portmapper_config` — LAN-local, not n0. Do not read that as a failure.)*
2. Does LAN connection succeed with no relay?
3. **What does `conn.stats()` actually expose?** Confirmed from docs: byte and loss counters only, no throughput. Verify, and measure whether differentiating `udp_tx.bytes` over a transfer yields a stable enough figure to drive placement. **The cost model depends on this.**
4. Does `MdnsAddressLookup` find the peer, and does it survive the Windows firewall prompt?
5. Does `ep.online().await` hang under `Minimal`? Its contract is "has contacted a relay server," and there is no relay. **Never put it on the `Minimal` path.**
6. Real dependency-tree weight and build-time delta, on both platforms.

- [ ] **Step 3: Report** to `docs/src/architecture/iroh-spike-findings-2026.md` with frontmatter.
- [ ] **Step 4: Delete the spike.**

### Task 0.3: Four authorizations — STOP and wait

Do not write any of these yourself.

- [ ] **Step 1: Crate edges.** New crate `vox-mesh-transport` (L2), consumed by `vox-orchestrator-mcp` and `vox-ml-cli` (L4). Both are new edges needing an `exceptions` entry. *(Named `-transport`, not `vox-mesh`: `vox-mesh-types` and `vox-mesh-policy` already exist and a bare `vox-mesh` would read as the umbrella.)*
- [ ] **Step 2: `vox-schema.json`.** `docs/agents/governance.md:142` — new crate definitions must be registered **before the file is created**. Error severity; blocks the first commit.
- [ ] **Step 3: The ADR.** ADR-020 §Decision.1 requires a future ADR to replace ADR-008 as the default transport. Draft "ADR-046: iroh QUIC replaces the HTTP populi control plane" — supersedes 008, 020, and 017-partial; **upholds 018**.
- [ ] **Step 4: The crypto-provider decision** (spec Part 8): standardize on `ring` or `aws-lc-rs`. Both currently ship. Independently of iroh, fix the reqwest feature selection and replace the dead textual detector — `scanner.rs:132` means it has never run.

---

## Phase 1 — The transport crate

### Task 1.1: Identity

**Files:** Create `crates/vox-mesh-transport/{Cargo.toml,src/lib.rs,src/identity.rs}`

- [ ] **Step 1: Write the failing test**

```rust
//! Persisted mesh identity. The `EndpointId` IS the ed25519 public key.
//!
//! Deliberately separate from `vox_identity`'s node identity: that one is
//! password-sealed and may be locked, which must never prevent the mesh from
//! starting headless. The two keys cannot share a Rust type anyway —
//! vox-crypto pins ed25519-dalek 2.x, iroh pins 3.0-rc.

pub fn load_or_create(path: &Path) -> anyhow::Result<SecretKey> { /* … */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_key_is_stable_across_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        assert_eq!(id_of(&load_or_create(&p).unwrap()), id_of(&load_or_create(&p).unwrap()),
            "identity must survive restart or every pairing breaks");
    }

    #[test]
    fn a_corrupt_key_file_is_an_error_not_a_silent_new_identity() {
        // Silently regenerating would orphan every peer that trusted the old key.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        std::fs::write(&p, b"not a key").unwrap();
        assert!(load_or_create(&p).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn the_key_file_is_not_readable_by_group_or_other() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        load_or_create(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "a plaintext private key must not be group/world readable");
    }

    #[test]
    #[cfg(unix)]
    fn a_world_readable_key_is_refused_with_the_fix_in_the_message() {
        // Loading silently would teach the user nothing.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        load_or_create(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        let e = load_or_create(&p).unwrap_err().to_string();
        assert!(e.contains("chmod 600"), "error must name the fix: {e}");
    }
}
```

- [ ] **Step 2: Run to verify it fails, implement, verify PASS**
- [ ] **Step 3: Windows protection.** `fs::write` leaves default ACLs. Use DPAPI (`CryptProtectData`, user scope) — at-rest encryption with **no password prompt**, so headless start is preserved. macOS Keychain equivalently. Linux: `0o600` is the honest answer.
- [ ] **Step 4: `vox mesh rotate`.** The key's blast radius is "peers that trust it"; easy rotation is what makes the residual risk acceptable. Prints the new ticket and lists peers that must re-trust.
- [ ] **Step 5: Commit**

### Task 1.2: Trust, and the three-pattern detector

**Files:** Create `src/trust.rs`; add a `vox-code-audit` detector

- [ ] **Step 1: Write the failing test**

```rust
/// Allowlist of trusted `EndpointId`s in `~/.vox/mesh_trust.json`.
///
/// NOT `trusted_nodes.json`: that is keyed by `node_id` from a different
/// keyspace and its `pubkey_hex` can be empty. Overloading it would put two
/// identifier spaces in one file and make `untrust` ambiguous.
pub struct MeshTrust { /* store + live connections */ }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel { Sandboxed, Native }

#[cfg(test)]
mod tests {
    #[test] fn an_untrusted_endpoint_is_refused() { /* … */ }
    #[test] fn trust_persists_across_handles() { /* … */ }

    #[test]
    fn pairing_grants_sandboxed_not_native() {
        let (_d, t) = temp_trust();
        t.trust(&id(), None).unwrap();
        assert_eq!(t.level(&id()), Some(TrustLevel::Sandboxed),
            "pairing must never imply native execution");
    }

    #[test]
    fn a_registry_read_error_fails_closed() {
        let t = MeshTrust::at(Path::new("/nonexistent/dir"));
        assert!(!t.is_trusted(&id()));
    }

    #[test]
    fn writes_are_atomic_so_a_crash_cannot_truncate_the_allowlist() {
        // fs::write truncates then writes; a crash mid-write disables the whole
        // mesh with a parse error nobody connects to the reboot.
        let (_d, t) = temp_trust();
        t.trust(&id(), None).unwrap();
        assert!(!t.path().with_extension("tmp").exists());
        assert!(serde_json::from_str::<Vec<TrustedEndpoint>>(&read(t.path())).is_ok());
    }
}
```

- [ ] **Step 2: Run to verify fail, implement, verify PASS.** Atomic = temp + rename. A read failure logs at `error!` with the path.
- [ ] **Step 3: The detector — three patterns, one gate**

Fails at Error severity on `presets::N0`, `N0DisableRelay`, and `into_0rtt` anywhere in workspace code, with a test asserting each fires. The first two contact n0 infrastructure (Requirement 2); the third makes `remote_id()` fallible, at which point a `?` added to satisfy the compiler silently turns every trust check advisory.

- [ ] **Step 4: Commit**

### Task 1.3: Protocol, with the security properties built in

**Files:** Create `src/protocol.rs`, `src/endpoint.rs`

- [ ] **Step 1: Write the failing tests**

```rust
pub const ALPN: &[u8] = b"vox/job/1";
pub const PROTO: u16 = 1;

/// First frame on every stream. Layout frozen forever; every other message may
/// change. Without this, version skew is a hang rather than a sentence.
#[derive(Serialize, Deserialize)]
pub struct Hello { pub proto: u16, pub vox: String }

pub enum JobRequest {
    Probe,
    /// `payload_bytes` is the sender's *claim*, checked before the transfer and
    /// enforced during it. Payload rides the stream — no blobs in v1.
    Run { kind: TaskKind, payload_bytes: u64 },
    Cancel { job_id: JobId },
}

/// The tier a *received* job runs at. Never taken from the request: the sender
/// proposes a kind, the receiver decides the sandbox.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Isolation { Wasm, Container, Native }
impl Isolation { pub const DEFAULT_FOR_MESH: Self = Self::Wasm; }

pub struct JobLimits {
    pub wall_clock: Duration,      // 300s, hard kill
    pub max_output_bytes: usize,   // 10 MiB, carried from dispatch.rs:388
    pub max_payload_bytes: u64,    // 1 GiB
    pub isolation: Isolation,
}

#[cfg(test)]
mod tests {
    #[test] fn requests_round_trip_through_postcard() { /* … */ }

    #[test]
    fn a_version_mismatch_names_both_versions() {
        let r = check_hello(&Hello { proto: 2, vox: "0.8.1".into() });
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("0.8.1") && msg.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn the_default_isolation_is_a_sandbox() {
        assert_eq!(Isolation::DEFAULT_FOR_MESH, Isolation::Wasm);
    }
}
```

**Note:** `kind` is the **existing** `vox_mesh_types::TaskKind` (`TextInfer`, `ImageGen`, `SpeechTranscribe`, `TrainQLoRA`, `Embed`, `VoxScript`). Revision 3 invented a second `JobKind` — the split-brain the deletion inventory exists to end. Add `Compile` to `TaskKind` if compile jobs ship.

- [ ] **Step 2: Run to verify fail, implement, verify PASS**

- [ ] **Step 3: The accept loop — bounded, gated, and 0-RTT-free**

```rust
const MAX_INFLIGHT_HANDSHAKES: usize = 64;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_THRESHOLD: usize = 32;
const MAX_CONCURRENT_JOBS_PER_PEER: usize = 4;

pub async fn serve(ep: Endpoint, trust: Arc<MeshTrust>, exec: Arc<dyn JobExecutor>) {
    let gate = Arc::new(Semaphore::new(MAX_INFLIGHT_HANDSHAKES));
    while let Some(incoming) = ep.accept().await {
        // A TLS handshake is ~100us of unauthenticated work and iroh has no
        // connection cap. Make spoofed sources prove reachability first.
        if gate.available_permits() < MAX_INFLIGHT_HANDSHAKES - RETRY_THRESHOLD
            && !incoming.remote_addr_validated()
        {
            let _ = incoming.retry();
            continue;
        }
        let Ok(permit) = Arc::clone(&gate).try_acquire_owned() else {
            incoming.ignore();          // cheaper than refuse(): sends nothing
            continue;
        };
        let (trust, exec) = (Arc::clone(&trust), Arc::clone(&exec));
        tokio::spawn(async move {
            let _permit = permit;
            // Awaiting `incoming` completes the handshake, so `remote_id()` is the
            // peer's proven public key. Never call `Accepting::into_0rtt()`: in
            // that state `remote_id()` is fallible and every check below is advisory.
            let Ok(Ok(conn)) = timeout(HANDSHAKE_TIMEOUT, incoming).await else { return };
            let remote = conn.remote_id();          // infallible
            if !trust.is_trusted(&remote) {
                // No protocol-level refusal to a stranger — it would be an oracle.
                conn.close(REFUSED_UNTRUSTED, b"not trusted");
                return;
            }
            trust.register(remote, conn.clone());   // so untrust() can close it
            handle(conn, remote, trust, exec).await;
        });
    }
}
```

- [ ] **Step 4: Endpoint construction**

```rust
pub async fn bind(sk: SecretKey) -> anyhow::Result<Endpoint> {
    let ep = Endpoint::builder(presets::Minimal)
        .secret_key(sk)
        .alpns(vec![protocol::ALPN.to_vec()])
        .bind()
        .await?;
    let mdns = MdnsAddressLookup::builder().build(ep.id())?;
    ep.address_lookup()?.add(mdns);     // Result, not Option
    Ok(ep)
}
```

- [ ] **Step 5: The four security integration tests**

```rust
#[tokio::test] async fn an_untrusted_peer_cannot_reach_the_executor() { /* invocations == 0 */ }

#[tokio::test]
async fn a_trusted_peer_gets_a_sandbox_by_default() {
    trust.trust(&client_id(), None).unwrap();     // ordinary pairing
    run_job(&client, TaskKind::VoxScript, payload).await;
    assert_eq!(last_limits().isolation, Isolation::Wasm);
    assert!(last_limits().wall_clock <= Duration::from_secs(300));
}

#[tokio::test]
async fn untrust_closes_a_live_connection() {
    // iroh has no Endpoint-level "close everything to this peer", so MeshTrust
    // holds the handles. Without this, revocation is a file write and nothing else.
    trust.untrust(&client_id()).unwrap();
    assert!(timeout(Duration::from_secs(2), client_conn.closed()).await.is_ok());
}

#[tokio::test]
async fn a_payload_larger_than_the_cap_is_refused_before_any_transfer() { /* … */ }
```

- [ ] **Step 6: Commit**

---

## Phase 2 — The demo

**This is where goal 1 — "online and interop" — is proven.** Revision 3 put it after 12,000 lines of deletion.

### Task 2.1: Two-machine ticket pairing

- [ ] **Step 1: `vox mesh join`** — one verb, both directions. With no argument: print this node's ticket and listen. With a ticket: connect and trust. The daemon is started if not running; the user never learns the word `vox-orchestrator-d`.
- [ ] **Step 2: Consuming a ticket requires confirmation.** Print the decoded `EndpointId` and require a yes (`--yes` for non-interactive). This is the highest-privilege action in the system and revision 3's spec implied the opposite.
- [ ] **Step 3: Register the verbs**, then `cargo run -p vox-cli -- ci command-sync`.
- [ ] **Step 4: Run it Windows ↔ Mac.** Pair, `Probe`, confirm capabilities come back including `aarch64-apple-darwin` and free VRAM.
- [ ] **Step 5: Refusal before trust** — from an untrusted third endpoint, confirm the executor is never reached.
- [ ] **Step 6: No internet** — disconnect both, keep them on the LAN, repeat. **Blocking if it fails.**
- [ ] **Step 7: Commit and record.** Goal 1 is now demonstrated; everything after is quality.

---

## Phase 3 — Port the four capabilities

Nothing in Phase 6 may run until these land. Spec Part 2.

- [ ] **Task 3.1: A2A mailbox.** Store-and-forward to an offline peer. `remote_worker.rs` (1,325 lines) is the consumer. A request/response RPC is not a substitute — this needs a durable inbox with ack, on its own ALPN.
- [ ] **Task 3.2: Peer directory → model selector.** Replace `federation_directory()` in `registry.rs:379`, `catalog.rs:535`, `task_submit.rs:700` with an enumeration of trusted, probed peers. **Test: a trusted probed peer appears as a `ProviderType::PopuliMesh` candidate; a dropped peer disappears.** That is goal 4's only real acceptance criterion, and deletion without it is silent.
- [ ] **Task 3.3: Queue stats.** Axis calls `vox_mesh_queue_stats` today.
- [ ] **Task 3.4: `PopuliHttpOp`.** Decide: port the Vox `activity` surface, or retire it as a **language-level breaking change** with an ADR. Do not delete it as collateral.

---

## Phase 4 — Placement: data before model

### Task 4.1: Record decisions

- [ ] **Step 1:** `PlacementRecord { job_id, kind, placement, reason, est_local_ms, est_remote_ms, actual_ms, transit_ms, payload_bytes, result_bytes, outcome }` — **one row per decision**, including decisions to stay local. Persisted beside `mesh.key` so a restart does not re-enter cold start.
- [ ] **Step 2: Route on admission plus one honest rule:** ship if the peer has a GPU, this machine does not, and the payload is small. Record what the medians *would* have predicted **without acting on them**.
- [ ] **Step 3: Hard admission is separate from cost.** VRAM (free, not total) and `host_triple` are gates, not preferences. A cross-arch artifact cannot run at any speed.
- [ ] **Step 4: Commit**

### Task 4.2: Switch on the model, if the data earns it

- [ ] **Step 1:** `worth_shipping` as in spec Part 6 — every input `Option`, absent measurement means local. `result_bytes` observed alongside duration, not passed in.
- [ ] **Step 2: Throughput** derived by differentiating `udp_tx.bytes` over a transfer; `rtt_ms` from `rtt(PathId::ZERO)`. Both `Option`.
- [ ] **Step 3: Floor each peer's estimate** relative to the best honestly-observed time — a peer reports its own speed, so the median is attacker-influenced.
- [ ] **Step 4: Bootstrap** — the first job of a kind ships when the peer has a GPU we lack and the local estimate says being wrong is cheap. Otherwise history never accumulates. Tag exploratory dispatches and exclude them from calibration.
- [ ] **Step 5: Show the recorded data moved.** If it didn't, delete the model.
- [ ] **Step 6: Commit**

---

## Phase 5 — Axis

Real-harness facts confirmed by audit — do not rediscover: there is **no** `renderMeshView` helper; the suite mocks `@tauri-apps/api/core`'s `invoke` and must answer **two** tools; `pushToast` is required; `Toast` is `{ tone, title, body?, cause }` (no `kind`/`message` — `vox ci gui-honesty` exists to catch that); args are camelCase; new sections need `col-span-12`; use design tokens (`statusTone()` is already flagged `severity: med`).

- [ ] **Task 5.1:** `vox_mesh_nodes` gains pending peers and `discovery_state`. Enable the feature in **both** `vox-gui` and `vox-orchestrator-d` — revision 1's equivalent was gated behind `populi-transport`, off in every shipped binary. Verify with `cargo tree -e features`.
- [ ] **Task 5.2: MeshView** — ticket paste box (decoded `EndpointId` shown **before** Trust is live), this node's ticket with a copy button labelled *"contains this machine's network addresses,"* Untrust, the local `EndpointId`, and trusted peers still visible after approval.
- [ ] **Task 5.3: `vox mesh explain`** — why a job did or didn't ship, per peer, with the numbers. **CLI, not GUI-only.** An automatic decision the user cannot inspect is a support nightmare.
- [ ] **Task 5.4:** Estimate vs actual, cumulative saving **beside median calibration error**. `MeshWidget` takes `{ data }: { data: DashboardData }` and calls no hook — adding a count is a real change. Keep the literal label `Mesh Peers`.

---

## Phase 6 — Deletion, last

Only after Phases 1–5. Each task its own commit.

- [ ] **Task 6.1: HTTP plane.** `vox-populi/src/transport/` (5,483) **plus `http_client.rs` (649), `http_auth.rs` (112), `http_lifecycle.rs` (198)** — they are `use crate::transport::…` and cannot survive. Plus `vox-plugin-populi-mesh/src/transport/` (3,702, dormant; **keep the crate** — it is catalogued) and `tls.rs` (119). Remove `start_transport` from `extension-points.v1.yaml` in the same commit.
- [ ] **Task 6.2: Confirmed dead only.** `pairing/{device_flow,github_attestation}` (422), `quota/` (267), and in `vox-mesh-types` **only `quorum` and `model_inventory` (37 lines)**. **Do not delete** `secret_sync` (live in `vox secrets`), `kudos` (vox-gamify, vox-db), `op_fragment` (hopper mesh adapter), or `PublicAttestationManifest` (six call sites in `vox-ml-cli`). Remove the `weak-test-baseline.v1.json` entry with the test.
- [ ] **Task 6.3: Leases — grant protocol only.** Delete routes, client methods, and the renew loop. **Keep** the `mesh_exec_leases` table, `vox-db/src/mesh_exec_leases.rs`, and `lease_gate.rs` — the ADR-017 duplicate-execution guard is a DB check, transport-independent, and `known-tables.txt:112` pins the table to a drift fixture.
- [ ] **Task 6.4: Retire the commands.** `vox populi serve` (**two** call sites) and `up`. Update the container sidecar (`infra/containers/entrypoints/vox-entrypoint.vox`, the compose block) and the CI job that invokes it. Delete `openapi_paths.rs` with the router it asserts parity against.
- [ ] **Task 6.5: Env-var sunset.** 36 → ~5. Delete the dead reads, update `env-vars.md`, and add a `vox doctor` warning for retired vars — a user with an old `mesh.env` sourced in their shell gets ignored config and no explanation. **Do not add a `Vox.toml [mesh]` block**: with five survivors it is YAGNI, and it would be project-scoped and gitable, which is wrong for a per-machine identity.
- [ ] **Task 6.6: Docs.** File ADR-046. Rewrite `docs/src/reference/populi.md` (389 lines) and the quickstart; delete two overlay how-tos; status-banner the phase 4/5/6 plans and the config-baseline spec. **~6 files touched, not 21 rewritten.**
- [ ] **Task 6.7: Measure.** `git diff --stat` against the plan's claims. Revision 3 asserted ~12,000/~1,000 with no gate.

---

## Appendix — verified correct, do not relitigate

**iroh 1.1.0:** `Endpoint::{builder, bind, connect, accept, addr, id, close}`; `Builder::{alpns, secret_key, bind}`; `Connection::{open_bi, accept_bi, closed, remote_id, stats, rtt, close}`; `Incoming::{retry, remote_addr_validated, ignore}`; `presets::{Empty, Minimal, N0, N0DisableRelay}`. `remote_id()` is **infallible** on `HandshakeCompleted` and returns `Result` only in 0-RTT states. `Minimal` = `empty()` + crypto provider, verified from source: no relay, no DNS, no pkarr. `iroh_tickets::endpoint::EndpointTicket::{new, endpoint_addr}` (**not** `addr()`). `MdnsAddressLookup::builder().build(id)`; `address_lookup()` returns **`Result`**. iroh uses **`noq`**, not quinn. `tls-ring` is iroh's default feature.

**Codebase:** `TrustedNodeRegistry` is keyed by `node_id`, `pubkey_hex` can be empty. `TaskKind` exists with six variants. `A2ADeliverRequest` has 16 consumers via a root re-export. `max_job_duration_secs` and `max_concurrent` have **no readers** — decorative. Exec policy defaults to `"permissive"`. `vox populi up` never passed `--enable`, so it has never started a server — which is why there is nothing to migrate. `aws-lc-sys` builds on Windows with neither cmake nor nasm on PATH. `crypto_ban.rs`'s manifest branch is unreachable via `scanner.rs:132`.

**Policy:** the edge ratchet models edges, not features. Arch-check LoC budgets are `warn`/`off`. The test-first detector is file-granular. `category: "How-To Guides"` is canonical. Tauri args are camelCase. No `contracts/` change is needed for new `vox_mesh_nodes` fields.

**Research (spec Part 7):** no system achieves decorator-free distribution — Unison, Cloud Haskell, Erlang, Chapel, and X10 all require explicit placement, and the last two rejected hiding it on purpose. The enabling condition is **re-executability**, not purity and not element cost. Cilk's `cilk_spawn` is permission-not-command at 2–6× a function call; a network hop is 3–5 orders of magnitude more expensive, so the same design needs prediction where Cilk needs none.
