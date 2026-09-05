# Vox on the MacBook — Clone and Setup Handoff

**Written:** 2026-09-04, from `blaptop04` (Windows) against `bertrands-macbook-pro`.
Every fact below was checked on the machine it describes. Where a version or path appears, it was
read from that machine, not assumed.

---

## 1. The short version

Vox is public, fully pushed, and the Mac already has the toolchain. This is a clone, not a
migration — **nothing needs to be transferred from the Windows box.**

```bash
cd ~/Developer/GitHub
git clone https://github.com/vox-foundation/vox.git
cd vox
git checkout fix-all-ci-failures
rustup target add wasm32-wasip1
cargo build -p vox-cli
```

The rest of this document is why each of those lines is there, and what to do when one fails.

---

## 2. What was verified before writing this

| Check | Result |
|---|---|
| Repo is public (anonymous clone) | **Yes** — `git ls-remote` with credentials disabled succeeded *from the Mac* |
| Remote HEAD matches this machine | `bcc50766a` on both |
| Unpushed commits on Windows | **None** — a clone gets everything |
| Uncommitted work at risk | None: a `.claude/scheduled_tasks.lock` edit and an untracked backup dir |
| Mac has Rust | `rustup`, `cargo`, `rustc 1.98.0`, `sccache`, all present |
| Mac has a linker | Xcode CLT at `/Library/Developer/CommandLineTools` |
| Mac disk free | 759 GB |
| Vox already on the Mac | **No** — nothing under `~` and nothing in `~/Developer/GitHub` |

That last row is the reason this document exists: vox was assumed to be there already, and it is
not.

---

## 3. Where to put it

The Mac keeps its checkouts in `~/Developer/GitHub` — that is where `gigme`, `fableforge` and
~35 other repos live. Clone vox beside them:

```bash
cd ~/Developer/GitHub && git clone https://github.com/vox-foundation/vox.git
```

No credentials required. `gh auth status` on the Mac currently reports
`Failed to log in to github.com account brbrainerd`, which does not matter here — HTTPS anonymous
clone works, and it is what the command above uses. Fix `gh` separately if you want it for PRs.

---

## 4. The branch

The Windows checkout is on **`fix-all-ci-failures`**, not the default branch, and it tracks
`origin/fix-all-ci-failures`. A fresh clone lands on the default branch, so check out explicitly:

```bash
git checkout fix-all-ci-failures
```

Confirm you match this machine:

```bash
git rev-parse --short HEAD    # expect bcc50766a
```

---

## 5. Toolchain — the one real trap

`rust-toolchain.toml` pins:

```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
targets = ["wasm32-wasip1"]
```

The Mac's default is **1.98.0**. That is fine: `rustup` reads the pin and fetches 1.96.0 on the
first cargo command inside the repo, so the first build will pause to download a toolchain before
it compiles anything. That is expected, not a hang.

**The `targets` line is not always honoured automatically** depending on rustup version, and vox
compiles plugins to WASM. If a build fails with a message about `wasm32-wasip1` not being
installed, that is the cause:

```bash
rustup target add wasm32-wasip1
```

Running it up front costs seconds and removes the failure mode entirely, which is why it is in the
quick-start above.

---

## 6. Building

This is a **136-crate workspace**, so build scope matters more than usual.

`default-members` is `crates/vox-cli`, so a bare `cargo build` builds the CLI, not all 136 crates:

```bash
cargo build -p vox-cli          # the binary
cargo test -p <crate>           # what CONTRIBUTING.md asks for on changed crates
cargo install --path crates/vox-cli    # puts `vox` on PATH (README's own instruction)
```

**Expect the first build to be long and disk-hungry.** A cold Rust workspace of this size
routinely produces several GB in `target/`. The Mac has 759 GB free, so this is comfortable there
— worth stating because on 2026-09-03 the Windows machine hit **3.9 GB free** partly from Rust
build artefacts, and a peer session had to clear its `target/` to recover.

`sccache` is already installed on the Mac. It is not wired in by default; if you want it:

```bash
export RUSTC_WRAPPER=sccache
```

Verify the binary works:

```bash
cargo run -p vox-cli -- commands --recommended
```

`vox commands --recommended` is the README's own first-run discovery command.

---

## 7. Credentials

`.env.example` is tracked, so the clone brings it. **There is no real `.env` on the Windows
machine to copy** — only the example — so nothing secret needs to move for vox, unlike gigme and
fableforge.

Copy the template and fill in what you actually need:

```bash
cp .env.example .env
```

The keys it declares:

| Key | Notes |
|---|---|
| `OPENROUTER_API_KEY` | Same provider gigme uses; a working key already exists in `~/Developer/GitHub/gigme/.env` |
| `GEMINI_API_KEY` | |
| `VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_DB_PATH` | Database wiring |
| `VOX_GITHUB_TOKEN` | |
| `PORT` | |

Vox builds and runs without these; they gate specific features. Add them when you hit something
that needs one, rather than up front.

---

## 8. First-run checklist

```bash
cd ~/Developer/GitHub/vox
git rev-parse --short HEAD              # bcc50766a
rustup show active-toolchain            # should say 1.96.0 (from the pin), not 1.98.0
cargo build -p vox-cli                  # long on first run
cargo run -p vox-cli -- commands --recommended
```

If `rustup show active-toolchain` still reports 1.98.0 while inside the repo, the pin is not being
read — check you are actually in the repo root and that `rust-toolchain.toml` survived the clone.

---

## 9. Two loose ends on the Windows copy

Neither blocks the clone; both are noted so they are not mistaken for real files later.

- Two stray files sit at the repo root whose *names* are mangled Windows temp paths
  (`C:UsersiacchAppDataLocal...scratchpaddiff1.txt` and `diff2.txt`). They are artefacts of an
  earlier session that wrote to a path without separators. They are untracked, so they will not
  clone.
- `graphify-out.pre-graphify-backup/` is untracked and local-only.

---

## 10. Related

- `docs/agents/handoff-protocol.md` — the project's own handoff conventions
- `README.md` §Install — upstream build-from-source instructions this document follows
- `CONTRIBUTING.md:54` — the targeted-test expectation for changed crates
- For the sibling project on the same machine: `~/Developer/GitHub/gigme/docs/HANDOFF.md`
