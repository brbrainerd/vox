# Local-First CI: Queue Signal, Auto-Clear, and Agent Enforcement

> Design spec, brainstormed and approved 2026-07-02. Goal: make the local runner
> fleet the default CI plane for every harness call and every future agent,
> mechanically — not by instruction — while preventing job floods, self-healing
> queue backlogs, and emitting a machine-readable queue signal that LLM tool
> calls can read and act on.

## 0. Problem and contract

Agents routinely regress to GitHub-cloud behavior despite repeated instructions:
they push, then sit in `gh pr checks` / `gh run watch` loops waiting on remote
check runs. This is slow, queue-dependent, and floods the fleet with per-push
runs. Instructions alone have demonstrably failed; the fix must be mechanical.

**The approved verification contract:**

- Local gates green (`vox ci pre-push` tier) = the verdict. Push and move on.
- Fleet CI is an **async safety net**. Failures come back as a signal, never by
  an agent sitting in a watch loop.
- The only sanctioned queue interactions are `vox ci queue` (read) and
  `vox ci queue --clear` (clear).
- Auto-clear may cancel **superseded** runs (a newer run exists for the same
  workflow+branch) and **stale** queued runs (queued longer than a TTL,
  default 45 min). Main-branch, merge-group, and schedule/manual runs are
  always exempt.

Explicitly deferred (own passes later, nothing here blocks them):

- Local verdict ledger (`vox ci verdict <sha>` answering from local gate runs).
- Local orchestration plane (dispatching jobs to the fleet without Actions).
- Migrating the ~30 `ubuntu-latest` jobs to the fleet
  (`vox ci runner-policy-check --strict` is the existing flip for that).

## 1. Ground truth (verified against source, 2026-07-02)

- The fleet consumes GitHub's job queue by labels. The autoscaler
  `vox ci runner-scale` ([runner_scale.rs](../../../crates/vox-cli/src/commands/ci/runner_scale.rs))
  runs every ~2 min via Task Scheduler, reconciles ephemeral
  `vox-runner-auto-*` containers against queued-job demand, and already has all
  the `gh` plumbing this design needs: `gh_json`, `REPO_SLUG`
  (from `ci/constants.rs`), run/job queries, a scale-event JSONL ledger under
  `~/.vox/`, and a stale-lock pattern.
- 32 of 44 workflows already carry
  `concurrency: { group: workflow-ref, cancel-in-progress: true }` (ci.yml
  included). The 12 without are mostly schedule-triggered nightlies and
  release workflows, where cancel-in-progress is wrong.
- `vox ci runner-policy-check` (vox-cli-ci) already lints hosted `runs-on`
  labels against `docs/src/ci/github-hosted-exceptions.md` — advisory by
  default, `--strict` to fail. The new concurrency guard mirrors this pattern.
- `vox ci watch-run` exists and blocks up to 10 min polling GitHub check runs
  for HEAD. Its doc comment claims a post-push hook installs it; that is
  **stale** — `install_hooks.rs` does not reference it. It survives as a
  human diagnostic; the hook-guard blocks agents from calling it.
- `vox ci runner-status` prints fleet/container state and queue depth. It is
  fleet-centric; the new `queue` command is run-centric (classification +
  clearing). They share plumbing, not purpose.
- There is no checked-in project `.claude/settings.json` (only
  `settings.local.json`). The hooks in this design create it.

## 2. Component 1 — `vox ci queue`: the SSOT signal

New module `crates/vox-cli/src/commands/ci/queue.rs`, beside `runner_scale.rs`
(NOT vox-cli-ci: it needs the same `gh` plumbing runner_scale uses, which sits
on the vox-cli side of the contracts seam). New `CiCmd::Queue` variant with
flat flags per house style:

```text
vox ci queue                     # human table + advice line
vox ci queue --json              # full QueueSnapshot JSON
vox ci queue --brief             # one-paragraph summary (for context injection)
vox ci queue --from-snapshot     # read ~/.vox/ci-queue-snapshot.json, no network
vox ci queue --clear [--dry-run] # cancel superseded + stale (exempt-aware)
vox ci queue --ttl-mins <N>      # override stale TTL (default 45)
vox ci queue --hook-guard        # PreToolUse guard mode (see Component 4)
```

### Data model

```rust
// vox:skip (spec excerpt — lands in queue.rs)
pub enum RunClass { Active, Superseded, Stale }

pub struct QueueRun {
    pub id: u64,
    pub workflow: String,     // .name
    pub branch: String,       // .head_branch
    pub event: String,        // push | pull_request | merge_group | schedule | workflow_dispatch
    pub status: String,       // queued | in_progress
    pub age_secs: i64,
    pub class: RunClass,
    pub exempt: bool,
}

pub struct QueueSnapshot {
    pub generated_at: i64,
    pub degraded: bool,       // gh unreachable / partial data
    pub runs: Vec<QueueRun>,
    pub queued: u32,
    pub in_progress: u32,
    pub superseded: u32,
    pub stale: u32,
    pub fleet_alive: u32,     // from runner-scale's managed-container count
    pub fleet_max: u32,
    pub advice: String,       // THE machine-readable signal, always present
}
```

`advice` examples:

- `"queue healthy: 3 active ≤ capacity 4"`
- `"queued 14 > capacity 4: run 'vox ci queue --clear' (would cancel 9 superseded + 3 stale)"`
- `"degraded: gh unreachable; snapshot is 2026-07-02T14:31Z; do not retry-loop, proceed on local gates"`

### Classification (pure functions, unit-tested)

- `exempt` iff `branch == "main"` OR `event ∈ {merge_group, schedule, workflow_dispatch}`.
- `Superseded` iff a non-exempt run with the same `(workflow, branch)` has a
  strictly newer `created_at` (only the newest run per key survives).
- `Stale` iff `status == queued` AND `age_secs > ttl` AND not exempt.
- Exempt runs are always `Active` and never cancelled.

### Queries (reuse `gh_json` + `REPO_SLUG`)

One call per status — run-level fields only, **no per-run jobs fan-out**
(classification does not need job labels, unlike the autoscaler's demand count):

```text
gh api repos/{REPO_SLUG}/actions/runs?status=queued&per_page=100
  --jq '.workflow_runs[]|"\(.id)\t\(.name)\t\(.head_branch)\t\(.event)\t\(.created_at)\t\(.status)"'
```

(and `status=in_progress`). Cancel via
`gh api -X POST repos/{REPO_SLUG}/actions/runs/{id}/cancel`, best-effort per
run: one failed cancel logs and continues, never aborts the sweep.

### Snapshot file

Every networked `vox ci queue` invocation and every autoscaler tick writes the
full `QueueSnapshot` JSON to `~/.vox/ci-queue-snapshot.json` (same
home-dir convention as the scale ledger). `--from-snapshot` reads only this
file; if it is missing or older than 10 min the output says
`"queue snapshot unavailable/stale — run 'vox ci queue' for live state"`
and exits 0 (never blocks a session on it).

## 3. Component 2 — auto-clear on the autoscaler tick

At the top of `run_scale(apply)` in `runner_scale.rs`:

1. Build the snapshot (shared function from `queue.rs`).
2. If `apply`, cancel superseded + stale per the classification above.
3. Write the snapshot file.
4. Proceed with the existing demand/spawn/reap reconcile.

The existing 2-minute Task Scheduler tick thus self-heals backlogs — **no new
scheduler, no new script**. `scale_event_json` gains two fields:
`cleared_superseded` and `cleared_stale` (ledger stays append-only JSONL with
the existing rotation). On `gh` failure the clear is skipped for that tick and
the event logs `degraded:true`; the reconcile continues as today.

## 4. Component 3 — concurrency sweep + guard

**Sweep:** add the standard block to push/PR-triggered workflows among the 12
currently missing one; leave schedule-only nightlies and release workflows
alone:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

**Guard:** new `workflow-concurrency-guard` in vox-cli-ci (exact pattern of
`runner_policy_check.rs`): for every workflow under `.github/workflows/` whose
triggers include `push` or `pull_request`, require a top-level `concurrency:`
key. Exceptions live in `docs/src/ci/concurrency-exceptions.md` (one row per
workflow + reason), mirroring the hosted-runner exceptions doc. Advisory by
default, `--strict` in Tier-1 CI where `merge-group-fanout-guard` runs. This
makes flood-at-source protection unregressable.

## 5. Component 4 — hooks: the mechanical guarantee

New **checked-in** `.claude/settings.json` (project scope, applies to every
future agent session in this repo):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "vox ci queue --hook-guard" }]
      }
    ],
    "SessionStart": [
      {
        "hooks": [{ "type": "command", "command": "vox ci queue --brief --from-snapshot" }]
      }
    ]
  }
}
```

### `--hook-guard` (PreToolUse)

Reads the hook JSON from stdin, extracts `tool_input.command`, and matches —
**purely locally, no file IO beyond stdin, no network** (it runs on every Bash
call and must be fast):

- `gh pr checks`
- `gh run watch`
- `gh run view` … with `--watch`
- `gh api` … containing `check-runs` or `check_runs`
- `vox ci watch-run`

On match: exit code 2 with the replacement carried in stderr (fed back to the
model):

```text
Local-first CI: remote check-watching is disabled.
- Verdict: run local gates (`vox ci pre-push`); green = done, push and move on.
- Queue state: `vox ci queue --json` (advice field tells you what to do).
- Clear backlog: `vox ci queue --clear`.
```

No match: exit 0 silently. Parse failures of the hook JSON also exit 0 —
fail-open on infrastructure, fail-closed only on the banned patterns.

### SessionStart

`vox ci queue --brief --from-snapshot` prints ≤5 lines (queue depth, fleet
state, advice) from the snapshot file; the hook's stdout lands in the new
session's context, so every agent starts already knowing the queue and the
sanctioned commands. Zero network at session start; ≤2 min staleness.

## 6. Component 5 — the written contract

- **AGENTS.md** gains a short "CI verification contract" section stating the
  contract from §0 verbatim, plus: never target hosted runners for new jobs
  without a row in the exceptions doc.
- New `docs/src/ci/local-first-ci.md` (with required frontmatter) documenting
  the queue command, classification semantics, TTL, exemptions, the hooks, and
  the deferred roadmap (verdict ledger → orchestration plane → hosted-job
  migration). `docs/src/ci/runner-autoscaling.md` gets a cross-link.

## 7. Error handling

| Failure | Behavior |
| --- | --- |
| `gh` unreachable in `queue` | `degraded:true`, advice says proceed on local gates; exit 0 for `--brief`/`--from-snapshot`, exit 1 for `--clear` |
| `gh` unreachable in autoscaler clear | skip clear this tick, log `degraded`, reconcile continues |
| single `gh run cancel` fails | log, continue sweep, count in `advice` |
| snapshot file missing/stale | one-line "unavailable" message, exit 0 |
| hook JSON unparseable | exit 0 (fail-open) |
| race: run completes between snapshot and cancel | cancel returns 409/422 → treated as already-done, logged |

## 8. Testing

House style: pure logic as free functions with unit tests in-module (like
`count_matching_queued_jobs`).

- `classify_runs`: superseded/stale/exempt matrix incl. tie-break (equal
  `created_at`), main/merge_group/schedule exemptions, TTL boundary.
- `advice_for`: healthy / over-capacity / degraded phrasings.
- `hook_guard_matches`: every banned pattern, plus near-misses that must pass
  (`gh run view 123` without `--watch`, `gh pr checks` inside a quoted string
  is acceptable collateral — document as known coarse match).
- `workflow-concurrency-guard`: fixture workflows with/without triggers +
  exceptions parsing (mirror runner_policy_check tests).
- Snapshot round-trip: write → `--from-snapshot` renders identically; stale
  threshold honored.

## 9. Landing order

Each component is independently landable and useful alone:

1. `vox ci queue` (read-only) + snapshot file.
2. `--clear` + autoscaler-tick integration.
3. Hooks (`.claude/settings.json` + `--hook-guard` + SessionStart brief).
4. Concurrency sweep + `workflow-concurrency-guard`.
5. AGENTS.md + docs page.
