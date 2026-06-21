---
title: "Skill-Discovery Engine — Follow-up Fixes + Branch Isolation"
description: "Antigravity/Gemini-3.5-Flash-executable TDD plan applying the 7 code-review follow-ups to the vox-similarity / vox-skill-discovery engine (minhash hot-path perf, ignore-aware walking, signature-length invariant, representative cluster score, real dedup score, test temp-dir hygiene, single-linkage doc) AND isolating the work onto a clean branch + verifying the unplanned vox-runtime L1→L2 arch change. First deliverable from the AGH-0001 code review."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Skill-Discovery Engine — Follow-up Fixes + Branch Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Apply the 7 code-review follow-ups to the discovery/dedup engine and land them on a clean, isolated branch with the `vox-runtime` arch change verified.

**Architecture:** The engine is two crates — `vox-similarity` (L2 pure) + `vox-skill-discovery` (L3). Fixes are localized: signature math (`signature.rs`), file walking (`code_miner.rs`), index invariants (`index.rs`), and dedup scoring (`catalog.rs`). Task 1 first isolates the work onto a fresh branch off current `origin/main` so it's mergeable in isolation.

**Tech Stack:** Rust; `blake3`, `serde` (vox-similarity); `walkdir`/`ignore`, `tempfile`, `vox-plugin-types`, `vox-mcp-registry` (vox-skill-discovery).

**Execution target:** Gemini 3.5 Flash in Antigravity. Basis: `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.

## Operating rules (apply to EVERY task — includes AGH-0001 hardenings)
1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** Run each task's `rg`/read step; never invent an API.
3. **Self-contained** — full code inline; don't rely on recalling earlier tasks.
4. **Two-strike circuit breaker** — fail twice → STOP + handoff note.
5. **Tag `[PARALLEL-SAFE]`/`[SEQUENTIAL]`** by file-write disjointness; never two subagents on one file.
6. **🔒 AGH-0001 §B-2 — NO unplanned shared-config edits.** Do NOT edit `docs/src/architecture/layers.toml`, add `orphan_exempt`, or relabel any crate's layer EXCEPT exactly as Task 2 directs. If `cargo run -p vox-arch-check` is red for a reason unrelated to your task, STOP and report.
7. **🔒 AGH-0001 §B-3/4 — branch isolation + delivery manifest.** All work lands on the clean branch from Task 1. In your final handoff, list EVERY file you changed (including any shared config).
8. **Vox house rules:** no `cargo fmt --all` (use `cargo fmt -p <crate>`); no-stub check via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed>` (there is no `vox stub-check`); `docs/src/` `.md` needs frontmatter.

## Per-task verification ritual (before each commit)
`cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → stub `rg` → `cargo fmt -p <crate>`, pasting real output.

## Pre-flight (run once)
- [ ] `git fetch origin main && git log --oneline -1 origin/main` — note the current tip.
- [ ] **Baseline arch-check is green on `origin/main`.** `git switch -c tmp-baseline-check origin/main && cargo run -p vox-arch-check`. Expected: exits 0. **If it is RED**, the pre-existing baseline (vox-runtime layer, orphan crates, where-things-live, docstrings) is broken on `main` independent of this engine — STOP and report which crates fail; cherry-picking the engine will NOT fix them, and Task 1's green-gate would falsely block. Delete the temp branch afterward (`git switch - && git branch -D tmp-baseline-check`).
- [ ] `rg -n '^ignore |^tempfile |^walkdir ' Cargo.toml` — confirm `ignore = "0.4.25"`, `tempfile = "3"`, `walkdir = "2"` are workspace deps.
- [ ] `rg -n 'pub fn minhash|pub fn jaccard_estimate|fn band_keys' crates/vox-similarity/src/signature.rs crates/vox-similarity/src/index.rs` — confirm the functions to edit exist.

---

## Task 1: Isolate the engine onto a clean branch [SEQUENTIAL]

> **⚠ Git-surgery task — escalate rather than thrash.** This is a 14-commit cherry-pick that may conflict on `layers.toml`/`where-things-live.md`/`Cargo.toml`. For a fast model this is the highest-risk task in the plan. **Rule:** if any single cherry-pick produces a conflict that is NOT a trivial "keep both rows" merge of registration lines, run `git cherry-pick --abort` and STOP with a handoff note listing the conflicting commit + files — do NOT attempt creative conflict resolution (two-strike applies immediately here, not after two tries). The orchestrating human/controller may prefer to perform this isolation directly and hand the executor the already-clean branch.

**Files:** none (git only).

- [ ] **Step 1: Identify the 14 skill-discovery commits.**

Run: `git log --oneline --reverse --grep='vox-similarity\|vox-skill-discovery\|vox-discover' main..claude/auto-gui-debug-plans-2026-06-18`
Expected: ~14 commits (scaffold → signature → fragment → index → discovery scaffold → options/candidate → code block extraction → mine → export → dedup → ssot → reporter → bin → final). Record their SHAs in order.

- [ ] **Step 2: Create a clean branch off current origin/main.**

```bash
git fetch origin main
git switch -c claude/skill-discovery-engine origin/main
```

- [ ] **Step 3: Cherry-pick the engine commits in order.**

```bash
git cherry-pick <sha1> <sha2> ... <sha14>
```
If a cherry-pick conflicts on `layers.toml`/`where-things-live.md` (because origin/main moved), resolve by KEEPING the other crates' rows and ADDING only the `vox-similarity`/`vox-skill-discovery` rows (do not delete unrelated rows). `git cherry-pick --continue` after each.

- [ ] **Step 4: Verify the isolated branch is green.**

Run: `cargo test -p vox-similarity -p vox-skill-discovery`
Expected: 19 tests PASS.
Run: `cargo run -p vox-arch-check`
Expected: exits 0. **If it fails ONLY because `vox-runtime` is at L1 here (the kitchen-sink branch had promoted it):** do NOT patch it now — that is Task 2. If it fails for the two new crates (WTL/orphan), fix those rows (they are this plan's crates).

- [ ] **Step 5: Commit nothing extra; the branch IS the deliverable base.** Record the branch name in your handoff.

---

## Task 2: Verify (don't blindly accept) the `vox-runtime` L1→L2 change [SEQUENTIAL]

The kitchen-sink branch promoted `vox-runtime` L1→L2 to clear a red baseline. Determine whether that is correct or whether the underlying `vox-config` dependency is the real bug.

**Files:** possibly `docs/src/architecture/layers.toml` and/or `crates/vox-runtime/Cargo.toml`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n 'vox-config|vox_config' crates/vox-runtime/Cargo.toml crates/vox-runtime/src` and `rg -n 'vox-runtime' docs/src/architecture/layers.toml`.
  - **Case A — `vox-runtime` genuinely uses `vox-config` (a real `use vox_config::...`):** L2 is correct. Set `vox-runtime = { layer = 2, max_dependents = 20 }` in `layers.toml` with a comment `# deps vox-config (verified 2026-06)`, run `cargo run -p vox-arch-check` (must pass), and commit `chore(arch): confirm vox-runtime L2 (depends on vox-config)`.
  - **Case B — the `vox-config` dep is only in `Cargo.toml` but never `use`d, or is a leftover:** removing it restores the L1 invariant. Remove the `vox-config` line from `crates/vox-runtime/Cargo.toml`, set `vox-runtime = { layer = 1, max_dependents = 20 }`, run `cargo build -p vox-runtime` + `cargo run -p vox-arch-check` (must pass), and commit `fix(runtime): drop unused vox-config dep; restore L1 invariant`.
  - **Case C — ambiguous (used transitively / unclear):** STOP and write a handoff note with the exact `use` sites found. Do NOT guess.

- [ ] **Step 2: Run the gate.** `cargo run -p vox-arch-check` → exits 0. Paste output.

---

## Task 3: minhash — one hash per shingle (perf hot path) [SEQUENTIAL]

**🔒 AGH-0001 §B-5 hot-path callout.** `minhash` currently re-initializes a blake3 hasher `num_hashes × shingles` times (128 inits/shingle at defaults). Replace with double-hashing: ONE blake3 per shingle, derive lanes by `a + i·b`.

**Files:**
- Modify: `crates/vox-similarity/src/signature.rs` (the `minhash` fn)

- [ ] **Step 1: Replace `minhash` with the double-hashing implementation.**

```rust
/// MinHash with `num_hashes` lanes derived from ONE blake3 per shingle via
/// double-hashing (`a + i·b`). Deterministic; ~num_hashes× fewer blake3 calls
/// than per-(shingle,lane) hashing.
pub fn minhash(shingles: &[String], num_hashes: usize) -> Vec<u32> {
    let mut mins = vec![u32::MAX; num_hashes];
    for s in shingles {
        let h = blake3::hash(s.as_bytes());
        let bytes = h.as_bytes();
        let a = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        // force `b` odd so lanes don't collapse (i·b stays well-distributed mod 2^32)
        let b = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) | 1;
        for (i, slot) in mins.iter_mut().enumerate() {
            let v = a.wrapping_add((i as u32).wrapping_mul(b));
            if v < *slot {
                *slot = v;
            }
        }
    }
    mins
}
```

- [ ] **Step 2: Add a perf-invariant test (lanes are distinct for distinct seeds; determinism holds).** Append to the `tests` module in `signature.rs`:

```rust
    #[test]
    fn minhash_is_deterministic_and_multi_lane() {
        let sh = shingle("the quick brown fox jumps over the lazy dog", 2);
        let a = minhash(&sh, 64);
        let b = minhash(&sh, 64);
        assert_eq!(a, b, "deterministic");
        assert_eq!(a.len(), 64);
        // not all lanes identical (double-hashing spreads them)
        assert!(a.windows(2).any(|w| w[0] != w[1]), "lanes must differ");
    }
```

- [ ] **Step 3: Run the full crate suite.** `cargo test -p vox-similarity`
Expected: all PASS, including the existing `identical_text_has_zero_hamming_and_full_jaccard` (identical text ⇒ identical minhash ⇒ jaccard 1.0) and `dissimilar_text_has_low_jaccard` (< 0.3). If `dissimilar` now exceeds 0.3, that is a real signal — re-check the `b | 1` line; do not relax the assertion.

- [ ] **Step 4: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-similarity -- -D warnings
cargo fmt -p vox-similarity
git add crates/vox-similarity/src/signature.rs
git commit -m "perf(vox-similarity): minhash via double-hashing (one blake3 per shingle)"
```

---

## Task 4: ignore-aware file walking (skip target/.git/node_modules) [SEQUENTIAL]

`mine_repeated_code` walks the whole tree including build artifacts. Switch from `walkdir` to the `ignore` crate so `.gitignore`/hidden dirs are respected.

**Files:**
- Modify: `crates/vox-skill-discovery/Cargo.toml` (swap `walkdir` → `ignore`)
- Modify: `crates/vox-skill-discovery/src/code_miner.rs` (the walk loop)

- [ ] **Step 1 (verify-before-use):** `rg -n 'WalkBuilder|fn build' ~/.cargo/registry/src/*/ignore-0.4*/src/lib.rs 2>/dev/null | head` OR rely on the known API: `ignore::WalkBuilder::new(root).build()` yields `Result<DirEntry, _>`; `entry.path()` is the path; it skips gitignored/hidden by default.

- [ ] **Step 2: Update `Cargo.toml`.** In `crates/vox-skill-discovery/Cargo.toml`, replace the `walkdir = { workspace = true }` line with `ignore = { workspace = true }`.

- [ ] **Step 3: Update the walk in `code_miner.rs`.** Replace the `use walkdir::WalkDir;` import with `use ignore::WalkBuilder;` and replace the walk loop body inside `mine_repeated_code`:

```rust
    for entry in WalkBuilder::new(root).build().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(p) else {
            continue;
        };
        // ... (block extraction unchanged) ...
```

- [ ] **Step 4: Add a test that a gitignored dir is skipped.** Append to the `tests` module in `code_miner.rs`:

```rust
    #[test]
    fn mining_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(root.join("ignored")).unwrap();
        let body = "let subtotal = unit_price * quantity\nlet tax = subtotal * tax_rate\nlet total = subtotal + tax\n";
        // two copies in a tracked dir → a candidate; one extra in ignored/ must NOT inflate members
        std::fs::write(root.join("a.vox"), body).unwrap();
        std::fs::write(root.join("b.vox"), body).unwrap();
        std::fs::write(root.join("ignored").join("c.vox"), body).unwrap();
        let opts = DiscoverOptions { min_tokens: 5, min_occurrences: 2, ..DiscoverOptions::default() };
        let cands = mine_repeated_code(root, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].members.len(), 2, "ignored/ copy must be excluded");
    }
```
(Requires `tempfile` as a dev-dependency — see Task 9 Step 1, which adds it. If running Task 4 before Task 9, add `tempfile = { workspace = true }` under `[dev-dependencies]` in `crates/vox-skill-discovery/Cargo.toml` now.)

- [ ] **Step 5: Run, clippy, fmt, commit.**

```bash
cargo test -p vox-skill-discovery
cargo clippy -p vox-skill-discovery -- -D warnings
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/Cargo.toml crates/vox-skill-discovery/src/code_miner.rs
git commit -m "fix(vox-skill-discovery): ignore-aware walking (skip gitignored/build dirs)"
```

---

## Task 5: signature-length invariant (fail loud, not silent) [SEQUENTIAL]

`jaccard_estimate` returns `0.0` on length mismatch and `band_keys` clamps — both silently degrade if a Fragment's `num_hashes` ≠ the index's `bands*rows`. Add a `debug_assert` so config drift surfaces in tests/dev.

**Files:**
- Modify: `crates/vox-similarity/src/index.rs` (`insert`)

- [ ] **Step 1: Add the invariant in `insert`.** In `LshIndex::insert`, before computing band keys, add:

```rust
        debug_assert_eq!(
            fragment.signature.minhash.len(),
            self.bands * self.rows,
            "Fragment minhash length ({}) must equal bands*rows ({}*{}={}); \
             build Fragments with the same num_hashes as the index config",
            fragment.signature.minhash.len(), self.bands, self.rows, self.bands * self.rows
        );
```

- [ ] **Step 2: Add a release-safe guard test (debug builds panic; assert that matching configs are fine).** Append to the `tests` module in `index.rs`:

```rust
    #[test]
    fn insert_accepts_matching_signature_length() {
        let mut idx = LshIndex::new(16, 4); // 64 hashes
        idx.insert(frag("a", "let total = price * quantity + tax", "a.vox:1")); // frag() uses 64
        assert_eq!(idx.len(), 1);
    }
```

- [ ] **Step 3: Run, clippy, fmt, commit.**

```bash
cargo test -p vox-similarity
cargo clippy -p vox-similarity -- -D warnings
cargo fmt -p vox-similarity
git add crates/vox-similarity/src/index.rs
git commit -m "fix(vox-similarity): debug_assert signature length matches index config"
```

---

## Task 6: representative cluster score + single-linkage doc [SEQUENTIAL]

Cluster `score` uses only the first two members; replace with the mean pairwise jaccard. Document the single-linkage clustering semantics.

**Files:**
- Modify: `crates/vox-similarity/src/index.rs` (add shared `mean_pairwise_jaccard` helper + doc `cluster`)
- Modify: `crates/vox-similarity/src/lib.rs` (export the helper)
- Modify: `crates/vox-skill-discovery/src/code_miner.rs` (`mine_repeated_code` score block)

- [ ] **Step 1: Add a SHARED helper in `vox-similarity` (so both miners and dedup reuse it — DRY).** In `crates/vox-similarity/src/index.rs`, add a free function (after the `impl LshIndex` block):

```rust
/// Mean of pairwise minhash-jaccard over all member pairs of an index cluster.
/// Returns 1.0 for a singleton. Shared by discovery (code clusters) and dedup
/// (skill clusters) so the scoring logic lives in one place.
pub fn mean_pairwise_jaccard(index: &LshIndex, members: &[usize]) -> f32 {
    if members.len() < 2 {
        return 1.0;
    }
    let mut sum = 0.0f32;
    let mut pairs = 0u32;
    for i in 0..members.len() {
        for j in (i + 1)..members.len() {
            let a = &index.fragment(members[i]).signature.minhash;
            let b = &index.fragment(members[j]).signature.minhash;
            sum += jaccard_estimate(a, b);
            pairs += 1;
        }
    }
    sum / pairs as f32
}
```
(`jaccard_estimate` is already imported at the top of `index.rs` via `use crate::signature::{jaccard_estimate, Signature};`.)

- [ ] **Step 2: Export it.** In `crates/vox-similarity/src/lib.rs`, add `mean_pairwise_jaccard` to the `pub use index::{...}` line (alongside `Cluster, LshIndex, Match`).

- [ ] **Step 3: Document single-linkage on `cluster`.** In `index.rs`, extend the `cluster` doc comment:

```rust
    /// Group indexed fragments into clusters via union-find over confirmed
    /// neighbor pairs (**single-linkage**: A and C land in one cluster if a chain
    /// A~B~C exists, even when A and C are not directly above `min_jaccard`).
    /// Returns only clusters with `>= min_members`.
```

- [ ] **Step 4: Use the shared helper in `mine_repeated_code`.** Replace the `let score = if cluster.members.len() >= 2 { ... } else { 1.0 };` block with:

```rust
        let score = vox_similarity::mean_pairwise_jaccard(&index, &cluster.members);
```

- [ ] **Step 5: Add a unit test for the helper.** Append to the `tests` module in `index.rs`:

```rust
    #[test]
    fn mean_pairwise_jaccard_singleton_and_identical() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "let total = price * quantity + tax", "a:1"));
        idx.insert(frag("b", "let total = price * quantity + tax", "b:9"));
        assert_eq!(mean_pairwise_jaccard(&idx, &[0]), 1.0);
        assert!(mean_pairwise_jaccard(&idx, &[0, 1]) >= 0.9);
    }
```

- [ ] **Step 6: Existing `mine_finds_duplicate_block_across_two_files` still asserts `score >= 0.9`** — two identical blocks ⇒ mean pairwise jaccard = 1.0, so it passes. Run `cargo test -p vox-similarity -p vox-skill-discovery`.

- [ ] **Step 7: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-similarity -p vox-skill-discovery -- -D warnings
cargo fmt -p vox-similarity -p vox-skill-discovery
git add crates/vox-similarity/src/index.rs crates/vox-similarity/src/lib.rs crates/vox-skill-discovery/src/code_miner.rs
git commit -m "fix(vox-similarity): shared mean_pairwise_jaccard; representative cluster score; doc single-linkage"
```

---

## Task 7: real measured score for `dedup_skills` [SEQUENTIAL]

`dedup_skills` sets `score: opts.min_jaccard` (the threshold, not the measured overlap). Surface the real mean pairwise jaccard.

**Files:**
- Modify: `crates/vox-skill-discovery/src/catalog.rs` (`dedup_skills`)

- [ ] **Step 1: Compute the real score using the SHARED helper** added to `vox-similarity` in Task 6 (do NOT duplicate the logic here). In `dedup_skills`, in the `for cluster in index.cluster(2, opts.min_jaccard)` loop, compute `let score = vox_similarity::mean_pairwise_jaccard(&index, &cluster.members);` before building the `Candidate`, and replace `score: opts.min_jaccard,` with `score,`. (If executing Task 7 before Task 6, do Task 6 Steps 1–2 first — the helper + export must exist.)

- [ ] **Step 2: Strengthen the existing dedup test.** In `dedup_flags_near_identical_skills`, after the existing asserts add:

```rust
        assert!(cands[0].score >= 0.9, "near-identical skills score high, got {}", cands[0].score);
```

- [ ] **Step 3: Run, clippy, fmt, commit.**

```bash
cargo test -p vox-skill-discovery
cargo clippy -p vox-skill-discovery -- -D warnings
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src/catalog.rs
git commit -m "fix(vox-skill-discovery): dedup_skills reports measured overlap, not threshold"
```

---

## Task 8: test temp-dir hygiene via `tempfile` [SEQUENTIAL]

Replace the pid-keyed temp dir in `mine_finds_duplicate_block_across_two_files` with `tempfile::tempdir()` (unique + RAII cleanup).

**Files:**
- Modify: `crates/vox-skill-discovery/Cargo.toml` (`[dev-dependencies]`)
- Modify: `crates/vox-skill-discovery/src/code_miner.rs` (the test)

- [ ] **Step 1: Add the dev-dep.** In `crates/vox-skill-discovery/Cargo.toml`, under `[dev-dependencies]` (create the section if absent), add `tempfile = { workspace = true }`. (If Task 4 already added it, skip.)

- [ ] **Step 2: Rewrite the temp-dir test.** Replace the body of `mine_finds_duplicate_block_across_two_files` that uses `std::env::temp_dir().join(format!("voxdisc_{}", std::process::id()))` and the manual `remove_dir_all` with:

```rust
    fn mine_finds_duplicate_block_across_two_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let body = "let subtotal = unit_price * quantity\nlet tax = subtotal * tax_rate\nlet total = subtotal + tax\nreturn total\n";
        std::fs::write(root.join("a.vox"), body).unwrap();
        std::fs::write(root.join("b.vox"), body).unwrap();
        let opts = DiscoverOptions { min_tokens: 5, min_occurrences: 2, ..DiscoverOptions::default() };
        let cands = mine_repeated_code(root, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].kind, CandidateKind::RepeatedCode);
        assert_eq!(cands[0].members.len(), 2);
        assert!(cands[0].score >= 0.9);
        // `dir` auto-removed on drop
    }
```

- [ ] **Step 3: Run, clippy, fmt, commit.**

```bash
cargo test -p vox-skill-discovery
cargo clippy -p vox-skill-discovery -- -D warnings
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/Cargo.toml crates/vox-skill-discovery/src/code_miner.rs
git commit -m "test(vox-skill-discovery): use tempfile for temp dirs (unique + RAII cleanup)"
```

---

## Task 9: Final verification [SEQUENTIAL]

- [ ] **Step 1:** `cargo test -p vox-similarity -p vox-skill-discovery` — paste counts (≥ 22 tests now).
- [ ] **Step 2:** `cargo clippy -p vox-similarity -p vox-skill-discovery -- -D warnings` — clean.
- [ ] **Step 3:** `cargo run -p vox-arch-check` — exits 0.
- [ ] **Step 4:** Smoke: `cargo run -p vox-skill-discovery --bin vox-discover -- --root crates/vox-similarity --min-tokens 20 --min-occurrences 2` — runs, prints a report.
- [ ] **Step 5: Delivery manifest (AGH-0001 §B-4).** In your handoff, list every file changed and confirm the ONLY `layers.toml` change is the Task-2 `vox-runtime` line.

## Self-Review (author)
- **Coverage:** review findings 1 (minhash perf, T3), 2 (ignore walking, T4), 3 (signature invariant, T5), 4 (single-linkage doc, T6), 5 (cluster score, T6), 6 (dedup score, T7), 7 (test hygiene, T8); process findings 8 (isolation, T1), 9 (vox-runtime, T2).
- **DRY:** the cluster-scoring helper `vox_similarity::mean_pairwise_jaccard(&LshIndex, &[usize]) -> f32` is defined ONCE in `vox-similarity` (T6) and reused by both `code_miner` (T6) and `catalog::dedup_skills` (T7) — no duplicated scoring logic.
- **Ordering:** T1→T2 sequential first (isolation + arch). T6 adds the shared helper that **T7 depends on**, so T6 must precede T7. `jaccard_estimate(&[u32],&[u32])->f32` unchanged; `DiscoverOptions { min_tokens, min_occurrences, .. }` consistent.
- **Parallelism:** after T1/T2, T3/T5 (vox-similarity) and T4/T8 (vox-skill-discovery) are two parallel lanes; T6 spans both crates and T7 depends on it → run T6 then T7 on one agent. Keep `[SEQUENTIAL]` unless dispatching by lane.
