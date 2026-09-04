---
title: "Vox Efficacy Benchmark — Execution Handoff (2026-09-04)"
description: "Everything needed to run the first real Vox/MENS-vs-frontier-model benchmark on a new machine: exact commands, corpus ground truth, what claims the data can support, and the known-unverified edges."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# Vox Efficacy Benchmark — Execution Handoff

**Written for:** a Claude session on a different machine (macOS) that will run the
actual benchmark.
**Written on:** 2026-09-04, from a Windows session that built and reviewed the
harness but could not run the eval (reasons in §6).

> **One-sentence status:** the benchmark harness is built, reviewed, and correct;
> **nobody has ever run it against a real model**, and the single highest-value
> thing you can do is be the first.

## 1. Where the work is

| | |
|---|---|
| Branch | `fix-all-ci-failures` (pushed; `afad83017` at time of writing) |
| vs `origin/main` | 66 ahead, 1 behind — the 1 is `docs: add audited macOS AI development handoff`, already present on the branch as an identical-content commit, so the merge is clean (verified: 0 conflicts) |
| Command | `vox model eval-corpus` → [`crates/vox-cli/src/commands/model/eval_corpus.rs`](../../../crates/vox-cli/src/commands/model/eval_corpus.rs) |
| Scoring | [`crates/vox-eval/src/corpus_score.rs`](../../../crates/vox-eval/src/corpus_score.rs), [`corpus_stats.rs`](../../../crates/vox-eval/src/corpus_stats.rs) |
| Verifier | `vox_corpus::humaneval_runner::verify_program` |
| Corpus | `contracts/eval/humaneval-vox/` |

Read before running: the [adversarial audit](vox-efficacy-benchmark-adversarial-audit-2026-09-01.md)
(what the methodology gets right and what it still cannot claim) and the
[v2 plan](../../superpowers/plans/2026-09-01-vox-efficacy-benchmark-v2.md).

## 2. Corpus ground truth (verified 2026-09-04, not copied from prose)

Counted directly from `contracts/eval/humaneval-vox/manifest.v1.yaml`, scoped to
the `fixtures:` section only:

- **164 fixtures total**
- **31 held-out** (`training_eligible: false`) — this is the number that matters
- **133 training-eligible**

**Do not trust the manifest's prose header comments.** They still say
"164 problems: 120 training-eligible, 44 held-out" and "corpus complete
(164 problems, 44 held-out)". Those are stale. The machine-read field
`held_out_current: 31` is correct and carries its own note
(`regen-verified 2026-06-03; was 44, stale`). The code reads the per-fixture
`training_eligible` field, never the comments, so **the eval behaves correctly**;
only a human skimming the header gets misled. Worth a cleanup commit, not a blocker.

Also note: a naive `grep -c '^\s*- id:' manifest.v1.yaml` returns **174**, not 164 —
the extra 10 are in the `council_ratification_log` (6) and `provenance_kinds` (4)
sections. I briefly mis-flagged this as "9 fixtures missing required fields," which
would have been a blocking bug (`load_corpus` hard-errors via `?` on a fixture
missing `added_at` or `training_eligible` — it does not skip). It isn't one. If you
re-derive corpus counts, scope your count to the `fixtures:` section.

## 3. How to actually run it

Build first (`--release`; the eval shells out to the `vox` binary to compile and
run each candidate, so a debug binary makes a long sweep much slower):

```bash
cargo build --release -p vox-cli
```

### MENS (local checkpoint)

MENS models are discovered from `mens/runs/<name>/` directories that contain a
`final` or `checkpoint-*` subdirectory, and get the registry id `mens/<name>`
(see `MensCatalog::refresh` in `crates/vox-orchestrator/src/catalog.rs`). So:

```bash
vox model eval-corpus --model mens/<run-name> --harness vox-harness \
  --condition C1 --n 1 --k 1 --temperature 0.0 \
  --output target/eval/mens-c1.json
```

`--checkpoint <label>` folds into the row id as `<model>@<checkpoint>` so successive
MENS builds are separate leaderboard rows instead of silently overwriting each other.

### A frontier model (the actual comparison)

Any registry id routed through `vox_actor_runtime::llm` (OpenRouter etc.). MENS gets
no special-case code path — identical corpus, identical verifier, identical scoring:

```bash
vox model eval-corpus --model <registry-id> --harness vox-harness \
  --condition C1 --n 1 --k 1 --temperature 0.0 \
  --cutoff <model knowledge cutoff, ISO date> \
  --max-spend-usd <cap> \
  --output target/eval/<model>-c1.json
```

**Use `--cutoff`.** It restricts scoring to fixtures added strictly after the
model's training cutoff, which is the contamination defense. Prefer the OpenRouter
catalog's `knowledge_cutoff` over a hand-typed date. Without it you cannot claim the
model hadn't seen the problems.

**Use `--max-spend-usd`.** The run marks itself incomplete rather than silently
burning budget.

### An external harness (Claude Code, Cursor, Warp)

Their UIs can't be driven, so score their output instead — same verifier, which is
what makes it fair:

```bash
vox model eval-corpus --model <label> --harness claude-code \
  --from-dir path/to/solutions/   # <fixture-id>.vox per file
```

Token/latency/cost come back `None` for these rows, never a fabricated `0`.

### Sampling regime

`--n 1 --temperature 0.0` is the greedy headline number. For a pass@k estimate use
`--n 10 --k 1 --temperature 0.6+` (or higher n). All n samples are always drawn —
there is deliberately no early stop on first success, because the unbiased pass@k
estimator needs every outcome. Note `--k` must be ≤ `--n`.

## 4. What the results can and cannot support

This is the part most likely to be gotten wrong, so it is stated plainly.

**31 held-out fixtures gives roughly 9% power to detect a true 10-point gap between
two models.** The audit's own math says you need 122–208 problems for a defensible
ranking claim. Therefore, from a run on today's corpus:

- ✅ Defensible: failure taxonomies, MENS-vs-its-own-previous-checkpoint deltas,
  cost/latency at matched quality, "the harness runs end-to-end and here is what
  came out."
- ❌ Not defensible: "Vox/MENS beats <model> at writing Vox." A leaderboard ranking
  at n=31 is noise dressed as a result, and publishing one would be the single
  easiest way to discredit the whole program.

If a ranking claim is the actual goal, **growing the held-out corpus past ~122
problems is the prerequisite**, and it is a larger and more valuable piece of work
than running the current 31 faster.

Corpus coverage is also narrow: the audit measured ~17.3% of the language and 0 of
56 decorators exercised. Results describe a slice of Vox, not Vox.

## 5. What changed on this branch (relevant to you)

The harness was already substantially built. This session reviewed it and fixed
real defects, the first of which would have hit you immediately:

- **`corpus_pass_at_k` crashed the entire run when any fixture had zero recorded
  attempts** (`crates/vox-eval/src/corpus_score.rs`). If every attempt for one
  fixture hit an infra error — provider rate-limit, timeout, or a C3-condition
  prompt exceeding a model's context window — that fixture ended with `n=0`, and
  `pass_at_k(0,0,1)` tripped its own `assert!(n >= k)`, aborting the sweep and
  losing every already-computed result plus the API spend behind it. Zero-attempt
  fixtures are now excluded from the pass@k mean. **This is the fix most likely to
  have saved your first long run.**
- `GuardedConnection::query` data race → fixed, then the first fix introduced a
  self-deadlock (non-reentrant `tokio::sync::Mutex` vs this codebase's
  SELECT-then-INSERT-in-one-scope idiom), caught and re-fixed properly by draining
  rows under the guard. `vox-db`: 248/248 tests pass.
- `vox secrets set` no longer takes the token as a positional CLI argument
  (shell-history/`ps` leak); it reads `--stdin`, enforced by clap, not convention.
- Several CI-gate and clippy fixes (`turso-import-guard`, `large_enum_variant` on
  `EvalCorpusArgs`, `dead_code`, SSOT regenerations).

## 6. Known-unverified edges — read this before trusting "it's green"

**The full `vox ci pre-push --complete` gate never completed green end-to-end on
the Windows machine.** Each iteration got further and each failure was fixed
(clippy passed clean on the final attempts), but the last run died on
`no space on device`, not on a gate verdict. **Re-run it on macOS as your first
action** — it is the honest verification I could not finish:

```bash
vox ci pre-push --complete
```

Expect it to want SSOT regenerations (`doc-inventory.json` especially) after any
code edit; the gate names the exact regen command in its own error text, and those
regenerations are mechanical and safe to commit.

Windows-specific pain that **should not follow you to macOS** (noted so you don't
inherit workarounds you don't need):

- `rustc` OOM (`STATUS_STACK_BUFFER_OVERRUN`) linking large test binaries on a
  15.7 GB shared machine; `-j 2`/`-j 1` were the mitigation.
- `cargo fmt --all` overflows the Windows command-line limit — the repo mandates
  `vox run scripts/fmt.vox`. On macOS plain `cargo fmt` may be fine, but the repo
  policy still says use the script.
- Repeated shared-disk exhaustion (476 GB drive hitting 0 bytes free with several
  concurrent sessions building).

Not verified anywhere, by anyone, on any platform:

- `cargo test -p vox-cli` full-suite execution (the `eval_corpus` unit tests
  themselves do pass: 9/9).
- **The eval has never been run against a live model.** There is no leaderboard
  row, no scoreboard JSON, no output artifact in the repo. The only "result" in
  existence is a 5-fixture worked example in the research doc that used Claude
  Sonnet as a stand-in generator, not MENS.

## 7. Suggested first hour on macOS

1. `git checkout fix-all-ci-failures && git pull` (or work from `main` once merged).
2. `vox ci pre-push --complete` — close out the verification this handoff couldn't.
3. `cargo build --release -p vox-cli`.
4. One cheap smoke run: a frontier model, `--n 1 --temperature 0.0`, `--max-spend-usd 2`,
   `--output` set. Confirm a report JSON lands and a scoreboard row is written.
5. Only then scale up: add MENS, add conditions C0–C3, raise `--n`.
6. Report the numbers **with** the n=31 power caveat attached, every time.

## Related

- [Adversarial audit](vox-efficacy-benchmark-adversarial-audit-2026-09-01.md) — read first
- [Comparative efficacy research](vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md)
- [v2 implementation plan](../../superpowers/plans/2026-09-01-vox-efficacy-benchmark-v2.md)
- [Leaderboard plan](../../superpowers/plans/2026-09-01-vox-efficacy-benchmark-and-leaderboard.md)
