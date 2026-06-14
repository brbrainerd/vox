# Guardrail Capability Plan — Catch the Perennial Bug Classes with Measured Precision/Recall

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Subagent *writes* are denied in the Claude worktree sandbox ([[feedback_subagents_readonly_in_sandbox]]) — implement + commit in the main session; use read-only subagents only for review.

**Goal:** Close the highest-leverage detection gaps surfaced by (a) the 5,977-node graphify codebase audit and (b) a scan of 528 `fix()` commits, by adding/tuning static guardrails — TOESTUB detectors, `rules.v1.yaml` rules, CI gates, and AGENTS.md guidance — each **measured by the existing F1 fixture bench** so we raise true-positives without raising false-positives.

**Architecture:** Build on the *existing* mature detection stack, do not reinvent it:
- 51 Rust detectors implementing `DetectionRule` (`crates/vox-code-audit/src/detectors/*.rs`, registered in `mod.rs::all_rules()`).
- ~90 pattern rules in `contracts/code-audit/rules.v1.yaml` (schema `rules.v1.schema.json`).
- The **F1 bench** (`crates/vox-rule-pack/src/bench.rs`, CLI `vox ci detect-rules-bench --min-f1 0.70`) computing precision/recall/F1 per rule against `contracts/code-audit/fixtures/<parent-id>/<sub-id>_{pos,neg}_*.txt`.
- Existing FP-reduction toolkit to reuse: comment/string skipping (`rust_byte_is_non_code`), `#[test]`/`#[cfg(test)]` exclusion, allowlists, `// toestub-ignore(<rule>)` suppression, confidence levels, and per-rule thresholds.

**The TDD loop is the F1 bench itself.** For every detector task: **write the pos/neg fixtures first** (they ARE the spec + the test), run the bench to watch it fail (no detector / F1=0), implement the detector, run the bench until F1 ≥ the task's target. This is genuine TDD with precision/recall as the pass condition.

**Tech Stack:** Rust (`vox-code-audit`, `vox-rule-pack`), the `rules.v1.yaml` rule-pack, GitHub Actions, AGENTS.md.

**Non-overlap:** `docs/superpowers/plans/2026-06-07-wiring-remediation.md` *fixes* specific wiring gaps and builds zero detection; this plan builds the *prevention* layer so the classes can't silently recur. No task here duplicates it.

---

## Grounding data (what the audit + history actually show)

| Signal | Source | Magnitude | Current guard | Verdict |
|---|---|---|---|---|
| **Reached-but-unproven symbols** (executed in tests, no assertion on output) | `graphify-out/COVERAGE_BEHAVIORS_INDEX.md` | **7,950** | TDD-guard enforces test *presence* only | **GAP** (the headline) |
| **Happy-only gaps** (only happy-path covered) | same | **1,751** | none | GAP |
| **Exact-body duplicate clusters** (cross-crate split-brain) | `graphify-out/DUPLICATION_AND_WIRING.md` | **435** (e.g. `preflight_native_qlora`, `finalize_training_run`, `deliver_a2a`, `build_dir`×3) | `dry_violation` = **same-file only** | **GAP** (cross-crate) |
| **Catch-all-swallow** (e.g. `mutation_kind_for_tool` → `read_only` for unknown tools) | `agentos_mutation.rs:58`; history headline `let`→`Decl::Const`→catch-all→vanishes | 1 confirmed + a class | none specific | GAP |
| **Toolchain-bump lint wave** | 528-fix scan (1.96 `manual_is_multiple_of`, `field_reassign_with_default`) | recurring per bump | clippy in CI but cache masks new lints | GAP (no toolchain-change gate) |
| **Pre-push clippy gap** | 528-fix scan (~45 clippy fixes) | recurring | only `--complete`+ runs clippy | partial |
| Hotspots | coverage index | vox-compiler 371 / vox-orchestrator 277 / vox-codegen 165 happy-only gaps | — | informs prioritization |

Already well-covered (do NOT rebuild): stub/hollow/empty-body, effect/purity, secret-shape, LLM-provider-call, crypto-ban, retired-surfaces, decorator-position, state-machine-unreachable, import-cycles, arch layering. These have detectors + F1 fixtures.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `contracts/code-audit/fixtures/test-assertion-depth/*` (create) | pos/neg fixtures for the assertion-depth detector | T1 |
| `crates/vox-code-audit/src/detectors/test_assertion_depth.rs` (create) | detect tests that call a pub fn but assert nothing on its output | T1 |
| `contracts/code-audit/fixtures/cross-crate-dup/*` + `crates/vox-code-audit/src/detectors/cross_crate_dup.rs` (create) | cross-crate exact-body-hash split-brain detector | T2 |
| `contracts/code-audit/fixtures/catch-all-swallow/*` + `crates/vox-code-audit/src/detectors/catch_all_swallow.rs` (create) | match `_ =>` arm returning Default/empty for an enum scrutinee | T3 |
| `.github/workflows/ci.yml` (modify) | toolchain-change → clean clippy+rustdoc gate | T4 |
| `contracts/code-audit/fixtures/<low-f1-rules>/*` (expand) | enlarge pos/neg corpora for the weakest existing rules; raise min-f1 | T5 |
| `crates/vox-code-audit/src/detectors/mod.rs` + `rules.v1.yaml` + `docs/.../detector-coverage-ledger.md` (modify/create) | register detectors; a bug-class→detector→F1 ledger | T6 |

Tasks are independent; recommended order **T4 → T2 → T3 → T1 → T5 → T6** (cheapest/highest-precision first; T1 is highest-value but highest-FP-risk so it lands after the bench muscle is warm).

---

## Task T4: Toolchain-bump lint-wave CI gate (cheapest, highest-certainty)

**Files:** Modify `.github/workflows/ci.yml` (add a job).

**Why:** Every `rust-toolchain.toml` bump introduces new clippy/rustdoc lints; on the self-hosted fleet the persisted `target/` + sccache mask them, so they land and need a cleanup commit. A job that runs **fresh** (clean) clippy+rustdoc only when the toolchain file changes catches the wave in the bumping PR.

- [ ] **Step 1: Write the failing check (a workflow assertion)**

There is no unit-test harness for workflows; the "test" is: the job must (a) trigger only on `rust-toolchain.toml` change, (b) run clippy with the cache defeated. Capture the intent as a job whose run-script greps the PR diff. Add to `ci.yml`:
```yaml
  toolchain-lint-wave:
    name: Fresh clippy+rustdoc on toolchain bump
    needs: setup
    if: github.event_name == 'pull_request'
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 40
    steps:
      - uses: actions/checkout@v6
        with: { fetch-depth: 0 }
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - name: Detect toolchain change
        id: tc
        shell: bash
        run: |
          git fetch origin "${{ github.base_ref }}" --depth=1 || true
          if git diff --name-only "origin/${{ github.base_ref }}...HEAD" | grep -qx 'rust-toolchain.toml'; then
            echo "changed=true" >> "$GITHUB_OUTPUT"
          else
            echo "changed=false" >> "$GITHUB_OUTPUT"
          fi
      - name: Fresh clippy + rustdoc (cache defeated)
        if: steps.tc.outputs.changed == 'true'
        env:
          CARGO_INCREMENTAL: "0"
          RUSTC_WRAPPER: ""   # bypass sccache so the new toolchain re-evaluates every crate
        run: |
          cargo clean
          cargo clippy --workspace --all-targets --exclude vox-gui -- -D warnings \
            -A clippy::items_after_test_module -A clippy::collapsible_match \
            -A clippy::collapsible_if -A clippy::should_implement_trait \
            -A clippy::doc_overindented_list_items -A clippy::doc_lazy_continuation
          RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
```
(The clippy allowlist MUST stay in lockstep with the `lints` job's allowlist — reference it.)

- [ ] **Step 2: Validate parse**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` and (if available) `act --list -W .github/workflows/ci.yml`.
Expected: parses; `toolchain-lint-wave` present; nothing `needs:` it (cannot block the required gate).

- [ ] **Step 3: Verify the trigger logic locally**

Run: `git diff --name-only origin/main...HEAD | grep -qx 'rust-toolchain.toml' && echo would-run || echo would-skip`
Expected: `would-skip` on a PR that doesn't touch the toolchain; `would-run` when it does.

- [ ] **Step 4: Commit**
```bash
git add .github/workflows/ci.yml
git commit -m "ci: fresh clippy+rustdoc gate on rust-toolchain bump (catch new-lint waves)"
```

> **Honest caveat (carry into review):** the `RUSTC_WRAPPER: ""` + `cargo clean` is the robust way to defeat sccache/persisted-target masking, but it costs a full cold build *on toolchain-bump PRs only* (rare). Confirm on the first real toolchain-bump PR that a deliberately-introduced new lint actually fails this job.

---

## Task T2: Cross-crate split-brain detector (exact-body-hash, high precision)

**Files:** Create `crates/vox-code-audit/src/detectors/cross_crate_dup.rs`, fixtures `contracts/code-audit/fixtures/cross-crate-dup/`; modify `detectors/mod.rs`.

**Why:** `dry_violation` only finds near-duplicates *within one file*. The graph found **435 exact-body duplicate clusters across crates** (`preflight_native_qlora` in the cuda + metal plugins, `deliver_a2a` in populi-plugin vs populi-core). Exact-body equality = very high precision (low FP) and is the split-brain that diverges silently.

- [ ] **Step 1: Write fixtures (the spec)**

Create `contracts/code-audit/fixtures/cross-crate-dup/`:
- `exact_pos_a.txt` and `exact_pos_b.txt` — same normalized function body (whitespace/comment-stripped), two files. Should match (a cluster).
- `near_neg_a.txt` / `near_neg_b.txt` — bodies differing by one statement. Should NOT match (only EXACT bodies; near-dup is `dry_violation`'s job, kept separate to avoid double-flag FP).
- `trivial_neg.txt` — a 2-line `fn new() -> Self { Self::default() }`. Should NOT match (below `min_body_lines`, and on the allowlist).

- [ ] **Step 2: Run the bench, watch it fail**

Run: `cargo run -p vox-cli -- ci detect-rules-bench --fixtures contracts/code-audit/fixtures --rules contracts/code-audit/rules.v1.yaml --min-f1 0.70`
Expected: the `cross-crate-dup` fixtures are unscored / F1 = 0 (no detector yet).

- [ ] **Step 3: Implement the detector**

`cross_crate_dup.rs` (DetectionRule, batch-mode — it needs all files, like `import_cycles`'s batch path):
```rust
// Normalize each fn body: strip comments (rust_byte_is_non_code), collapse whitespace,
// hash. Group by hash across files. Flag any hash with >=2 sites in DIFFERENT crates.
// FP guards: min_body_lines >= 5; skip ALLOWED_FN_NAMES (reuse hollow_fn's list);
// skip #[test]/#[cfg(test)]; honor `// toestub-ignore(cross-crate-dup)`.
```
- `id`: `arch/cross-crate-dup`, severity `Warning`, languages Rust (+ Vox later).
- Register in `mod.rs::all_rules()` and bump `rule_count()`.

- [ ] **Step 4: Run the bench until precision is high**

Run the bench command from Step 2.
Expected: `cross-crate-dup` F1 ≥ **0.90** (exact-hash → precision should be ~1.0; tune `min_body_lines`/allowlist until the `*_neg_*` cases don't match).

- [ ] **Step 5: Smoke against the real finding**

Run: `cargo run -p vox-code-audit --bin toestub -- crates --rules arch/cross-crate-dup --format terminal | grep -i preflight_native_qlora`
Expected: it flags the known `preflight_native_qlora` cuda/metal cluster (a true positive from the graph).

- [ ] **Step 6: Commit**
```bash
git add crates/vox-code-audit/src/detectors/cross_crate_dup.rs crates/vox-code-audit/src/detectors/mod.rs contracts/code-audit/fixtures/cross-crate-dup/
git commit -m "feat(code-audit): cross-crate exact-body split-brain detector (F1>=0.90)"
```

---

## Task T3: Catch-all-swallow detector (`match _ =>` returning empty for an enum)

**Files:** Create `crates/vox-code-audit/src/detectors/catch_all_swallow.rs`, fixtures `contracts/code-audit/fixtures/catch-all-swallow/`; modify `mod.rs`.

**Why:** The headline pipeline bug (`let`→`Decl::Const`→catch-all→vanishes) and `mutation_kind_for_tool()` returning `read_only` for unknown tools are the same shape: a `match` whose `_ =>` arm returns `Default::default()`/`None`/`Vec::new()`/an empty/neutral value for a scrutinee that is an enum with named variants — silently swallowing cases that should be handled or errored.

- [ ] **Step 1: Write fixtures**

`contracts/code-audit/fixtures/catch-all-swallow/`:
- `swallow_pos_default.txt` — `match kind { Kind::A => real(), _ => Default::default() }`. Match.
- `swallow_pos_none.txt` — `_ => None` after real arms. Match.
- `exhaustive_neg.txt` — `match` with all variants named, no `_`. No match.
- `error_neg.txt` — `_ => return Err(...)` / `_ => unreachable!()` / `_ => panic!()`. No match (explicit handling, not silent).
- `passthrough_neg.txt` — `_ => x` (returns the scrutinee/an input, not a neutral default). No match.

- [ ] **Step 2: Run bench, watch fail** (as T2 Step 2, for `catch-all-swallow`).

- [ ] **Step 3: Implement**

`catch_all_swallow.rs` using the Rust token/AST context (`RustFileContext`, as `reachability`/`hollow_fn` do):
```rust
// For each `match` expr: if it has a wildcard `_ =>` (or binding-without-use) arm whose
// body is a KNOWN-NEUTRAL value (Default::default(), None, Vec::new(), "", 0, false, an
// empty block) AND there is >=1 explicit non-wildcard arm → flag.
// FP guards: do NOT flag if the wildcard arm returns/raises (Err/panic/unreachable/todo),
// rebinds-and-uses the scrutinee, or the match has a single arm; skip #[test];
// honor `// toestub-ignore(catch-all-swallow)`. Start severity = Info (advisory) until F1 proves out.
```
- `id`: `vox/catch-all-swallow` (also applies to Vox `when{}`/`match`); register + count.

- [ ] **Step 4: Tune to target**

Run the bench.
Expected: F1 ≥ **0.80**. Catch-alls are legitimately common (CLI arg parsing, etc.), so expect to add several `*_neg_*` fixtures; if precision stays < 0.8, keep severity `Info` and narrow to enum-typed scrutinees only.

- [ ] **Step 5: Smoke** against `crates/vox-foundation/src/primitives/agentos_mutation.rs` — expect it flags `mutation_kind_for_tool`'s `_ => read_only`-style fallback (the confirmed TP), and does NOT flag a nearby exhaustive match.

- [ ] **Step 6: Commit** (`feat(code-audit): catch-all-swallow detector (Info; F1>=0.80)`).

---

## Task T1: Test-assertion-depth detector (the headline TP gap — 7,950 unproven symbols)

**Files:** Create `crates/vox-code-audit/src/detectors/test_assertion_depth.rs`, fixtures `contracts/code-audit/fixtures/test-assertion-depth/`; modify `mod.rs`.

**Why:** The single biggest gap: TDD-guard enforces a test *exists*, never that it *asserts*. The graph found **7,950 reached-but-unproven symbols**. A test that calls `foo()` but never asserts on the result is a "structural-only golden" — it touches code for coverage without proving behavior (the "useless touch" the user has explicitly rejected). **Highest value, highest FP risk** — land it last, ship at `Info`, tune hard.

- [ ] **Step 1: Write fixtures (define "meaningful assertion")**

`contracts/code-audit/fixtures/test-assertion-depth/`:
- `no_assert_pos.txt` — `#[test] fn t() { let _ = foo(); }` (calls, binds to `_`, asserts nothing). Match.
- `call_only_pos.txt` — `#[test] fn t() { foo(); }` (call in statement position, no assert, no `?`). Match.
- `assert_neg.txt` — `assert_eq!(foo(), 3);` / `assert!(...)`. No match.
- `expect_panic_neg.txt` — `#[should_panic]` test, or `foo().unwrap()` where the unwrap IS the assertion. No match.
- `insta_neg.txt` — `insta::assert_snapshot!(foo())` / `assert_yaml_snapshot!`. No match (snapshot = assertion).
- `propagate_neg.txt` — `let x = foo()?; use_x(x);` inside a `-> Result` test where the value flows into a later assert. No match (value is used, not discarded).

- [ ] **Step 2: Run bench, watch fail.**

- [ ] **Step 3: Implement (conservative — only the unambiguous cases)**

`test_assertion_depth.rs`:
```rust
// For each #[test] fn (and golden @test): collect call-exprs to non-std fns. FLAG only when
// the test body contains a call whose result is discarded (`let _ =`, or call in statement
// position) AND the body has ZERO assertion signals anywhere:
//   assert!/assert_eq!/assert_ne!/assert_matches!, insta::assert_*, .unwrap()/.expect(),
//   #[should_panic], panic!/unreachable! as expected-path, or `?`-propagation into a used value.
// If ANY assertion signal is present, do NOT flag (a test with one assert is "proven enough"
// for v1 — depth-per-symbol is a later iteration). severity = Info. Honor toestub-ignore.
```
- `id`: `vox/api/test-asserts-nothing`. Register + count.

- [ ] **Step 4: Tune to target on the real corpus**

Run the bench, then a real-corpus precision check:
`cargo run -p vox-code-audit --bin toestub -- crates/vox-integration-tests --rules vox/api/test-asserts-nothing --format json | <count>`
Expected: bench F1 ≥ **0.75**; manually inspect 20 real hits — **precision ≥ 0.9 on the sample** (false positives here erode trust fast). If precision is low, narrow further (only flag `let _ =`-discarded results). Ship `Info`, never blocking, until a follow-up proves precision.

- [ ] **Step 5: Commit** (`feat(code-audit): test-asserts-nothing detector (Info; structural-only-golden gap)`).

---

## Task T5: Reduce false-positives on the weakest existing rules (raise the floor)

**Files:** Expand `contracts/code-audit/fixtures/<rule>/` for the lowest-F1 rules; consider raising `--min-f1`.

**Why:** Increasing TP is half the job; the other half is trust. A rule that misfires gets ignored (or `toestub-ignore`d) and stops catching real bugs. The bench already measures per-rule F1 — use it to find and fix the weak ones.

- [ ] **Step 1: Rank rules by current F1**

Run: `cargo run -p vox-cli -- ci detect-rules-bench --fixtures contracts/code-audit/fixtures --rules contracts/code-audit/rules.v1.yaml --min-f1 0.0 --json > /tmp/f1.json` then sort ascending by F1.
Expected: a ranked list; identify the bottom ~8 rules (closest to the 0.70 floor) and whether they fail on precision (FP) or recall (FN).

- [ ] **Step 2: For each weak rule, add targeted fixtures**

For a precision-weak rule, add `*_neg_*` fixtures reproducing its real-world false positives (mine them: run the detector on `crates/` and eyeball misfires). For a recall-weak rule, add `*_pos_*` fixtures for missed shapes. Then tune the detector's guards (thresholds, skip-contexts, allowlists) using the existing FP toolkit until both pos and neg pass.

- [ ] **Step 3: Re-run the bench; raise the floor if safe**

Run the bench at `--min-f1 0.70`; once the bottom rules clear ~0.78, bump the CI gate (`ci.yml`) `--min-f1 0.70 → 0.75`.
Expected: all rules ≥ new floor; CI gate raised.

- [ ] **Step 4: Commit** (`test(code-audit): expand fixtures for low-F1 rules; raise min-f1 floor`).

---

## Task T6: Detector-coverage ledger + AGENTS.md wiring (make gaps visible)

**Files:** Create `docs/src/contributors/detector-coverage-ledger.md`; modify AGENTS.md §"Perennial Bug Patterns" to link it.

**Why:** Today there's no single view of "bug-class → detector → F1 → enforcement point," so gaps are invisible until a graph audit. A committed ledger makes the prevention layer auditable and tells the next agent where the holes are.

- [ ] **Step 1: Write the ledger**

`detector-coverage-ledger.md` (with frontmatter — it's under `docs/src/`): a table of every bug class (from this plan + the audit), its detector id (or "GAP"), current F1, severity, and enforcement point (pre-commit / pre-push fast|complete / CI). Include the still-open gaps (e.g. semantic-equivalence split-brain, per-symbol assertion depth) as explicit `GAP` rows so they're tracked, not forgotten.

- [ ] **Step 2: Link from AGENTS.md**

In §"Perennial Bug Patterns (catch early)" add one line: "Coverage of these classes by detector + F1 is tracked in [detector-coverage-ledger.md](docs/src/contributors/detector-coverage-ledger.md); add a row when you add a detector."

- [ ] **Step 3: Verify frontmatter + doc lint**

Run: `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/contributors/detector-coverage-ledger.md`
Expected: passes (valid frontmatter `title`/`description`/`category`).

- [ ] **Step 4: Commit** (`docs: detector-coverage ledger + AGENTS.md link`).

---

## Self-Review

**1. Spec coverage (against the ask — catch perennial bugs via AGENTS.md / stub / rules / CI):**
- AGENTS.md → the "Perennial Bug Patterns" section (already shipped in #288) + T6 ledger link. ✔
- TOESTUB/stub detectors → T1 (assertion-depth), T2 (cross-crate split-brain), T3 (catch-all-swallow). ✔
- rules.v1.yaml / detector framework → all detector tasks register + ship fixtures. ✔
- CI/CD check → T4 (toolchain lint wave); T5 raises the F1 gate. ✔
- **Reduce FP / increase TP** → every detector task gates on the F1 bench with an explicit target; T5 is dedicated FP reduction; high-FP-risk detectors (T1, T3) ship `Info` until precision proves out. ✔

**2. Placeholder scan:** No "add validation/handle edge cases" hand-waves — each detector task names the exact FP guards (allowlists, `#[test]` skip, neutral-value list, `toestub-ignore`, min-body-lines) and a numeric F1 target. The two genuinely-uncertain spots (T1 precision, T4 sccache-masking) are written as explicit verify-on-real-corpus / verify-on-first-bump steps, not deferrals.

**3. Type/name consistency:** detector ids are unique and namespaced (`arch/cross-crate-dup`, `vox/catch-all-swallow`, `vox/api/test-asserts-nothing`); each task registers in `mod.rs::all_rules()` + bumps `rule_count()` + ships `fixtures/<parent-id>/`; all reference the same bench command and `--min-f1` gate.

**Open risks the implementer must respect:** (a) T1 is the highest-value but highest-FP detector — keep it `Info` and prove precision ≥0.9 on a real sample before anyone proposes promoting it; (b) T4's cache-defeat must be confirmed against one real toolchain-bump PR; (c) cross-crate-dup (T2) must not double-flag what `dry_violation` already flags (keep it exact-body-only, cross-crate-only).
