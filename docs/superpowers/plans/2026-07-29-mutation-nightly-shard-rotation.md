# Mutation-Nightly Shard Rotation Implementation Plan

> **For agentic workers:** Execute inline in the main session (no subagent dispatch — this environment's subagents are read-only in the worktree sandbox and cannot commit). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `mutation-nightly.yml` cover all 4453 `vox-compiler` mutants over a rolling cycle instead of only the first ~40-per-run before hitting the 120-minute job timeout, with zero new runner capacity.

**Architecture:** Use cargo-mutants' native `--shard k/n` flag with `n=128` (measured: `--list --shard i/128` yields ~35 mutants per shard, comfortably inside the observed ~40-mutants-per-120min throughput). The shard index `k` is derived deterministically each run from day-of-year mod 128 (`date -u +%j`), so no persisted state is needed and the schedule naturally rotates through the whole corpus roughly every 128 days, looping forever. No workflow_dispatch behavior change beyond also honoring the day-based shard (manual runs use "today's" shard, same as the scheduled run would).

**Tech Stack:** GitHub Actions workflow YAML, `cargo-mutants` CLI (`--shard`, already installed by the workflow), `date` (already available in the `bash` step shell).

---

### Task 1: Rotating shard index in mutation-nightly.yml

**Files:**
- Modify: `.github/workflows/mutation-nightly.yml:50-69` (the "Run mutants (package vox-compiler)" step)

- [ ] **Step 1: Confirm shard math still holds**

Run: `cargo mutants -p vox-compiler --list --shard 0/128 2>&1 | wc -l`
Expected: a number in the 30-45 range (measured 35 on 2026-07-28; re-check here since the mutant count drifts as the crate changes — if it's grown past ~55/shard, bump `128` up proportionally in Step 2 so each shard still finishes well inside the 120m budget).

- [ ] **Step 2: Replace the run step with the shard-rotation version**

Replace the existing step body (everything under `- name: Run mutants (package vox-compiler)`, from the comment block through the final `run:` block) with:

```yaml
      - name: Run mutants (package vox-compiler)
        # Full-crate mutation testing (4453 mutants as of 2026-07-28) does not
        # fit in a single 120m nightly run at observed throughput (~40
        # mutants/120m at --jobs 4; confirmed --jobs 12 is WORSE -- 25/4453 --
        # because each mutant job runs its own fully-parallel rustc build and
        # 12 concurrent jobs oversubscribe the 24-core runner). Rather than
        # fight per-mutant throughput, shard the corpus with cargo-mutants'
        # native --shard k/n and rotate k by day-of-year, so the schedule
        # cycles through every mutant over ~128 days with zero new runner
        # capacity. n=128 measured ~35 mutants/shard (2026-07-28) -- comfortably
        # inside the 120m budget. Re-measure `--list --shard 0/128 | wc -l`
        # occasionally as the crate grows and bump 128 up if a shard creeps
        # past ~50 mutants.
        #
        # emission_ladder_test shells out to rustc to compile generated code
        # per case (~293s for the whole file locally) -- fine once in normal
        # `cargo test`, but cargo-mutants re-runs the test command for every
        # candidate mutant, so it's excluded from the mutants inner loop only;
        # the real `cargo test` gate still runs it every time.
        run: |
          SHARD_TOTAL=128
          SHARD_INDEX=$(( 10#$(date -u +%j) % SHARD_TOTAL ))
          echo "Mutation shard: ${SHARD_INDEX}/${SHARD_TOTAL} (day-of-year $(date -u +%j))" | tee -a "$GITHUB_STEP_SUMMARY"
          cargo mutants -p vox-compiler --no-times --jobs 4 \
            --shard "${SHARD_INDEX}/${SHARD_TOTAL}" \
            -- -E 'not binary(emission_ladder_test)'
```

Note the `10#` prefix on `$(date -u +%j)` in the arithmetic expansion: `date -u +%j` zero-pads to 3 digits (e.g. `007`), and bash arithmetic treats a leading-zero number as octal, which breaks for day-of-year values `008` and `009` (not valid octal digits). `10#` forces base-10 interpretation.

- [ ] **Step 3: Verify the day-of-year arithmetic locally**

Run (on the actual runner, since this is bash-specific arithmetic — verify via the same docker-wsl runner container used earlier in this session, or any bash 4+ shell):

```bash
bash -c 'SHARD_TOTAL=128; for d in 001 007 008 009 059 365 366; do echo "day=$d -> shard=$(( 10#$d % SHARD_TOTAL ))"; done'
```

Expected: 7 lines, one per test day, each printing a `shard=` value in `[0, 127]` with no arithmetic syntax errors (this is the case that would break without the `10#` prefix — days `008` and `009` would error with "value too great for base (error token is \"008\")" or similar).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/mutation-nightly.yml docs/superpowers/plans/2026-07-29-mutation-nightly-shard-rotation.md
git commit -m "feat(ci): rotate mutation-nightly through all mutants via day-of-year sharding

Full-crate mutation testing doesn't fit a single 120m nightly run at
observed throughput (~40/4453 mutants per run, confirmed job-count
tuning doesn't help -- see prior commits same day). Uses cargo-mutants'
native --shard k/n with n=128 (~35 mutants/shard measured) and k
derived from day-of-year, so the corpus cycles fully roughly every 128
days with zero new runner capacity."
```

- [ ] **Step 5: Push and verify with a real dispatched run**

```bash
git push origin main
```

Then trigger `gh workflow run mutation-nightly.yml --ref main`, confirm the dispatched run's `headSha` matches the pushed commit (`gh run list --workflow=mutation-nightly.yml --limit 1 --json headSha`), and watch it through to actual completion (not just queued/in_progress) before considering this done. Expected: the run's `Run mutants` step logs a `Mutation shard: N/128 (day-of-year ...)` line near the top, and the job completes (not cancelled by timeout) — since ~35 mutants/shard is well under the ~40/120m rate that was already measured to survive a full 120m window.

---

### Self-review notes (already applied above, recorded for traceability)

- **Spec coverage:** day-of-year rotation ✓ (Step 2), shard size validated against current mutant count ✓ (Step 1), no new runner capacity required ✓ (single job, unchanged `runs-on`/fleet), verified via a real run not just a local dry-run ✓ (Step 5).
- **Placeholder scan:** none — every step has literal commands/YAML, no "add appropriate X."
- **Known trap already fixed inline:** octal misinterpretation of zero-padded `date +%j` output in bash arithmetic (`10#` prefix) — this would have silently broken on the 2nd, 8th, and 9th day of most months (and days 100-999 range is fine, but 001-099 needed the fix). Caught before Step 2 was finalized, not left for review to find.
