# lld-link on Windows — Investigation & Adoption Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. **Investigation-led** — Phase 0 captures the lock holder, which DECIDES Phase 2. Do not pick a fix before Phase 1 classifies the holder. Run in an isolated worktree (Windows, warm `target/`).

**Goal:** Adopt lld-link on `x86_64-pc-windows-msvc` for faster linking by finding + fixing the root cause of the "permission-denied overwriting locked test binary" error, or conclude cleanly with the holder recorded.

**Spec:** `docs/superpowers/specs/2026-06-30-lld-link-windows-design.md`.

**Pre-req:** `C:\Program Files\LLVM\bin\lld-link.exe` present (the spec confirms it). Large crate `vox-cli` already built once (warm) so the experiment relinks, not cold-builds.

---

## Phase 0 — Reproduce & capture the lock holder (keystone)

- [ ] **Step 1: build vox-cli tests once with MSVC (baseline, warm the tree).**
```bash
cargo test -p vox-cli --no-run > /tmp/baseline.out 2>&1; echo "exit $?"
```
- [ ] **Step 2: switch to lld-link and loop until it trips.** RUSTFLAGS env REPLACES config rustflags, so carry the existing link-args:
```bash
LLD='C:/Program Files/LLVM/bin/lld-link.exe'
export RUSTFLAGS="-Clinker=$LLD -Clink-arg=/DEBUG:NONE -Clink-arg=/STACK:8388608"
for i in $(seq 1 8); do
  touch crates/vox-cli/src/main.rs
  cargo test -p vox-cli --no-run > /tmp/lld_$i.out 2>&1
  ec=$?; echo "run $i exit=$ec"
  grep -iE "permission denied|access is denied|ERROR_SHARING|LNK|failed to write|os error 5" /tmp/lld_$i.out | head -2
  [ $ec -ne 0 ] && break
done
```
- [ ] **Step 3: at the failure, identify the failing `.exe` + its holder.** Extract the path from the error, then (Sysinternals `handle.exe` if present, else Restart-Manager via PowerShell):
```bash
FAILEXE="$(grep -oE '[A-Za-z]:[^ ]+\.exe' /tmp/lld_$i.out | head -1)"
echo "locked file: $FAILEXE"
# Sysinternals (preferred):
"handle.exe" -nobanner "$FAILEXE" 2>/dev/null || true
# Fallback — PowerShell Restart Manager (no install):
powershell -NoProfile -Command "
  \$f='$FAILEXE';
  Add-Type -Namespace RM -Name N -MemberDefinition @'
  [DllImport(\"rstrtmgr.dll\")] public static extern int RmStartSession(out uint h,int f,string k);
'@ 2>\$null;
  Get-Process | Where-Object { \$_.Modules.FileName -contains \$f } | Select Name,Id"
# Cheap heuristics that usually answer it outright:
powershell -NoProfile -Command "Get-Process MsMpEng -EA SilentlyContinue | Select Name,Id  # Defender active?"
powershell -NoProfile -Command "Get-Process -EA SilentlyContinue | Where-Object {\$_.Path -like '*target*deps*'} | Select Name,Id  # zombie test exes?"
```
- [ ] **Step 4: RECORD the holder** (process name + PID) and the exact error in the plan/notes. **STOP and classify (Phase 1) before fixing.** If lld-link does NOT trip in 8 runs, the lock may already be gone (orphans cleared this session) → jump to Phase 3 and just adopt + prove.

---

## Phase 1 — Classify (decision, no code)

- [ ] Map the Step-4 holder to the cause:
  - `MsMpEng.exe` → **Defender** (Phase 2A).
  - a `*-<hash>.exe` under `target/**/deps` → **zombie test process** (Phase 2B).
  - no foreign holder / holder is the linker's own prior invocation → **lld no-retry race** (Phase 2C).
- [ ] Write the holder + chosen branch into the plan before proceeding.

---

## Phase 2A — Defender holds the handle

- [ ] **Step 1 (admin): exclude the worktree target dirs.**
```bash
powershell -Command "Start-Process powershell -Verb RunAs -ArgumentList '-Command','Add-MpPreference -ExclusionPath \"C:\\Users\\Owner\\vox\\target\",\"C:\\Users\\Owner\\vox-*\\target\"'"
```
- [ ] **Step 2: re-run Phase 0 Step 2 loop** → expect zero failures. If clean, go to Phase 3.

## Phase 2B — Zombie test process holds its own image

- [ ] **Step 1: confirm the holder is a stale test exe** (alive after its run finished).
- [ ] **Step 2: reap stale test exes before relink** (reuse the orphan-hygiene approach; scoped to the build path):
```bash
powershell -NoProfile -Command "Get-Process -EA SilentlyContinue | Where-Object {\$_.Path -like '*\\target\\*\\deps\\*'} | Stop-Process -Force"
```
- [ ] **Step 3: re-run the loop** → expect clean. Decide whether reaping belongs in a pre-test hook (out of scope here = manual + documented) or the doctor.

## Phase 2C — lld-link doesn't retry a transient sharing violation

- [ ] **Step 1: write a thin retry shim** `crates/vox-lld-shim` (or a script) that execs lld-link and, on `ERROR_SHARING_VIOLATION`/exit-with-that-text, retries ≤3× with 200 ms backoff; propagates all other errors immediately.
- [ ] **Step 2: unit test** the retry decision: sharing-violation → retried; other error → not.
- [ ] **Step 3: point `-Clinker` at the shim, re-run the loop** → expect clean.

---

## Phase 3 — Adopt + prove

- [ ] **Step 1: bench link time (large crate), lld-link vs MSVC.**
```bash
# MSVC baseline:
unset RUSTFLAGS; touch crates/vox-cli/src/main.rs
bash -c 'start=$(date +%s%3N); cargo build -p vox-cli --quiet >/dev/null 2>&1; echo "MSVC relink: $(( $(date +%s%3N)-start ))ms"'
# lld-link:
export RUSTFLAGS="-Clinker=C:/Program Files/LLVM/bin/lld-link.exe -Clink-arg=/DEBUG:NONE -Clink-arg=/STACK:8388608"
touch crates/vox-cli/src/main.rs
bash -c 'start=$(date +%s%3N); cargo build -p vox-cli --quiet >/dev/null 2>&1; echo "lld-link relink: $(( $(date +%s%3N)-start ))ms"'
```
**Gate: lld-link ≥25% faster.** If not faster, stop (MSVC stays; record the bench).
- [ ] **Step 2: stability gate — `cargo test -p vox-cli` 5× consecutively, zero permission-denied.**
```bash
for i in 1 2 3 4 5; do touch crates/vox-cli/src/main.rs; cargo test -p vox-cli --no-run >/tmp/st_$i.out 2>&1; echo "run $i exit=$?"; done
grep -liE "permission denied|access is denied" /tmp/st_*.out && echo "FAILED" || echo "5x STABLE"
```
- [ ] **Step 3: make it permanent** — edit `.cargo/config.toml` `[target.x86_64-pc-windows-msvc]`:
```toml
[target.x86_64-pc-windows-msvc]
linker = "lld-link"
rustflags = ["-C", "link-arg=/DEBUG:NONE", "-C", "link-arg=/STACK:8388608"]
```
and REPLACE the stale "revisit when fixed" comment with the measured holder + fix applied + the bench number.
- [ ] **Step 4: commit + push to main** (admin `--no-verify`; pre-push fmt gate is unrelated).

---

## Phase 4 — Guard (doctor linker check)

- [ ] **Step 1: failing test** in `build_health.rs` — pure classifier:
```rust
#[test]
fn linker_verdict_flags_msvc_fallback_and_missing_av() {
    assert!(linker_ok("lld-link", /*av_excluded=*/true).is_none());
    assert!(linker_ok("link.exe", true).is_some());          // still on slow MSVC
    assert!(linker_ok("lld-link", false).is_some());          // lld but no AV exclusion
}
```
- [ ] **Step 2: implement** `linker_ok(configured: &str, av_excluded: bool) -> Option<String>` + a check that reads the configured linker (from `cargo -Z unstable-options config get`, or parse `.cargo/config.toml`) and, on Windows with Defender active, whether `target/` is excluded (`Get-MpPreference`). Emit `[diag id=linker.msvc_fallback|linker.av_no_exclusion …]`; add ids to `KNOWN_DIAGNOSIS_IDS`.
- [ ] **Step 3: tests green; `vox doctor` shows the linker row; commit.**

---

## Exit criterion (no infinite loop)
If Defender exclusion is policy-blocked OR no bounded retry survives the race OR lld-link isn't ≥25% faster → **stay on MSVC**, and replace the config comment with the measured holder + reason. The deliverable is then the *record*, not the switch.

## Verification
`cargo build -p vox-cli` relinks ≥25% faster under lld-link; `cargo test -p vox-cli` green 5×; `.cargo/config.toml` uses `linker = "lld-link"` with the holder/fix documented; `vox doctor` shows `✓ linker`.
