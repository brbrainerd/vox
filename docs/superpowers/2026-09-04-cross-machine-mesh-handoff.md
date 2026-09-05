# Cross-Machine Handoff — Version Parity and the Mesh's Real State

**Written:** 2026-09-04 from `BLAPTOP04` (Windows), as the last session on that
machine. Every fact was checked on the machine it describes.

**Read this first.** As of `60d51c520`, `vox populi up` starts a real control
plane for the first time — verified on BLAPTOP04: `curl /health` returns **HTTP
200**. Getting there took five separate pre-existing fixes, because `vox populi`
did not even compile (see §5.1).

**That is a single-machine milestone, not a working mesh.** The control plane
binds loopback and there is still no cross-machine transport. §4 is what you can
verify today; §5 is the work that makes it a mesh.

> **SUPERSEDED IN PART, 2026-09-04 (Mac).** Phase 1 has landed and the mesh now
> works between the two machines. Measured on `007b43c48` with
> `vox-mesh-transport`'s `mesh_smoke` example, macOS (`aarch64`) dialling
> BLAPTOP04 (`x86_64`), no relay and no discovery service:
>
> ```text
> # untrusted, before pairing
> connected in 12.517917ms to 93a10036…6693
> Error: connection lost
>   Caused by: closed by peer: not trusted (code 4001)
>
> # after `mesh_smoke trust <mac-endpoint-id>` on BLAPTOP04 — no restart,
> # the allowlist is re-read on every check
> connected in 10.450042ms to 93a10036…6693
> response: Probed { host_triple: "x86_64-windows", vox: "0.6.0" }
> # listener side:  executing Probe for 62333c19…8086 at Wasm
> ```
>
> So: a peer is refused before pairing, admitted after it, and runs
> **sandboxed** — pairing granted `Wasm`, not native. That is Phase 2 Steps 4
> and 5 proven with the real crate rather than a spike. Still outstanding for
> the full Phase 2 sign-off: the `vox mesh join` verbs (Steps 1–3) and the
> **both-machines-offline** repeat (Step 6), which is blocking and needs the
> router's uplink physically pulled.

---

## 1. Driving BLAPTOP04 from the Mac — enabled 2026-09-04

**OpenSSH Server is installed, running, and tailnet-scoped.** Verified on the
machine:

| | |
|---|---|
| Service | `sshd: Running / Automatic` |
| Listening | `0.0.0.0:22` and `[::]:22` |
| Banner | `SSH-2.0-OpenSSH_for_Windows_9.5` |
| Firewall | `sshd-tailscale` allows **only** `100.64.0.0/10` (the tailnet CGNAT range) |
| Windows' broad rule | `OpenSSH-Server-In-TCP` **disabled** — port 22 is not reachable from the LAN or the internet |

From the Mac:

```bash
ssh iacch@100.107.222.96          # or: ssh iacch@blaptop04.tail4f69a0.ts.net
```

Password authentication works today (Windows OpenSSH defaults to it).

### Key-based auth — one gotcha that catches everyone

**`iacch` is a member of Administrators**, so Windows OpenSSH does **not** read
`~/.ssh/authorized_keys` for this account. It reads
`C:\ProgramData\ssh\administrators_authorized_keys`, and that file must be
owned by Administrators/SYSTEM with inheritance disabled or sshd silently
ignores it. Both facts are undocumented in most guides and produce a
"permission denied (publickey)" with nothing in the logs.

From the **Mac**, print your public key:

```bash
cat ~/.ssh/id_ed25519.pub    # or generate: ssh-keygen -t ed25519
```

Then on **BLAPTOP04**, in an **Administrator** PowerShell, paste it in and fix
the ACL in the same step:

```powershell
$key = '<paste the ed25519 line here>'
$f = "$env:ProgramData\ssh\administrators_authorized_keys"
Add-Content -Path $f -Value $key
icacls $f /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F'
Restart-Service sshd
```

### Why not Tailscale SSH

Tailscale's SSH **server** does not run on Windows — v1.60+ disables it
deliberately over privilege-delegation and session-isolation constraints
(tailscale/tailscale#14942, #17261). Windows can be a Tailscale SSH *client*
only, which is why OpenSSH Server is the path. The Tailscale GUI
(`tailscale-ipn`) and the `Tailscale` service run together with no conflict —
both were live while this was verified.

## 2. State BLAPTOP04 was left in

| | |
|---|---|
| Branch | `fix-all-ci-failures`, tracking `origin/fix-all-ci-failures` |
| Working tree | Clean except `graphify-out.pre-graphify-backup/` (untracked, pre-existing, safe to delete) |
| Running vox processes | None. No `vox.exe`, no `vox-orchestrator-d.exe`. |
| Stale mesh state | `.vox/populi/mesh.env` **deleted** — see the security note below |
| Tailnet | `blaptop04` = `100.107.222.96`, online, tailnet `tail4f69a0.ts.net` |
| Toolchain | Rust 1.96 (`rust-toolchain.toml`); no `cmake`, no `nasm` on PATH — and nothing needs them |

### Security note, acted on

`.vox/populi/mesh.env` was **tracked in git** and contained `VOX_MESH_TOKEN`. It
entered history in `044b68cb9`, which is on `origin/main` and many other branches
of a **public** repository.

Untracked and gitignored in this session; the local file is deleted. **History
still contains it, so treat the token as compromised.** Practical impact is low —
it authorizes a loopback-bound control plane that has never started — and the
value is generated rather than configured, so deleting the file *is* the
rotation. No action needed from you unless you want the history rewritten.

---

## 3. Get both machines on the same commit

**Run on: the Mac.** Vox is not installed there yet — verified from Windows:
nothing under `~`, nothing in `~/Developer/GitHub`. The Mac does have
`rustc 1.98.0`, `sccache`, Xcode CLT, and 759 GB free.

```bash
mkdir -p ~/Developer/GitHub && cd ~/Developer/GitHub
git clone https://github.com/vox-foundation/vox.git
cd vox
git checkout fix-all-ci-failures
git log --oneline -1        # must match §3.1 below
rustup target add wasm32-wasip1
cargo build -p vox-cli      # first build is long; sccache is present
```

**Run on: BLAPTOP04.**

```bash
cd ~/vox && git fetch origin && git checkout fix-all-ci-failures && git pull --ff-only
git log --oneline -1
cargo build -p vox-cli
```

### 3.1 Parity check — the actual ask

Run on **both**, compare the two outputs character for character:

```bash
git rev-parse HEAD && cargo run -q -p vox-cli -- --version
```

They must print the same commit SHA. The version string will differ in its
`+build.N (GITHASH)` suffix — that is `vox-build-meta` injecting per-machine
build metadata and is expected. **The SHA is the parity signal; the version
string is not.**

If the SHAs differ, the machine that is behind has not pulled. There is no
merge to resolve — this branch is linear.

---

## 4. What you can actually confirm today

Four things are verifiable now. None of them is "the mesh works" — that comes in Phase 2.

**4.1 Both machines build the same commit.** §3.1.

**4.2 The tailnet path works** (this is not vox, but it is the network the mesh
will use). From the Mac:

```bash
tailscale ping blaptop04
```

**4.3 Capability probing works on both.** This exercises the GPU/CPU truth layer
that survives the rewrite and is the part vox actually contributes:

```bash
cargo run -q -p vox-ml-cli --features populi -- populi registry-snapshot
```

**Note the crate: `-p vox-ml-cli`, not `-p vox-cli`.** `vox populi` is
intercepted in `vox-cli`'s `main.rs` and delegated to the separate `vox-ml-cli`
binary, and the `populi` feature lives on *that* crate — `cargo run -p vox-cli
--features populi` fails with "the package 'vox-cli' does not contain this
feature". `vox-ml-cli`'s default is `["mens-base"]` and the release builder
passes no features, so a released `vox` has no `populi` subcommand at all.

**4.4 The daemon now starts — on loopback only.** Run on either machine:

```bash
cargo run -q -p vox-ml-cli --features populi -- populi up --mode lan
curl http://127.0.0.1:9847/health          # expect 200

# NOTE: use the BUILT BINARY for `down`, not `cargo run`. While the daemon is
# up it holds a lock on target/debug/vox-ml-cli.exe, so cargo cannot relink and
# fails with "Access is denied (os error 5)" -- which looks like a permissions
# bug and is a file lock. The binary itself works fine.
./target/debug/vox-ml-cli.exe populi down
```

Until 2026-09-04 this returned connection-refused on every machine: `up` spawned
`populi serve` **without `--enable`**, so the child bailed on its first statement
while the parent printed a pid and wrote `mesh-state.json`. Both pipes went to
`Stdio::null()`, so the one diagnostic that would have explained it was
discarded. Fixed, with the child's stderr now captured to
`.vox/populi/serve.log`.

**Read the scope of this correctly.** It proves the daemon runs and the control
plane answers on loopback. It is **not** a mesh check — there is still no
cross-machine transport, and the control plane binds loopback, which is the only
thing currently containing the unauthenticated-execute path in F2. Do not widen
that bind by hand; Phase 1 replaces the plane entirely and does it safely.

---

## 5. Everything still to be done

Full detail: [`plans/2026-09-04-populi-mesh-iroh-transport.md`](plans/2026-09-04-populi-mesh-iroh-transport.md).
Design: [`specs/2026-09-04-populi-mesh-iroh-transport-design.md`](specs/2026-09-04-populi-mesh-iroh-transport-design.md).

### 5.1 Standalone fixes — real bugs in `main`, independent of the mesh rewrite

**Six are now done** (F1, F3, F4, F5, F6, F7 — struck through below), and two new ones (F10, F11) were found while verifying. `workspace-hack` is
**not** among the problems: 116 crates depend on it, CI gates it
(`cargo hakari generate --diff`), and it exists to unify features so dependencies
are not rebuilt. It faithfully mirrored the duplication; it did not cause it.
Removing it would increase build times.

Each is small, each is live today, and none needs the iroh work to land first.

| # | Fix | Location |
|---|---|---|
| ~~F1~~ | **FIXED 2026-09-04** (`60d51c520`). Passes `--enable`; child stderr now goes to `.vox/populi/serve.log`. **Verified end-to-end on BLAPTOP04: `populi up` → `curl /health` → HTTP 200**, the first time the control plane has ever served. | `populi_lifecycle.rs` |
| ~~F3~~ | **FIXED** (`60d51c520`). Bootstrap now verifies the token before consuming the one-shot window, in **both** handler copies, with a regression test. | `handlers/nodes.rs` ×2 |
| ~~F4~~ | **FIXED** (`60d51c520`). `lookup_by_pubkey_hex` loads from disk, returns owned, and refuses an empty key. | `vox-identity/src/trust.rs` |
| ~~F5~~ | **FIXED** (`60d51c520`). Effect inference is now a fixpoint over the call graph; `map(named_fn)` is governed; `placement.rs` blind spot closed. 4 new tests. | `typeck/effect_check.rs`, `placement.rs` |
| ~~F10~~ | **NOT A PRODUCT BUG — corrected.** `vox populi down` works. The `Access is denied (os error 5)` is **cargo** failing to relink `vox-ml-cli.exe` while the running daemon holds a lock on that same file. Run the built binary directly and it succeeds: `./target/debug/vox-ml-cli.exe populi down` → `SUCCESS: process ... terminated`. **Never wrap `down` in `cargo run` while the daemon is up.** Separately hardened `terminate_process_tree` to resolve `taskkill.exe` from `%SystemRoot%` instead of `PATH` — defensive only (sanitized PATHs in services/CI omit System32); it was not the cause here. | `vox-cli-core/.../process_supervision.rs` |
| **F11** | **NEW.** `WorkerDonationPolicy` had no `Default`, so adding fields broke struct literals silently — that is what stopped `vox populi` compiling. Derive added and the call site spreads it; watch for the same pattern on other shared structs. | fixed in `60d51c520` |
| F2 | Unauthenticated `FullAccess` when no token is configured — gates `worker/execute`, which writes posted bytes to temp, `chmod 0755`, and runs them, with policy defaulting to `"permissive"` and **no timeout**. Contained today only by the loopback bind. | `router.rs:124`, `auth.rs:148`, `dispatch.rs:230,263` |
| F3 | `/v1/populi/bootstrap/exchange` swaps its used-flag **before** comparing the token — one bad POST permanently burns the window | `handlers/nodes.rs` |
| F4 | `lookup_by_pubkey_hex` never reads disk, so on the production file-backed registry it always returns `None` — and it is the only function the wire-path verifier consults | `vox-identity/src/trust.rs:110` |
| F5 | Effect inference propagates **one hop**; `@pure` admits `net` at two. `xs.map(named_fn)` is unchecked entirely. `placement.rs` inherits the same blind spot. | `typeck/effect_check.rs:413` |
| ~~F6~~ | **FIXED 2026-09-04** (`63b2ea303`). The scanner now takes `Cargo.toml`/`Cargo.lock` by filename, with two regression tests that go *through* `scan()`. Verified: the gate runs against the workspace and reports nothing, so it does not break CI. | `vox-code-audit/src/scanner.rs` |
| F7 | **INVESTIGATED + LEDGERED 2026-09-04** (`63b2ea303`). Two providers are **unavoidable while preserving features**: two reqwest majors, and 0.13 (required by `chromiumoxide` and `gix`→`jj-lib`→`vox-vcs`) selects `aws-lc-rs`. `jj-lib` exposes no TLS feature. Recorded with attribution in `contracts/crypto/transport-providers.v1.json`; the SSOT no longer bans the provider the manifest chooses. **Collapse is Task 0.4** (hf-hub 1.0 → reqwest 0.13). | `contracts/crypto/transport-providers.v1.json` |
| F8 | `donation_policy`'s `max_job_duration_secs` and `max_concurrent` have **no readers anywhere** — decorative | `vox-mesh-types/src/donation_policy.rs` |

**F5 is the one worth doing first if you want a win before the mesh work.** It
pays for itself with no distribution story at all: `@pure`, `@place`, and
workflow determinism are all currently unsound past one hop. The fixpoint
algorithm already exists in the adjacent pass (`typeck/placement.rs::infer`) —
copy it, don't invent one. Add the two regression tests (a two-hop effect chain,
and `map(named_fn)`); both fail today.

### 5.2 The mesh work — six phases

Decisions are made; nothing is waiting on approval.

- **Phase 0** — Build on the Mac (§3). Run the iroh spike **between both
  machines**; six questions, two of them load-bearing (does `presets::Minimal`
  contact nothing, and do the byte counters support a placement estimate).
  Apply the authorized decisions: crate edges, `vox-schema.json`, ADR-046,
  and `ring` as the single crypto provider.
- **Phase 1** — `vox-mesh-transport`: identity, trust allowlist, protocol, with
  sandboxing and handshake bounds built in rather than added later.
- **Phase 2** — **The demo.** Ticket-pair Windows ↔ Mac, run a `Probe`. This is
  where "online and interop" is actually proven; everything after is quality.
- **Phase 3** — Port the four capabilities the HTTP plane still serves: A2A
  mailbox, federation directory (feeds the model selector), queue stats (Axis
  calls it today), `PopuliHttpOp` (a Vox `activity` language surface).
- **Phase 4** — Placement: telemetry first, model second.
- **Phase 5** — Axis.
- **Phase 6** — Deletion, last. ~7,000 lines.

### 5.3 Settled, so nobody relitigates them

- **Tailscale is not a dependency** and is not needed. The design contacts no
  third party.
- **`iroh-blobs` and `irpc` are cut from v1.** A bi-stream plus postcard is four
  lines; blobs earns its place for model/checkpoint distribution, not 10 MB
  payloads on a 3 ms LAN.
- **Crypto provider is `ring`.** Already pinned, iroh's default, smaller change.
- **Automatic distribution of arbitrary vox scripts is not attainable**, and the
  corpus has no surface for it anyway: 0 combinator calls in `scripts/` against
  143 `for` loops, 2 files declaring `@pure`, 74 of 84 touching I/O by design.
  Spec Part 7 has the evidence.

---

## 5.4 The two-machine runbook

Three phases. A is today, B is the Mac's work, C is what BLAPTOP04 does once the
Mac has pushed. Every block says which machine it runs on.

### Phase A — parity (today, both machines)

**Mac** — vox is not installed there yet:

```bash
mkdir -p ~/Developer/GitHub && cd ~/Developer/GitHub
git clone https://github.com/vox-foundation/vox.git && cd vox
git checkout fix-all-ci-failures
rustup target add wasm32-wasip1
cargo build -p vox-cli
```

**BLAPTOP04**:

```bash
cd ~/vox && git fetch origin && git pull --ff-only && cargo build -p vox-cli
```

**Both** — this is the parity signal:

```bash
git rev-parse HEAD          # SHAs must match exactly
```

The `--version` strings will differ in their `+build.N (GITHASH)` suffix. That is
`vox-build-meta` stamping per-machine metadata and is expected. **Compare SHAs,
not version strings.**

### Phase B — implementation (Mac)

Order matters; each unblocks the next.

| Step | Task | Why first |
|---|---|---|
| B1 | Plan Task 0.2 — the iroh spike, run **between both machines** | Two assumptions are load-bearing and unverified: does `presets::Minimal` contact nothing, and do iroh's byte counters support a placement estimate. Everything downstream assumes both. |
| B2 | Task 0.6 — sequester HF behind one seam | Turns B3 from a three-crate port into a one-file port. |
| B3 | Task 0.4 — hf-hub 1.0 (reqwest 0.13) | Collapses the reqwest split, which is what makes single-provider possible. Measure build time before/after. |
| B4 | Task 0.5 — `vox ci crypto-provider-check` | The lockfile gate. Land it *after* B3 so it locks in the collapsed state. |
| B5 | Task 0.3 — crate edges, `vox-schema.json`, ADR-046 | Contract/registration work; no code depends on it landing earlier. |
| B6 | Phases 1–2 — `vox-mesh-transport`, then the two-machine demo | Phase 2 is where "online and interop" is actually proven. |

Push after each step. BLAPTOP04 pulls; it does not develop.

### Phase C — enable populi on BLAPTOP04 (after the Mac pushes)

**Today, and until the iroh work lands**, `vox populi` is not in a default build —
`vox-ml-cli`'s default is `["mens-base"]` and the release builder passes no
features. So "enabling populi" means building with the feature:

```bash
cd ~/vox && git pull --ff-only
cargo build -p vox-ml-cli --features populi
cargo run -q -p vox-ml-cli --features populi -- populi up --mode lan
curl http://127.0.0.1:9847/health          # expect success once F1 has landed
cargo run -q -p vox-ml-cli --features populi -- populi down
```

Before the `--enable` fix this returned connection-refused; after it, the
loopback control plane actually starts. **That is a single-machine check, not a
mesh check** — it proves the daemon runs, nothing more.

**After Phase 2 lands**, the check becomes the real one:

```bash
# Mac
cargo run -q -p vox-cli -- mesh join            # prints this node's ticket

# BLAPTOP04 — paste the Mac's ticket
cargo run -q -p vox-cli -- mesh join <vox-mesh://...>
cargo run -q -p vox-cli -- mesh join            # reciprocate; paste back on the Mac

# Either machine
cargo run -q -p vox-cli -- populi dispatch ./probe.vox --node <peer-id>
```

Success is a job dispatched on one machine executing on the other and returning.
Then repeat it with **both machines disconnected from the internet** — that is
the requirement separating this design from the rejected ones, and a failure
there is blocking.

### Single-seat operation

§1 is done: BLAPTOP04 accepts SSH over the tailnet, so **every step above can be
driven from the Mac**. A two-machine check is now one terminal:

```bash
# from the Mac — build both sides in parallel, then compare
ssh iacch@100.107.222.96 'cd ~/vox && git pull --ff-only && cargo build -p vox-cli' &
(cd ~/Developer/GitHub/vox && git pull --ff-only && cargo build -p vox-cli)
wait
ssh iacch@100.107.222.96 'cd ~/vox && git rev-parse HEAD'
git -C ~/Developer/GitHub/vox rev-parse HEAD      # must match
```

Two Windows-specific notes for anything you run over that link:

- Use the **built binary** for `populi down`, not `cargo run` — the daemon locks
  `target\debug\vox-ml-cli.exe` and cargo's relink fails with a misleading
  "Access is denied". See F10.
- `--features populi` goes on **`-p vox-ml-cli`**, never `-p vox-cli`.

---

## 6. If you only do one thing

Run §3 and §3.1. Two machines on one commit, both building, is the prerequisite
for every phase and it is the thing that is genuinely blocked on you being at
the Mac.

Then §4.4, once, to see the current failure with your own eyes — it is the
clearest possible statement of why this is a rewrite rather than a repair.
