---
title: "The Vox Contribution Loop"
description: "How Vox contributions feed the MENS training pipeline, why quality gates matter doubly, and what makes code training-eligible."
category: "Contributors"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Core motivational narrative for the contribution-to-corpus feedback loop."

schema_type: "TechArticle"
---

# The Vox Contribution Loop

Every quality gate in this repository has two jobs: (1) keep the codebase sound, and (2) keep the training corpus clean. This page explains the loop and what it means for your contributions.

## The shipped loop (today)

```text
① WRITE
  .vox files, Rust code, golden examples
  │
② VERIFY  ← where most of your friction happens
  vox stub-check      — zero stubs / hollow fns
  cargo check/test    — compiler + unit tests green (prefer `-p <crate>` / nextest — see [AI dev loop overhead](../architecture/ai-dev-loop-overhead-2026.md))
  vox corpus eval     — .vox parse_rate ≥ 99.5%
  │                    ↓ fails here → negative example pool
③ INGEST
  examples/golden/**/*.vox  ─→  vox corpus validate-batch
  synthetic.jsonl           ─→  synthetic_valid.jsonl
                            ─→  golden_validated.jsonl
  │
④ TRAIN
  vox mens train  (Candle QLoRA, local GPU or cloud)
  │
⑤ IMPROVE
  Better .vox completions via LSP + local Populi serve
  └─→ back to ①
```

The verify step is the filter. Contributions that pass become training data;
contributions with stubs, hollow functions, or parse failures become
**negative examples** — the model learns to avoid those patterns.

## What makes a contribution training-eligible

To land in the positive training pool, a `.vox` file or Rust change must:

| Check | Command | Threshold |
| --- | --- | --- |
| Zero stubs and hollow fns | `vox stub-check --path <dir>` | No `Error` findings |
| Compiler clean | `cargo check -p <crate>` | Zero errors |
| Tests present and passing | `cargo test -p <crate>` | Green |
| `.vox` parse rate | `vox corpus eval --mode ast` | ≥ 99.5% |
| No CRLF line endings | `vox ci line-endings` | Zero CRLF |
| Docs code blocks valid | `// vox:skip` or `{{#include}}` | No bare snippets |
| `@test` block exists for new `.vox` capability | written before the implementation | One `@test` per new exported fn |

## @test-first for golden examples

The highest-signal workflow for new `.vox` capabilities follows a Red → Green → Ingest loop:

1. **Red** — write an `@test` block that calls the function you intend to add. The function doesn't exist yet, so `vox check` fails. That failure is the spec.
2. **Green** — implement the function until `vox check` and `cargo test` pass.
3. **Refactor** — clean up while keeping the test green.
4. **Ingest** — run `vox corpus eval` and commit. The `@test` block raises `r_test` in the planned GRPO reward signal (see §Planned additions).

Skipping step 1 (writing the implementation before the test) is not an error today, but it produces lower-quality corpus entries: the model learns the output without learning the intention. Agents are expected to follow @test-first for any new exported function added to `examples/golden/`.

## What sends code to the negative pool

The system generates negative training examples from:

- `stub/todo`, `stub/unimplemented`, `skeleton/hollow-fn` findings
- Missing `@test` for new exported `.vox` functions in `examples/golden/` (flagged by `skeleton/no-test-for-pub-fn`)
- `.vox` parse failures during `vox corpus validate-batch`
- MCP pre-emit validation failures (planned — see roadmap section)
- Replans triggered by failed victory-condition tiers

This is **not punitive** — negative examples are essential for DPO training.
But it means AI-generated skeleton code that looks plausible does real harm if
it enters the corpus unchecked. The `VictoryClaimDetector` specifically watches
for "implementation complete" adjacent to `unimplemented!()`.

## The golden examples path

The highest-signal contribution you can make to MENS is a well-formed golden example that follows @test-first (see §@test-first for golden examples above):

```text
examples/golden/<capability>.vox
```

Golden files are compiled against the current compiler in CI
(`cargo test -p vox-compiler --test golden_vox_examples`), validated for
corpus quality, and have first-priority ingest into the training mix.

See the [examples SSOT](../../../examples/examples.ssot.v1.yaml) for the
declared golden roots and the [golden examples corpus guide](../how-to/examples-corpus.md)
for how to add one correctly.

## Checking your own contribution's quality

```bash
# 1. Stub check on your directory
cargo run -p vox-cli --features stub-check -- stub-check crates/your-crate

# 2. Compiler + tests
cargo check -p your-crate
cargo test -p your-crate

# 3. .vox corpus quality (if you touched .vox files)
cargo run -p vox-cli -- corpus eval --mode ast examples/golden/

# 4. Push-ready parity (single command — mirrors merge-blocking CI subset)
cargo run -p vox-cli -- ci pre-push
# Narrow doc-only iteration: `vox ci pre-push --quick` still runs doc lint + doctest-md + drift-check
# Optional timings artifact: add `--report-json target/local/pre-push-last.json`
```

## After merging a snapshot-touching PR

The test suite uses [insta](https://insta.rs/) for snapshot assertions. CI runs with
`INSTA_UPDATE=unseen` ([`.github/workflows/ci.yml`](https://github.com/vox-foundation/vox/blob/main/.github/workflows/ci.yml)) so
**new** snapshots auto-accept in CI without failing the build, and the resulting
`tests/snapshots/` directories are uploaded as the `insta-snapshots` artifact. **Changed**
snapshots still fail.

If your PR added or moved snapshots, baselines do not commit themselves. Either:

1. **Run the suite locally before merging** so the new `.snap` files appear in your working
   tree, then `git add` them alongside your code changes. (Preferred — keeps the PR
   self-contained.)
2. **After merge**, download the `insta-snapshots` artifact from the merged CI run, drop
   the new `.snap` files into `crates/<crate>/tests/snapshots/`, commit, and push as a
   follow-up.

Without one of the two, every later contributor sees the snapshot tests fail locally with
"snapshot assertion failed" on tests they didn't touch — exactly because the baseline only
ever existed in the CI artifact, not in the repo. Three separate cleanup commits in
2026-05 (`reactive_smoke_test`, `state_machine_integration_test`, `web_ir_lower_emit_test`)
were needed to drain orphans accumulated from this gap; do not let it recur.

The orphan signal is a `tests/snapshots/*.snap.new` file with no matching `.snap` next to
it. To clear: spot-check the `.snap.new` content (does the recorded output match what your
test should produce?), then accept via `cargo insta accept` (or rename `.snap.new` →
`.snap` if `cargo-insta` isn't installed). Run the suite again and repeat — each accepted
baseline can unblock previously-skipped tests that produce their own first-run snapshots.

## Planned additions (roadmap — Wave 7–9)

> **These are not yet shipped.** They describe the direction from
> [`vox_agentic_loop_and_mens_plan.md`](../archive/research-2026-q1/vox_agentic_loop_and_mens_plan.md).

**Scientia auto-ingest (Wave 7):** IDE sessions will be observed by
`ScientiaObserver`. Sessions that produce valid `.vox` with high
`worthiness_score` auto-ingest as training rows without manual corpus tooling.
Sessions that trigger multiple replans auto-ingest as negative examples.

**GRPO reward shaping (Wave 9):** Instead of SFT-only training, the model will
be scored on three signals per generated candidate:

- `r_syntax` — parse passes (0/1)
- `r_test` — `@test` block pass rate
- `r_coverage` — AST construct richness

Combined reward: `0.6×parse + 0.3×test + 0.1×coverage`. This makes test
coverage inside `.vox` files a first-class quality signal.

## Related

- [TOESTUB contributor guide](toestub-contributor-guide.md) — fix specific CI failures
- [Vox source → MENS pipeline SSOT](../archive/research-2026-q1/vox-source-to-mens-pipeline-ssot.md) — authoritative technical crosswalk
- [Mens native training SSOT](../reference/mens-training.md) — training pipeline reference
- [AI agent panic and shortcut pathology](../archive/research-2026-q1/research-ai-panic-shortcuts-2026.md) — why shortcuts harm the corpus
