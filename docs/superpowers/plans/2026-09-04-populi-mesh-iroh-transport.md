# Populi Mesh on iroh — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vox distributes CPU and GPU work across a user's own machines, sandboxes what it receives, decides per-machine whether shipping is worth it, and shows honestly what that bought.

**Architecture:** iroh provides transport, identity, and NAT traversal. Vox keeps capability scheduling. Work received from a peer runs sandboxed by default. Deletion happens **last**, after the replacement is proven across two machines.

**Tech Stack:** Rust 1.96, `iroh 1.1` (pinned, `noq` QUIC — not quinn), **`iroh-tickets 1.0`**, `iroh-mdns-address-lookup` (pinned `=`, pre-1.0), Tauri 2 + React (Axis). **No `iroh-blobs`, no `irpc`** in v1 — see spec Part 13.

> **`iroh-tickets` must be `1.0`, not `0.1`** — verified on the Mac 2026-09-04.
> `0.1` pulls `iroh-base 0.94`, which pins `ed25519-dalek =3.0.0-pre.1` and
> conflicts irreconcilably with `iroh 1.1`'s `>=3.0.0-rc`. With `1.0` the tree
> resolves clean: `iroh 1.1.0`, `noq 1.2.0`, `iroh-tickets 1.0.0`,
> `ed25519-dalek 3.0.0` (**stable**, not `-rc`), `ring 0.17.14`, and **no
> `aws-lc-rs`** — confirming `tls-ring` holds for an iroh-only tree.

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

## Execution model — parallelism, and where it actively hurts

**BLAPTOP04 is now SSH-reachable over the tailnet** (`ssh iacch@100.107.222.96`),
so every two-machine step is single-seat from the Mac. That changes the shape of
this plan: the spike, the demo, and all cross-platform verification are now
scriptable rather than a walk to the other desk.

### What to parallelise

**Across machines — always.** Builds on the Mac and BLAPTOP04 are independent
and should overlap:

```bash
ssh iacch@100.107.222.96 'cd ~/vox && git pull --ff-only && cargo build -p vox-cli' &
(cd ~/vox && git pull --ff-only && cargo build -p vox-cli)
wait
```

**Subagent-driven development, per task, one agent at a time.** Each task in this
plan is scoped to a disjoint file set precisely so a fresh subagent can own it
with full context and no merge risk. Give the agent the task's own section, its
Interfaces block, and the appendix. Review between tasks, not during.

Good candidates for *concurrent* subagents, because their file sets do not
overlap:

| Group | Tasks | Files |
|---|---|---|
| A | 0.6 (HF seam) | `vox-speech`, `vox-plugin-speech`, `vox-populi/mens` |
| B | 0.5 (lockfile gate) | `vox-cli-ci`, `contracts/crypto` |
| C | 0.3 Steps 2–3 (schema, ADR) | `vox-schema.json`, `docs/src/adr` |

### What NOT to parallelise — learned the hard way

**Never run concurrent subagents that each invoke cargo on the same worktree.**
They serialise on `target/`'s build lock and then deadlock: a real attempt during
this planning session produced 17 cargo/rustc processes, the oldest blocked 50
minutes, with zero forward progress. The analysis phases parallelise; the
compilation phases do not.

Two ways to get real parallelism if you want it:

1. **`git worktree` per agent**, each with its own `target/` (this repo already
   sets per-worktree target dirs in `.cargo/config.toml`). Costs disk and a cold
   cache per worktree.
2. **One agent holds cargo**; the others do analysis, docs, and contract edits
   only. Simpler, and usually enough.

**Phases 1 → 2 are strictly sequential** — Phase 2 is the demo that proves Phase
1. Do not start Phase 3's ports before the demo passes; the whole point of the
ordering is that deletion follows proof.

---

## Phase 0 — Prerequisites and the spike

### Task 0.1: Build vox on the Mac

**Four lines, already documented** in `docs/superpowers/2026-09-04-macbook-clone-handoff.md`, which verified from Windows that the Mac has `rustc 1.98.0`, Xcode CLT, and 759 GB free, and that **vox is not installed there.** Every question in Task 0.2 needs two machines.

- [ ] **Step 1**

```bash
# Kick BLAPTOP04's build off FIRST so the two overlap — SSH is live now.
ssh iacch@100.107.222.96 'cd ~/vox && git pull --ff-only && cargo build -p vox-cli' &

# Then build here, on the Mac (now the development machine).
mkdir -p ~/Developer/GitHub && cd ~/Developer/GitHub
git clone https://github.com/vox-foundation/vox.git && cd vox
git checkout fix-all-ci-failures
rustup target add wasm32-wasip1
cargo build -p vox-cli
wait
```

- [ ] **Step 2:** Record the `host_triple` (`aarch64-apple-darwin`) and build time. This is the first cross-platform build verification in the project.

### Task 0.2: Throwaway spike, run between the two machines

**Output is an answer, not code.** If question 1 or 3 fails, stop and reassess.

**Now single-seat.** Copy the spike to BLAPTOP04 and drive both ends from the Mac:

```bash
scp -r $SCRATCH/iroh-spike iacch@100.107.222.96:C:/Users/iacch/iroh-spike
ssh iacch@100.107.222.96 'cd C:/Users/iacch/iroh-spike && cargo run --release -- serve' &
# read the printed ticket, then dial from the Mac:
cargo run --release -- dial '<ticket>'
```

For question 1 (zero third-party contact), disconnect **both** machines from the
internet first — the tailnet is not needed for a LAN dial, and leaving it up
would not prove the point.

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

### Task 0.3: Apply the authorized decisions

**All four were decided by the operator on 2026-09-04.** No further approval is
needed; these are now execution steps.

- [ ] **Step 1: Crate edges — AUTHORIZED.** Add `["vox-orchestrator-mcp",
  "vox-mesh-transport"]` and `["vox-ml-cli", "vox-mesh-transport"]` to
  `contracts/ci/crate-edges.allow.v1.json`, and assign `vox-mesh-transport`
  layer 2 in `contracts/ci/crate-layers.v1.json`. Both edges are L4→L2,
  downward, layer-legal.

  **Build-time rationale, since that was the operator's stated condition:**
  `rustls` and `ring` are already compiled in this tree, so the marginal cost is
  `iroh` + `noq` + `iroh-tickets` + `iroh-mdns-address-lookup` +
  `swarm-discovery`. Against that, Phase 6 deletes `axum`, `tower-http`,
  `jsonwebtoken`, and `reqwest-middleware` from two crates. **Task 0.2 Step 2
  question 6 measures this on both platforms.** If the delta is worse than ~90 s
  on a clean build, stop and report before Phase 1 — the authorization was
  conditional on it not exploding.

- [ ] **Step 2: `vox-schema.json`.** Register the crate definition **before**
  creating the directory (`docs/agents/governance.md:142`, Error severity).

- [ ] **Step 3: ADR-046.** File `docs/src/adr/046-iroh-transport.md` — supersedes
  ADR-008 and ADR-020, partially supersedes ADR-017 (grant protocol only; the
  duplicate-execution guard survives), and **upholds ADR-018**. ADR-020
  §Decision.1 requires this ADR by name and also pre-authorizes the move by
  naming QUIC as the future option.

- [ ] **Step 4: Crypto provider — `ring`, *provisionally*.** Chosen over
  `aws-lc-rs` because it is already the deliberate pin at `Cargo.toml:251`, it is
  iroh's default (`tls-ring`), and it is the smaller change **while two reqwest
  majors coexist**.

  > **This is an interim state, not the destination.** Task 0.4 Step 5 collapses
  > to a single provider, and that provider is `aws-lc-rs` — because once
  > hf-hub moves to reqwest 0.13, every consumer is on 0.13 and 0.13 selects
  > `aws-lc-rs`. Keeping `ring` past that point would mean carrying a second
  > provider solely to honour this pin. Do not treat this step as settled policy.

  Three sub-steps:
  1. Pin `iroh = { version = "1.1", default-features = false, features = ["tls-ring", ...] }`.
     Never `tls-aws-lc-rs`.
  2. Fix reqwest's feature selection so `workspace-hack` stops enabling
     `rustls/aws-lc-rs` on all four target sections, then `cargo hakari generate`.
     Verify with `cargo tree -i aws-lc-sys` returning nothing. **Worth doing
     whether or not iroh lands** — the workspace ships two providers today.
  3. Amend `AGENTS.md` §Cryptography Policy and `cryptography-ssot-2026.md` per
     spec Part 8: application crypto through `vox-crypto` (source scan, keep),
     transport crypto allowlisted and pinned (**lockfile** gate,
     `vox ci crypto-provider-check`), toolchain invariant verified by a CI image
     without cmake/nasm. Both documents currently ban `ring`, which line 251 uses
     on purpose. Delete `crypto_ban.rs`'s manifest branch and its four tests —
     `scanner.rs:132` means that branch has never executed.

- [ ] **Step 5: `PopuliHttpOp` — port, do not retire.** It is a Vox `activity`
  language surface; retiring it is a language-level breaking change. Resolves
  Task 3.4.

### Task 0.4: Collapse reqwest to one major via hf-hub 1.0

> **MEASURED CORRECTION, 2026-09-04 (Mac, commit `f48dbc810`).** The port is
> **done** — Steps 1–3 landed; all four call sites compile. But two premises
> below were checked and are **false**, so Steps 5–7 do not follow from it:
>
> 1. **`hf-hub` is not the only `reqwest 0.12` consumer, and never was the
>    important one.** The root manifest pins `reqwest = "0.12"` at
>    `Cargo.toml:227`, and **28 first-party `vox-*` crates** depend on it
>    (`vox-http-client`, `vox-actor-runtime`, `vox-llm-egress`, `vox-compiler`,
>    `vox-cli`, …), plus `nanopub`, `tavily`, `reqwest-middleware 0.4` and
>    `reqwest-retry 0.7`. After the hf-hub port, `Cargo.lock` **still shows both
>    `0.12.28` and `0.13.4`**, and both `ring 0.17.14` and `aws-lc-rs 1.17.0`
>    remain. Collapsing reqwest is a **workspace-wide pin bump**, not a
>    consequence of this task — re-scope it as its own task and cost it
>    accordingly (it also drags `reqwest-middleware` and `reqwest-retry` majors).
> 2. **hf-hub 1.0 is an API rewrite, not a renamed `Api`.** There is no
>    `api::sync` / `api::tokio`, no `Repo`, no `RepoType::Model` value, and no
>    `tokio` or `ureq` feature. The surface is `HFClient` / `HFClientSync`
>    (feature **`blocking`**, not `tokio`), `split_id("owner/name")`,
>    `client.model(owner, name)`, and
>    `repo.download_file().filename(f).revision(r).send()`. `info()` became
>    `info().send()` and its `siblings` field is now
>    `Option<Vec<RepoSibling>>` — an absent listing must be distinguished from an
>    empty one or a failed listing reads as "repo has no safetensors".
>
> What the port **did** buy: `hf-hub` no longer pulls `ureq` (the remaining
> `ureq 2.12.1` comes from `sherpa-onnx-sys`, unrelated), and `hf-xet` brings
> chunk-deduplicated model downloads to the MENS path.

**Why this is a task and not a version bump.** The workspace builds two reqwest
majors, and that is the root cause of shipping two TLS providers: 0.12's
`rustls-tls` selects `ring`, 0.13's selects `aws-lc-rs`. Verified requirements:

| Consumer | Requires |
|---|---|
| `hf-hub 0.5.0` | `reqwest ^0.12.2` |
| `chromiumoxide 0.9.1` | `reqwest ^0.13` |
| `gix-transport 0.57.1` (← `gix` ← `jj-lib` ← `vox-vcs`) | `reqwest ^0.13.2` |

`hf-hub` is the only consumer on the 0.12 side, and **hf-hub 1.0.0 (2026-07-10)
moved to `reqwest ^0.13`**. Upgrading collapses reqwest to one major, drops
`native-tls`/`hyper-tls` and `ureq`, and is expected to leave `aws-lc-rs` as the
sole provider — a real build-time and audit-surface win.

**hf-xet is a win, not a cost.** 1.0 adds `hf-xet` (HuggingFace's Xet storage
layer: chunk-level dedup and parallel range fetches) — materially faster model
and checkpoint downloads, which is exactly the path MENS uses. Step 4 still
measures compile time, but do not treat `hf-xet` as a regression to justify.

**It is an API port, not a bump.** hf-hub 1.0 drops `ureq`, and four call sites
use `hf_hub::api::sync::Api`:

- `crates/vox-speech/src/backends/candle_whisper.rs`
- `crates/vox-speech/src/backends/sherpa_model_config.rs` (two sites)
- `crates/vox-plugin-speech/src/backends/candle_whisper.rs`

plus one on `api::tokio::Api` in `crates/vox-populi/src/mens/hub.rs`. 1.0 also
adds `hf-xet`, `bon`, and `hyper` as non-optional dependencies — measure those
before assuming the build gets smaller.

- [ ] **Step 1: Read the 1.0 API** and decide the replacement for `api::sync::Api`
  (likely reqwest blocking, or make the four sites async).
- [ ] **Step 2: Bump and port**, one crate at a time: `vox-speech`,
  `vox-plugin-speech`, `vox-populi`.
- [ ] **Step 3: Verify the collapse.** `cargo tree -d | rg reqwest` shows one
  major; `cargo tree -i native-tls` and `-i ring` are empty.
- [ ] **Step 4: Measure.** Clean-build wall time and `cargo tree | wc -l` before
  and after. If `hf-xet` costs more than the removals save, keep the port but say
  so in the ledger.
- [ ] **Step 5: Collapse crypto to a single provider — the whole point.**

  With hf-hub on 0.13, **every** reqwest consumer is on 0.13 (chromiumoxide and
  `gix-transport` already require it), and 0.13 selects `aws-lc-rs`. So the
  single surviving provider is `aws-lc-rs`, and **VCS is untouched** — `jj-lib`
  and `gix` keep the reqwest major they already wanted. Nothing is given up.

  This **inverts the provider choice made in Task 0.3 Step 4.** `ring` was the
  right answer while two majors coexisted, because it was already pinned. Once
  the split is gone, holding `ring` would mean carrying a second provider *purely
  to honour the old pin* — the opposite of consolidation. Concretely:

  1. Flip the root pins: `rustls` and `tokio-rustls` from `features = ["ring"]`
     to `["aws-lc-rs"]`.
  2. Flip the mesh crate: `iroh` from `tls-ring` to `tls-aws-lc-rs`.
     **Both features exist; iroh does not require its default.** If Phase 1 has
     already landed on `tls-ring`, this is a one-line change there.
  3. `cargo hakari generate`, then verify: `cargo tree -i ring` **empty**,
     `cargo tree -i aws-lc-sys` non-empty and sole, `cargo tree -d | rg reqwest`
     shows one major.
  4. Confirm the build-toolchain invariant still holds — `aws-lc-sys` >= 0.41
     needs neither cmake nor nasm on Windows (verified empirically on BLAPTOP04
     2026-09-04), but re-verify on macOS and Linux before declaring it.

  **Abort condition:** if removing `ring` forces `jj-lib`, `gix`, or
  `chromiumoxide` onto a different major, or breaks the toolchain invariant on
  any platform, stop and keep both providers. Consolidation is not worth losing
  VCS or a clean-clone build.

- [ ] **Step 6: Update the ledger** — `contracts/crypto/transport-providers.v1.json`:
  `duplicates[reqwest]` resolved, `ring` removed from `allowed_providers`,
  `aws-lc-rs` restated as the sole provider with its new attribution. **Ledger
  edits are user-authorized; propose the diff, do not write it.**

- [ ] **Step 7: Tighten the policy once it is true.** `AGENTS.md` §Cryptography
  Policy currently says two providers ship deliberately. After the collapse it
  should say **one**, and `vox ci crypto-provider-check` (Task 0.5) should fail on
  a second appearing. A gate that permits two forever is not consolidation.

### Task 0.5: `vox ci crypto-provider-check` — the lockfile gate

The source-text detector now runs (its scanner bug is fixed), but it **cannot see
feature-mediated provider selection** — `rustls = { features = ["aws-lc-rs"] }`
names no banned crate. The sound input is `Cargo.lock`.

- [ ] **Step 1:** New gate in `crates/vox-cli-ci/`, ~120 lines, no cargo
  invocation: parse `Cargo.lock`, intersect package names with the known provider
  set, compare against `contracts/crypto/transport-providers.v1.json`.

  > **MEASURED CORRECTION, 2026-09-04.** A pure `Cargo.lock` name-intersection
  > produces **false positives**. `ring` is present in a lockfile resolved for
  > `hf-hub 1.0` alone, yet `cargo tree -i ring` reports *nothing to print* for
  > the host target — it is a target-gated entry that is never compiled, while
  > `aws-lc-rs` is the provider actually built. `Cargo.lock` records what was
  > *resolved*, not what is *built*. The gate must therefore resolve for the
  > target set (via `cargo metadata` and the resolve graph, or an equivalent
  > feature/target-aware walk) and report built providers — or it will fail CI on
  > a provider that ships no code.
- [ ] **Step 2:** Fail on an unexpected provider, a vanished allowlisted one, a
  version drift, or an unwaived duplicate-major.
- [ ] **Step 3:** Wire into `vox ci pre-push` (fast tier) and CI `ssot-drift`.
- [ ] **Step 4:** Add a `no-native-toolchain` CI lane on an image without cmake,
  nasm, Go, perl, or libclang. The build-toolchain invariant is a property of the
  build; only a build without those tools can verify it.

### Task 0.6: Sequester HuggingFace behind one seam

HF is already feature-gated (`vox-populi/mens-hf-hub`, `vox-speech/stt-sherpa`,
`vox-plugin-speech`), but the download logic is duplicated across three crates
and each names `hf_hub::` types directly — so Task 0.4's port touches all three.

- [ ] **Step 1:** One `hf_download(repo, revision, file) -> PathBuf` seam, owned
  by whichever crate the layer map allows, with the other two calling it.
- [ ] **Step 2:** `hf_hub::` appears in exactly one file afterward. Verify:
  `rg 'hf_hub::' crates/ | wc -l` == the seam's own count.
- [ ] **Step 3:** Do this **before** Task 0.4 — it turns a three-crate port into
  a one-file port, and makes the next upstream break equally cheap.

---

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

> **PARTIALLY PROVEN, 2026-09-04 (commit `007b43c48`).** Steps 4 and 5 are done
> ahead of Steps 1–3, using `vox-mesh-transport`'s `mesh_smoke` example instead
> of the CLI verbs — the transport path underneath is identical, so the evidence
> stands regardless of how the arguments are parsed. macOS (`aarch64`) dialled
> BLAPTOP04 (`x86_64`) over the LAN with **no relay and no discovery service**:
>
> - **Step 5 (refusal before trust):** `closed by peer: not trusted (code 4001)`,
>   connect 12.5 ms. The executor was never reached.
> - **Step 4 (pair and Probe):** after trusting the Mac's `EndpointId` on
>   BLAPTOP04 — **with the listener still running**, since the allowlist is
>   re-read on every check — connect 10.5 ms and
>   `Probed { host_triple: "x86_64-windows", vox: "0.6.0" }`. The listener logged
>   `executing Probe for 62333c19…8086 at Wasm`: **pairing granted a sandbox,
>   not native execution.**
>
> Still open: Steps 1–3 (the `vox mesh join` verbs and `ci command-sync`).
>
> **Step 6 is now largely done, and the earlier claim that it "needs the router's
> uplink physically pulled — it cannot be driven over SSH" was wrong.** It was
> asserted without being tested. The whole of what follows was driven over SSH
> from the Mac; see the findings doc §Offline for the method, the calibration and
> the one trap.
>
> Measured 2026-09-05 with a per-program Windows Firewall outbound rule on
> BLAPTOP04 (`iacch`'s SSH session is elevated, so `New-NetFirewallRule` works
> remotely):
>
> - **Instrument calibrated first.** Same rule shape on `curl.exe`:
>   internet `000`, `1.1.1.1` `000`, tailnet `100.107.222.96` `000`,
>   LAN `192.168.50.1` `200`.
> - **Negative control:** BLAPTOP04 dialling with *all* outbound blocked →
>   `Error: timed out`. The test can fail.
> - **Result:** BLAPTOP04 restricted to `192.168.50.0/24` only — no internet, no
>   tailnet — dialled the Mac and completed a `Probe` in **22.1 ms**:
>   `Probed { host_triple: "aarch64-macos", vox: "0.6.0" }`.
> - **Listener side:** with *all* outbound blocked, `serve` still bound and
>   printed a valid ticket, so it needs no outbound initiation to become
>   operational.
> - **Socket census on the Mac listener:** `UDP *:58535`, `UDP *:64100`,
>   `UDP *:5353` — three sockets, **zero remote addresses**. No relay, no DNS.
>
> **The trap, and why the first attempt was worthless.** Windows Firewall is
> **stateful**: an outbound block rule does not apply to the listener's replies on
> an already-established inbound flow. The first run blocked the *listener* and
> the dial succeeded anyway — which would have read as a pass. Only the negative
> control exposed it. **Block the initiator, never the responder**, and always
> calibrate the rule on a program you can independently observe before trusting
> a reading from the program under test.
>
> **Residual gap, honestly:** both machines simultaneously offline at the OS
> level. Not closed, and neither remaining option needs a physical act:
> (a) block the two hosts at the router — it is an ASUS at `192.168.50.1` with a
> reachable admin UI, so a per-client rule costs nothing and leaves the rest of
> the household online, which pulling the uplink would not; or (b) one `sudo pf`
> rule on the Mac. Given the layered evidence above, this is confirmation rather
> than discovery.

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
- [ ] **Task 3.4: `PopuliHttpOp` — port it** (decided in Task 0.3 Step 5). A Vox `activity` language surface; retiring it would be a language-level breaking change. Do not delete it as collateral.

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
