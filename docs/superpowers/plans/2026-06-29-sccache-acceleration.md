# sccache Acceleration — Investigation & Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. This is **investigation-led** — Phase 0 is a measurement that decides Phases 1–3. Do not skip to a fix.

**Goal:** Make sccache deliver real cache hits on same-worktree rebuilds (target **>80%** hit-rate on a `cargo clean -p X` + rebuild of unchanged code), or prove it cannot here and stay disabled — never a cache that's pure cost.

**Spec:** `docs/superpowers/specs/2026-06-29-sccache-acceleration-design.md`.

**Constraint:** sccache is currently disabled (`~/.cargo/config.toml` `rustc-wrapper` commented). All experiments **explicitly** set `RUSTC_WRAPPER=sccache` for the one command — do NOT re-enable globally until Phase 3 proves hits. Run experiments serially (no parallel cargo — target-lock + measurement noise).

---

## Phase 0 — Instrument & measure the miss reason (the keystone)

The 0.4%/0% hit-rate's cause is hypothesis, not fact. Measure it first.

- [ ] **Step 1: clean baseline + stats reset.**
```bash
sccache --stop-server; rm -rf ~/vox/.sccache 2>/dev/null
SCCACHE_DIR="$HOME/.sccache-exp" sccache --start-server
SCCACHE_DIR="$HOME/.sccache-exp" sccache --zero-stats
cargo clean -p vox-secrets
```
- [ ] **Step 2: first build (populates cache) WITH sccache + debug log.**
```bash
SCCACHE_DIR="$HOME/.sccache-exp" SCCACHE_LOG=debug SCCACHE_ERROR_LOG=/tmp/sccache1.log \
  RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 \
  cargo build -p vox-secrets > /tmp/build1.out 2>&1
SCCACHE_DIR="$HOME/.sccache-exp" sccache --show-stats | tee /tmp/stats1.txt
```
- [ ] **Step 3: second build (should HIT) — clean then rebuild identical source.**
```bash
cargo clean -p vox-secrets
SCCACHE_DIR="$HOME/.sccache-exp" SCCACHE_LOG=debug SCCACHE_ERROR_LOG=/tmp/sccache2.log \
  RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 \
  cargo build -p vox-secrets > /tmp/build2.out 2>&1
SCCACHE_DIR="$HOME/.sccache-exp" sccache --show-stats | tee /tmp/stats2.txt
```
- [ ] **Step 4: read the verdict.** If `stats2.txt` shows hits → caching works with explicit `CARGO_INCREMENTAL=0` (Hypothesis 1 confirmed → Phase 1). If still misses, grep `/tmp/sccache2.log` for the **miss reason** sccache logs per compilation (`cache miss`, `not cacheable`, `incremental`, `unhashed`) → identifies Hypothesis 2/3.

Expected outputs decide the branch — **stop and record which hypothesis held before proceeding.**

---

## Phase 1 — If H1 (incremental): make `CARGO_INCREMENTAL=0` reliable

The config sets it in `[env]`, but `cargo check` / some paths may not inherit it.

- [ ] **Step 1: verify inheritance** — `cargo build -p vox-secrets -v 2>&1 | grep -i incremental` (is `-C incremental` passed? if yes, sccache can't cache).
- [ ] **Step 2:** if leaking, confirm the fix is `~/.cargo/config.toml [env] CARGO_INCREMENTAL=0` applies to the actual build invocation (it did in Phase 0 because we set it explicitly); decide whether the broker/wrapper path stripped it.
- [ ] **Step 3:** re-run Phase 0 Step 3 relying ONLY on the config (no explicit env) — confirm hits persist.

---

## Phase 2 — If H2 (path/metadata keying): stabilize the cache key

sccache keys include the rustc invocation + crate metadata; per-worktree absolute `target/` dirs make every key unique.

- [ ] **Step 1:** diff the two debug logs' compile-command hashes for the same crate (`grep "compile" /tmp/sccache1.log`) — do the keys differ between identical builds? If yes, the variable is the path.
- [ ] **Step 2:** test `--remap-path-prefix` to a stable virtual root:
```bash
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/vox" \
  cargo build -p vox-secrets   # then clean + rebuild + check hits
```
- [ ] **Step 3:** if remap fixes same-worktree but cross-worktree still misses, that matches the spec's known limit — document; same-worktree hits are the win we need.

---

## Phase 3 — Prove the win, then re-enable under guard

- [ ] **Step 1: the success bench** — `cargo clean -p vox-secrets && time (RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build -p vox-secrets)`; capture wall-time + `--show-stats` hit-rate. **Gate: hit-rate > 80%** and a clear wall-time collapse vs the cold build.
- [ ] **Step 2: re-enable** — uncomment `rustc-wrapper = "sccache"` in `~/.cargo/config.toml` (with the proven `CARGO_INCREMENTAL=0` + any `--remap-path-prefix`), bound `SCCACHE_CACHE_SIZE`, cache dir outside any repo.
- [ ] **Step 3: guard** — the build-health doctor's `sccache_guard` (`build_health.rs`) already flags crash/0%-hit regression; confirm it passes now (`vox doctor` → `✓ sccache: health`).
- [ ] **Step 4: commit** the config note + a docs update recording the proven recipe.

---

## Exit criterion (no infinite tuning)

If after Phases 1–2 no configuration reaches the >80% gate on this machine's layout, **stop**: keep sccache disabled, and record in `~/.cargo/config.toml` + the spec exactly which hypotheses were tested and why it can't deliver here (per the spec's "Alternatives" — cargo-incremental same-worktree, shared target, or faster linker may beat a 0%-hit sccache outright).

## Verification
A reproducible `cargo clean -p vox-secrets && cargo build -p vox-secrets` with `RUSTC_WRAPPER=sccache` shows hit-rate >80% in `sccache --show-stats`; `vox doctor` reports `✓ sccache: health`.
