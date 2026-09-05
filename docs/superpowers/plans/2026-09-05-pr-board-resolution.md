# PR Board Resolution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> This is an operational/coordination plan, not a code-feature plan: there is no
> new production code to write. "Files" below means the PR's own diff; "tests"
> means the verification commands already run (or still to run) against it.
> Adapt literally where the templates don't fit — the discipline (bite-sized,
> exact commands, no placeholders) is what carries over.
>
> **Update 2026-09-05 (third pass): colima recovered, Task 1 Steps 1-5 done.**
> User authorized the restart after two independent sessions confirmed the same
> diagnosis (Apple Virtualization.framework crash at 00:05:00, guest dead for
> ~11h, hostagent alive but polling a corpse — `colima list`'s "Running" reads
> the hostagent pidfile, not guest liveness, so it was never a real signal).
> `colima stop -f` then `colima start` recovered cleanly; `docker ps` exits 0.
> Both runner containers restarted. Found and worked around a real bug in
> `scripts/ci-runner-local.sh` (P2P4-owned, not edited) while doing this: its
> GitHub-facing `RUNNER_NAME` is `hostname-$i` where `$i` is a loop-local
> counter starting at 1 in every script invocation, so two concurrent
> invocations always collide on their first registration. Worked around with
> an uncommitted `/tmp` copy carrying a distinct name offset for the second
> loop, rather than editing the tracked file. **Flag this to P2P4 as a real
> fix needed in the tracked script** — the workaround is session-local and
> won't survive a restart of this recovery.
> Confirmed via real evidence, not the `vox ci queue` tool's own `fleet_alive`
> field (which stayed stuck at 0 even with both runners genuinely online/busy
> per `gh api .../actions/runners` — that field is stale/unreliable right now,
> use `gh run list` for ground truth): `CI Health Dead-man` and `Cross-Platform
> Check` both completed `success` within a minute of the runners coming up.
> Task 1 Step 6 (the separate hosted-fallback break) is UNCHANGED and still
> open — colima recovering does not fix that independent problem.
>
> **Update 2026-09-05 (second pass):** the CI situation is worse than Task 1
> originally described. `main`'s hosted CI fallback (`ci-fallback-hosted.yml`)
> has failed on its own scheduled run for 3 consecutive days (Sep 3/4/5),
> independent of any PR — `DbConfig::resolve_canonical` is feature-gated behind
> `host-integration` in `crates/vox-db/src/config.rs` but referenced from
> generated fixture code in `vox-compiler`/`vox-codegen` tests without that
> feature enabled, plus several unrelated pre-existing test failures
> (`merge_group_fanout_guard`, `ci_workflow_contract`,
> `command_catalog_paths_baseline`, `db_migrate_semantics_test`,
> `vox-orchestrator::models::select`). Filed as a spawned task
> (`task_421d6c33`). **With the self-hosted fleet down (colima) AND this
> fallback broken, there is currently no working CI path for ANY PR on this
> repo, independent of anything in this plan.** Task 1 now has a Step 6 for
> this; do not treat "colima recovers" as sufficient to unblock the board —
> confirm the fallback lane is also green (or was never needed because the
> fleet recovered) before assuming CI can gate a merge.

**Goal:** land or definitively close every one of the 18 open PRs on
`vox-foundation/vox` with real evidence per PR, with `main` green throughout.

**Architecture:** One required branch-protection check (`Check, Build, and Test
(Rust)`) gates every PR. Two independent blockers currently sit in front of
that check: (a) a colima/docker outage that has taken both self-hosted CI
runners down, and (b) three dependabot PRs that would collide with an
in-flight workflow rewrite (P2P4) if merged first. Every other PR is already
individually verified and just waiting on the check to run at all.

**Tech Stack:** GitHub CLI (`gh`), git, cargo/pnpm/npm per-project builds,
`vox ci queue`, colima/docker.

**Spec:** none — this plan is derived from the actual state of the PR board as
scanned on 2026-09-05, not from a written design doc.

## Global Constraints

- **Never merge with only an exit code as evidence.** Every merge decision in
  this plan already has (or must acquire before merging) a real artifact:
  `test result: ok. N passed; 0 failed`, a build's final `Finished`/`built in`
  line, or a `shellcheck`/`sh -n` clean run. An exit code alone is not
  sufficient per this repo's own AGENTS.md rule.
- **Do not restart colima unilaterally.** It is shared infrastructure across
  at least 4 concurrent tabs. Get explicit sign-off in chat (from the user or
  the session that owns CI infra coordination) before running `colima
  restart`, even though the outage is now blocking every PR on the board.
- **Do not touch another tab's actively-owned worktree.** Reviews of another
  plan's PR (e.g. #501) happen in an isolated worktree/branch created for that
  purpose, never inside `crates/vox-gui/ui`-style shared checkouts or another
  session's named worktree.
- **Respect the P2P4 hold.** #453, #496, #497 touch `.github/workflows/` files
  P2P4's composite-action conversion (53 toolchain call sites →
  `.github/actions/setup-rust`) will rewrite. Do not re-arm their auto-merge
  until P2P4 has a PR open and it's confirmed those specific files are either
  already converted or P2P4 has explicitly rebased past them.

---

## Task 1: Confirm the CI infra blocker's current state

**PRs affected:** all 18 (nothing can merge while this is true).

**Interfaces:**
- Consumes: nothing.
- Produces: a fresh go/no-go signal for every other task in this plan. Every
  later task's "wait for CI" step depends on this being resolved first.

- [ ] **Step 1: Check whether the outage has self-resolved**

Run: `docker ps --format '{{.Names}}\t{{.Status}}'`
Expected if resolved: a list of container names (the two CI runner
containers), no error.
Expected if still down: `Cannot connect to the Docker daemon at
unix:///Users/brbrainerd/.colima/default/docker.sock`.

- [ ] **Step 2: If still down, confirm the queue is still stuck (not just slow)**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 vox ci queue --json | python3 -c "import json,sys;j=json.load(sys.stdin);print(j['queued'],j['in_progress'],j['fleet_alive'],j['fleet_max'])"`
Expected while stuck: `fleet_alive` is `0` and `queued` is not decreasing
across repeated checks a few minutes apart.

- [ ] **Step 3: If still down, get explicit sign-off before touching colima**

This is shared infrastructure. Post in chat (or to whichever session owns CI
coordination) naming the exact symptom (`colima list` shows "Running" but the
docker socket and `colima ssh` both fail) and ask for a go-ahead to run
`colima restart`. Do not run it without an explicit yes.

- [ ] **Step 4: Once sign-off is given, restart and verify**

Run: `colima restart`
Then: `docker ps` — must show no error.
Then re-launch both CI runner containers per this repo's documented runner
bring-up (`docs/src/ci/runner-autoscaling.md` or equivalent script this
repo uses) and confirm with:
`VOX_SKIP_FRESHNESS_CHECK=1 vox ci queue --json` — `fleet_alive` should climb
above 0 within a few minutes as jobs start completing.

- [ ] **Step 5: Do not commit anything for this task** — it's a runtime infra
      recovery step, not a code change.

- [ ] **Step 6: Separately confirm the hosted CI fallback itself is not also broken**

Run: `gh run list --branch main --workflow "ci-fallback-hosted.yml" --limit 3 --json conclusion,createdAt`
Expected if healthy: recent runs show `"conclusion":"success"`. As of
2026-09-05, three consecutive scheduled runs (Sep 3/4/5) show `"failure"` —
this is a real, standalone break on `main` (see the plan-header update above
and spawned task `task_421d6c33`), not a byproduct of the colima outage. If
still failing when this step runs, do not treat Task 1 as "done" just because
colima itself came back up — the self-hosted fleet recovering restores the
*primary* path, but confirm at least one working path (fleet OR fallback)
before assuming any PR's required check can go green.

---

## Task 2: Confirm #501's review fixes and flagged findings are acknowledged

**PRs affected:** #501.

**Interfaces:**
- Consumes: the review already completed (commit `9026ae785` on
  `claude/plan-p3p6-broker-payload-33cc68`, PR comment posted with the full
  finding list).
- Produces: a merge/hold decision for #501 that the owning tab (P3P6) has
  actually seen and responded to, not one made unilaterally on their branch.

- [ ] **Step 1: Check for a reply from the P3P6 session**

The review findings and the four fixes already pushed were sent to
`local_34ead9ab-d6cf-47ce-a871-9b05c911400d` (session title "Plans P3 and P6:
broker and payload"). Check whether that session has responded — either in
chat, or by inspecting the PR for new commits:

Run: `gh pr view 501 --json commits --jq '.commits[-3:][].messageHeadline'`
Expected: see whether a commit landed after `9026ae78543ce5993294a378cfbfb12db18cdef4`
addressing any of the 6 flagged-not-fixed findings (VOX_HOME wiring in
`vox-build-queue`, the two new `.sh` scripts against VoxScript-First, the
`build_broker.rs` duplication, the narrowed drift-check receiver pattern, the
dropped non-git-cwd fallback).

- [ ] **Step 2: If P3P6 has addressed the flagged findings, re-verify the delta**

For any new commit, re-run the specific test/build command that finding's
area covers (e.g. if they wire `VOX_HOME` into `vox-build-queue`, re-run
`cargo test -p vox-build-queue --lib` and grep for `test result: ok`). Do not
assume a "fixed" claim in a commit message is itself evidence.

- [ ] **Step 3: If P3P6 has not yet responded, do not merge #501 unilaterally**

This PR was never armed for auto-merge by this plan's author, deliberately —
it's another tab's design surface, not a mechanical dependency bump. Leave it
as-is (`gh pr view 501` should show no `autoMergeRequest`) until the owning
tab either merges it themselves or explicitly asks this session to.

- [ ] **Step 4: Once CI can run (Task 1 done) and P3P6 has either fixed or
      accepted the flagged findings, verify the branch protection check itself**

Run: `gh pr checks 501 | grep "Check, Build, and Test"`
Expected: `pass`, not `pending`/`fail`.

- [ ] **No commit for this task** — it's a review/coordination checkpoint, not
      a code change.

---

## Task 3: Land the 14 pre-verified dependabot/hygiene PRs once CI recovers

**PRs affected:** #461, #466, #479, #481, #483, #484, #486, #487, #488, #489,
#495, #498, #499, #500.

**Interfaces:**
- Consumes: each of these 14 already has `autoMergeRequest.mergeMethod ==
  "MERGE"` armed from prior verification (see the individual build/test
  evidence already recorded against each — not repeated here since it does
  not change; this task is purely "wait for the mechanism to fire and confirm
  it fired cleanly").
- Produces: `main` advancing by 14 additional commits, each independently
  bisectable.

- [ ] **Step 1: Confirm auto-merge is still armed on all 14**

Run:
```bash
for pr in 461 466 479 481 483 484 486 487 488 489 495 498 499 500; do
  echo "#$pr: $(gh pr view $pr --json autoMergeRequest --jq '.autoMergeRequest.mergeMethod // "NOT ARMED"')"
done
```
Expected: every line ends `MERGE`. If any say `NOT ARMED` (e.g. dependabot
rebased the PR and GitHub cleared the auto-merge flag), re-arm with
`gh pr merge <n> --auto` — do not re-verify the build first, since nothing
about the PR's own diff changed; only re-verify if `gh pr diff <n>` shows new
content since the last check.

- [ ] **Step 2: Wait for CI to actually run each one**

Once Task 1 is done (runners alive), these will enter the queue and get
processed. Do not poll continuously — check back every 10-15 minutes with:
`gh pr list --state open --json number,mergeStateStatus --jq '.[] | select(.mergeStateStatus != "BLOCKED")'`

- [ ] **Step 3: After each merges, confirm it actually landed clean**

Run: `gh pr view <n> --json state,mergedAt --jq '"\(.state) \(.mergedAt)"'`
Expected: `MERGED` with a real timestamp, for every PR in this task's list.

- [ ] **Step 4: If any of the 14 fails its required check for a reason
      unrelated to the outage**, stop and diagnose before letting it merge —
      do not assume "it was fine when I checked it" still holds if `main`
      moved underneath it. Re-run the PR's own build/test command against
      the PR's current head, same as the original verification, and report
      the real result before deciding to keep or disarm auto-merge.

- [ ] **No commit for this task** — GitHub performs the merges once the check
      passes; there is nothing to hand-commit.

---

## Task 4: Track P2P4 and re-arm the three held PRs once safe

**PRs affected:** #453, #496, #497.

**Interfaces:**
- Consumes: P2P4's composite-action conversion landing (a PR from
  `claude/plan-p2p4-toolchain-ci-0e5cdb` or its successor branch, converting
  `.github/workflows/*.yml` toolchain setup to `.github/actions/setup-rust`).
- Produces: three PRs safely re-armed, or safely superseded by dependabot's
  own rebase-driven regeneration.

- [ ] **Step 1: Check whether P2P4 has opened a PR yet**

Run: `gh pr list --state all --search "head:claude/plan-p2p4" --json number,title,state,headRefName`
Expected while still unlanded: no result, or a result still `OPEN` with
`mergedAt: null`.

- [ ] **Step 2: If P2P4's PR is open but not yet merged, do nothing further
      this cycle** — re-check in a later pass. Do not re-arm #453/#496/#497
      while P2P4 is still rebasing; the collision risk (53 site rewrites vs.
      three dependabot version bumps on the same lines) is exactly why they
      were held in the first place.

- [ ] **Step 3: Once P2P4 has merged into `main`, check whether #453/#496/#497
      still apply cleanly**

Run for each:
```bash
gh pr view 453 --json mergeable,mergeStateStatus
gh pr view 496 --json mergeable,mergeStateStatus
gh pr view 497 --json mergeable,mergeStateStatus
```
Expected outcomes, per PR:
- `CONFLICTING`: the composite-action conversion touched the same lines this
  bump touched. Comment `@dependabot rebase` on that PR and wait — do not
  hand-resolve the conflict, since dependabot's own resolver produces a
  correct, minimal diff against the new file shape.
- `MERGEABLE`: the conversion didn't touch the exact lines this PR edits (or
  already includes an equivalent version bump). Proceed to Step 4.

- [ ] **Step 4: Re-verify each PR against the new `main` before re-arming**

These are pure `uses: <action>@vN` version-string edits in workflow YAML —
there is no local build to run. Verify by reading the diff and confirming it
still targets a real, unconverted `uses:` line:

Run: `gh pr diff <n> | grep -E '^[-+]\s+uses:'`
Expected: the diff still shows a version bump on an actual line in the file
(not a stale line the conversion already deleted).

- [ ] **Step 5: Re-arm**

Run: `gh pr merge <n> --auto` for each PR that passed Step 4.

- [ ] **No commit for this task** — same as Task 3, GitHub performs the merge.

---

## Task 5: Final board sweep

**PRs affected:** all 18 (verification that none were missed or silently
stuck).

**Interfaces:**
- Consumes: Tasks 1-4 complete.
- Produces: a written confirmation (to the user, in chat) that the board is
  at zero open PRs, or a named, justified reason for each one still open.

- [ ] **Step 1: Full board scan**

Run: `gh pr list --state open --limit 100 --json number,title,mergeStateStatus`
Expected: empty array, or a short list where every remaining PR has a named
reason (e.g. "#501 still awaiting P3P6's response to the flagged findings").

- [ ] **Step 2: Confirm `main`'s required check is green on its own tip**

Run: `gh pr list --state open --limit 1` to get any recent merge commit, or
check the latest push to `main` directly:
`gh api repos/vox-foundation/vox/commits/main/check-runs --jq '.check_runs[] | select(.name == "Check, Build, and Test (Rust)") | .conclusion'`
Expected: `success`.

- [ ] **Step 3: Report to the user** — real counts (how many merged, how many
      closed with findings, how many still open and why), not a summary that
      elides the colima outage or the #501 handoff status.

- [ ] **No commit for this task.**

---

## Self-Review

**Spec coverage:** every one of the 18 PRs scanned at plan-writing time
appears in exactly one task (Task 2 for #501, Task 3 for the 14 pre-verified
ones, Task 4 for the 3 held ones). Task 1 and Task 5 are cross-cutting
(infra recovery, final sweep) and don't duplicate PR-specific work.

**Placeholder scan:** no TBD/TODO. Every step names an exact `gh`/`git`/
`docker`/`colima` command and its expected output shape. Task 4's Step 2 says
"do nothing further this cycle" deliberately — that's a real instruction (wait
and re-check later), not a placeholder for undefined work.

**Type/name consistency:** PR numbers, branch names, and session IDs quoted
here were verified against a live `gh pr list` scan at plan-writing time
(2026-09-05); re-verify PR numbers haven't shifted (a PR merging or a new one
opening) before executing a task that assumes a specific number still applies.

---

## Addendum (merge-coordination session, 2026-09-05)

Preserved into git from an untracked file in the `main` worktree, where it
would have been lost to any `git clean`. Two items above are now resolved:

- **The `ci-runner-local.sh` `RUNNER_NAME` collision is FIXED in the tracked
  script**, not just worked around. `RUNNER_NAME` now carries `$$` alongside
  the worker and iteration counters, exactly as `container_name()` already
  did. Landed in `e1c7b7fa8` via PR #502. The session-local `/tmp` copy with
  the name offset is no longer needed.

- **The required-context hole this board was working around is closed.**
  `ci-fallback-hosted.yml` named its `gate` job with the sole required
  branch-protection context *and* fired on `synchronize`, so on every push it
  skipped and posted `conclusion=skipped` — which GitHub counts as satisfying
  the requirement. Measured on PR #502: started and completed at `18:34:38Z`,
  the same second, while `ci.yml`'s `setup` was still queued and nothing had
  compiled; the PR read `mergeable=MERGEABLE`. The trigger is now
  `types: [labeled]`, and `vox ci required-context-guard` fails if any
  workflow but `ci.yml` claims that name from an ordinary PR event.
