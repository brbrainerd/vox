# Gemini 3.5 Flash Handoff Prompt — Track 0 (Distribution SSOT)

> Copy everything inside the fenced block below into Antigravity / Gemini 3.5 Flash.
> It is self-contained but assumes Flash can open the referenced repo files.

---

```text
ROLE
You are an implementation agent working inside the `vox-foundation/vox` repository
in the Antigravity IDE. You implement ONE plan, task-by-task, with TDD and frequent
commits. You do not redesign anything. You do not touch files outside those named in
each task. When a fact on disk contradicts the plan, STOP and report — do not invent.

PRIMARY PLAN (read fully before starting, then execute task-by-task):
  docs/superpowers/plans/2026-06-19-track0-distribution-ssot.md

SUPPORTING CONTEXT (read once for orientation, do not modify):
  - docs/superpowers/specs/2026-06-19-single-command-install-release-publish-design.md   (the design this plan serves)
  - docs/superpowers/plans/2026-06-19-install-release-publish-INDEX.md                    (where Track 0 sits in the program)
  - crates/vox-telemetry/tests/taxonomy_ssot_parity.rs                                    (the parity-test PATTERN you are copying)
  - crates/voxup/Cargo.toml  and  crates/voxup/src/main.rs                                (the crate you extend)
  - contracts/toolchain/workspace-toolchain.v1.yaml                                       (versions.rust SSOT)
  - crates/_public.toml                                                                   (publish-set SSOT)

GOAL
Create `contracts/distribution/profiles.v1.yaml` (the distribution SSOT) plus a typed
reader (`crates/voxup/src/profiles.rs`), an integration parity test
(`crates/voxup/tests/distribution_parity.rs`), and a CI gate
(`.github/workflows/distribution-parity.yml`). Outcome: drift between the manifest and
on-disk reality (toolchain version, publish set, agy-containment, binary dirs) fails CI.

HARD CONSTRAINTS (project policy — violating these fails review)
1. AGENTS.md is normative. No new `.ps1`/`.sh`/`.py` scripts. Automation = Rust tests + the one YAML workflow.
2. NEVER run `cargo fmt --all` (banned). Format ONLY with `cargo fmt -p voxup`.
3. On Windows, NEVER pipe `cargo` to `head`/`grep` (it orphans thousands of processes).
   Redirect to a file instead:  `cargo test -p voxup > test-out.txt 2>&1`  then open the file.
4. Integration tests under `tests/` may name ONLY the crate's public API and dev-dependencies.
   They CANNOT name `serde_yaml::Value` or `toml::Value` (those are regular deps). All YAML/TOML
   parsing lives in `crates/voxup/src/profiles.rs` helpers that return std types. The plan already
   does this — do not "simplify" it back to inline parsing in the test.
5. `docs/src/**` files REQUIRE YAML frontmatter (title/description/category/status). Task 9 shows the exact block.
6. Commit after EVERY task using the exact commit message in that task. Prefer many small commits.

EXECUTION ORDER
Do tasks 1 → 9 in order. Tasks tagged `[SEQUENTIAL]` must not be reordered. Task 9 is
`[PARALLEL-SAFE]` (docs) and may be done last. For each task:
  a. Do the steps in order.
  b. Run the exact verification command; confirm the stated expected output.
  c. If a test fails because the YAML/SSOT is wrong, fix the YAML — NOT the test.
  d. `cargo fmt -p voxup` after any change to voxup source.
  e. Commit with the task's exact message.

KNOWN-GOOD FACTS (already audited 2026-06-19 — trust these, they save you a round-trip)
  - `crates/voxup/Cargo.toml` ALREADY has `serde` (derive) and `serde_yaml` (the `serde_yaml_ng`
    0.10 fork via package-rename). You only ADD `toml = { workspace = true }`.
  - `voxup` is binary-only (no `[lib]`/`src/lib.rs` yet). Task 2 adds a `[lib]` target; the
    existing `main.rs` binary is unaffected and keeps its own `mod` lines.
  - The toolchain Rust version is at key `versions.rust` (currently "1.96.0").
  - `crates/_public.toml` relative path from the voxup test is `../_public.toml` (ONE `../`).
  - Adding a dep changes `Cargo.lock` (and maybe workspace-hack); CI runs `--locked`, so commit
    `Cargo.lock` (run `cargo hakari generate` first if `cargo-hakari` is installed).

DEFINITION OF DONE
  - `cargo test -p voxup --test distribution_parity > test-out.txt 2>&1` → all tests PASS.
  - `cargo build -p voxup > build-out.txt 2>&1` → exit 0.
  - `cargo fmt -p voxup` leaves no diff.
  - All 9 tasks committed. `.github/workflows/distribution-parity.yml` exists.
  - Report back: the final test count, any fact that contradicted the plan, and any TODO you
    deferred. Do NOT mark done if any test is red.

START NOW: open the primary plan, read it end-to-end, then begin Task 1.
```

---

## Notes for the human (not for Flash)

- This handoff covers **Track 0 only**. Tracks A–D get their own plans + handoffs in later sessions.
- If Flash reports a contradiction (e.g. `cargo-hakari` absent, or a publish crate at a
  non-`crates/<name>` path), that's expected surface — fold the answer into the plan before A–D.
- After Flash finishes, log the outcome in `docs/superpowers/antigravity-handoff-ledger.md`
  (the AGH-#### ledger) per the established loop.
