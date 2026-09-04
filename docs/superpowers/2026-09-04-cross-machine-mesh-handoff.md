# Cross-Machine Handoff — Version Parity and the Mesh's Real State

**Written:** 2026-09-04 from `BLAPTOP04` (Windows), as the last session on that
machine. Every fact was checked on the machine it describes.

**Read this first:** the mesh **does not work today**, and not because of
configuration. `vox populi up` has never started a server on any machine — it
spawns `populi serve` without `--enable`, the child bails on its first statement,
and both pipes are `Stdio::null()`, so it fails silently while the parent prints
"Populi started" and a pid. There is nothing to "get working" by fixing settings.
§4 is what you can actually verify; §5 is the work that would make the mesh real.

---

## 1. The blocker for driving both machines from the Mac

**BLAPTOP04 has no SSH server.** Verified: `Get-Service sshd` → *NOT INSTALLED*;
zero listeners on port 22. Tailscale SSH is **client-only on Windows** — the
`tailscale status --json` capability map advertises SSH, but there is no server
to accept it.

So from the Mac you can reach the Mac, and you cannot reach BLAPTOP04.

Two ways forward. Pick one before starting §3.

**Option A — enable OpenSSH Server on BLAPTOP04 (recommended, one time).**
Run this *at* BLAPTOP04, in an **Administrator** PowerShell:

```powershell
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd
# Tailnet-only: no public exposure.
New-NetFirewallRule -Name sshd-tailscale -DisplayName "OpenSSH (Tailscale only)" `
  -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 `
  -RemoteAddress 100.64.0.0/10
```

Then from the Mac: `ssh iacch@100.107.222.96` (BLAPTOP04's tailnet address).
Everything in §3–§4 becomes a single-seat operation.

This is a system settings change and requires elevation, which is why it was
left for you rather than done automatically.

**Option B — two seats.** Run the Mac half from the Mac, walk to BLAPTOP04 for
its half. Every block below is labelled with which machine it runs on.

---

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

Three things are verifiable now. The fourth is the honest negative result.

**4.1 Both machines build the same commit.** §3.1.

**4.2 The tailnet path works** (this is not vox, but it is the network the mesh
will use). From the Mac:

```bash
tailscale ping blaptop04
```

**4.3 Capability probing works on both.** This exercises the GPU/CPU truth layer
that survives the rewrite and is the part vox actually contributes:

```bash
cargo run -q -p vox-cli --features populi -- populi registry-snapshot
```

Note `--features populi` — **`vox populi` is not compiled into default builds.**
`vox-ml-cli`'s default is `["mens-base"]` and the release builder passes no
features, so a released `vox` has no `populi` subcommand at all.

**4.4 The mesh does not connect, and here is the proof.** Run on either machine:

```bash
cargo run -q -p vox-cli --features populi -- populi up --mode lan
curl http://127.0.0.1:9847/health          # connection refused
cargo run -q -p vox-cli --features populi -- populi down
```

`up` prints "Populi started" with a pid and writes `mesh-state.json`. The health
check fails because the child died immediately. Root cause:
`crates/vox-ml-cli/src/commands/populi_lifecycle.rs:178-190` spawns
`populi serve --bind <addr>` with **no `--enable`**, and
`crates/vox-ml-cli/src/commands/populi_cli.rs:850` bails without it.

**Do not spend time debugging this.** It is a known one-argument bug and it is
Task 0.1 of the implementation plan.

---

## 5. Everything still to be done

Full detail: [`plans/2026-09-04-populi-mesh-iroh-transport.md`](plans/2026-09-04-populi-mesh-iroh-transport.md).
Design: [`specs/2026-09-04-populi-mesh-iroh-transport-design.md`](specs/2026-09-04-populi-mesh-iroh-transport-design.md).

### 5.1 Standalone fixes — real bugs in `main`, independent of the mesh rewrite

Each is small, each is live today, and none needs the iroh work to land first.

| # | Fix | Location |
|---|---|---|
| F1 | `vox populi up` never passes `--enable`, so the daemon has never started | `populi_lifecycle.rs:178-190` |
| F2 | Unauthenticated `FullAccess` when no token is configured — gates `worker/execute`, which writes posted bytes to temp, `chmod 0755`, and runs them, with policy defaulting to `"permissive"` and **no timeout**. Contained today only by the loopback bind. | `router.rs:124`, `auth.rs:148`, `dispatch.rs:230,263` |
| F3 | `/v1/populi/bootstrap/exchange` swaps its used-flag **before** comparing the token — one bad POST permanently burns the window | `handlers/nodes.rs` |
| F4 | `lookup_by_pubkey_hex` never reads disk, so on the production file-backed registry it always returns `None` — and it is the only function the wire-path verifier consults | `vox-identity/src/trust.rs:110` |
| F5 | Effect inference propagates **one hop**; `@pure` admits `net` at two. `xs.map(named_fn)` is unchecked entirely. `placement.rs` inherits the same blind spot. | `typeck/effect_check.rs:413` |
| F6 | The crypto detector's manifest branch has **never executed** — `scanner.rs:132` drops every file whose language is `Unknown`, and `.toml` is `Unknown`. Its tests pass by bypassing the scanner. | `vox-code-audit/src/scanner.rs:132` |
| F7 | The workspace ships **two** TLS providers: `ring` (pinned on purpose at `Cargo.toml:251`) and `aws-lc-rs` (via `workspace-hack`, from reqwest). The SSOT bans `ring`, so the root manifest contradicts it. | `Cargo.toml:251`, `workspace-hack/Cargo.toml:373,467` |
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

## 6. If you only do one thing

Run §3 and §3.1. Two machines on one commit, both building, is the prerequisite
for every phase and it is the thing that is genuinely blocked on you being at
the Mac.

Then §4.4, once, to see the current failure with your own eyes — it is the
clearest possible statement of why this is a rewrite rather than a repair.
