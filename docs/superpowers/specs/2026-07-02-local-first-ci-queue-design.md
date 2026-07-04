# Local-First CI: Queue Signal, Auto-Clear, and Agent Enforcement

> Design spec, brainstormed 2026-07-02; **rev 2** after a 4-stream adversarial
> audit against the codebase and the live GitHub API (classification
> correctness, hook false-positives/negatives + hooks-schema verification,
> failure-signal honesty, SSOT registration). Goal: make the local runner
> fleet the default CI plane for every harness call and every future agent,
> mechanically — not by instruction — while preventing job floods, self-healing
> queue backlogs, and emitting a machine-readable queue **and failure** signal
> that LLM tool calls can read and act on.

## 0. Problem and contract

Agents routinely regress to GitHub-cloud behavior despite repeated instructions:
they push, then sit in `gh pr checks` / `gh run watch` loops waiting on remote
check runs. This is slow, queue-dependent, and floods the fleet with per-push
runs. Instructions alone have demonstrably failed; the fix must be mechanical.

**The verification contract (audited wording — the tiers matter):**

- **Local gates green = the verdict for what they cover.** Run
  `vox ci pre-push --complete` (or `--full` when code/tests changed): fmt,
  hygiene/SSOT guards, docs, workspace clippy, and the nextest partition.
  Green there = push and move on — never wait on remote checks. (The default
  fast tier omits clippy and all tests; do not treat fast-tier green as the
  verdict for code changes.)
- **Fleet CI is authoritative for the rest** — rustdoc, deny/audit, compiler
  gates, integration/docker/browser/GUI smokes, coverage and architecture
  budgets, all-features/mutation/cross-platform/mobile lanes — none of which
  has a local equivalent. Their verdicts arrive **asynchronously as the
  failure signal** (§2a: the queue snapshot's `failures` field, surfaced at
  SessionStart and by `vox ci queue`). A red there is new information to fix
  locally — never a reason to re-push and watch.
- The only sanctioned queue interactions are `vox ci queue` (read) and
  `vox ci queue --clear` (clear). Remote check reading — one-shot included —
  is blocked for agent sessions; the snapshot's `failures` field is the
  sanctioned channel. (`gh run list` and `gh run view <id> --log-failed`
  remain permitted as the manual escape hatch for reading a specific
  failure's logs.)
- Auto-clear may cancel **superseded** and **stale** runs per the audited
  rules in §2 — with exemptions and gates that make wrong cancellations
  structurally hard (see §2, the audit moved most of the risk here).

Explicitly deferred (own passes later, nothing here blocks them):

- Local verdict ledger (`vox ci verdict <sha>` answering from local gate runs).
- Local orchestration plane (dispatching jobs to the fleet without Actions).
- Migrating the remaining hosted jobs (`vox ci runner-policy-check --strict`
  is the existing flip for that).
- Force-cancel escalation is IN scope (§3) since the snapshot already persists
  the state it needs.

## 1. Ground truth (verified against source and live API, 2026-07-02)

- The fleet consumes GitHub's job queue by labels. The autoscaler
  `vox ci runner-scale` ([runner_scale.rs](../../../crates/vox-cli/src/commands/ci/runner_scale.rs))
  runs every ~2 min via Task Scheduler and already has the `gh` plumbing this
  design needs (`gh_json`, `REPO_SLUG`, `--paginate` precedent at
  `runner_rows()`, scale-event JSONL ledger under `~/.vox/`, two-tick zombie
  pattern `zombies_for_force_cancel`).
- 32 of 44 workflows already carry `concurrency` groups. Among the 12 without:
  only `ci-health-watchdog-test.yml` is PR-triggered; `release-*` are tag-push
  and `scorecard` is main-push (exception rows, §4).
- **Live API facts that shaped §2** (from the audit): merge-group runs have
  `head_branch = "gh-readonly-queue/main/pr-N-<sha>"` (unique per entry);
  tag-push release runs have `event=push, head_branch="v0.6.0"` (→ NOT
  main-exempt); `status=pending` runs exist here right now (concurrency-group
  blocked); re-runs keep `id` and `created_at`, bumping only `run_attempt` and
  `run_started_at`; fork PRs expose the fork's bare branch name in
  `head_branch` (collision-prone); the runs API also serves `waiting`
  (deployment approval) and an undocumented `dynamic` event (CodeQL);
  `fromdateiso8601` works in gh's jq (proven in `ci-timings.yml:44`);
  cancel-on-completed returns 409; `/force-cancel` exists for shielded runs.
- `vox ci watch-run` exists and blocks up to 10 min polling check runs. Its
  "post-push hook installs it" doc comment is **stale** — nothing installs it.
  It survives as a human diagnostic; the hook-guard blocks agents calling it.
- `ci-fallback-hosted.yml`: the `gate` job is properly gated on the
  `fleet-down` label, but `gui-windows-build-smoke` has **no `if:` at all** —
  it runs a full Tauri Windows build on `windows-latest` on **every PR
  synchronize**, contradicting both its own header and its exceptions-doc row.
  §6 tightens it.
- There is **no failure signal today**: the watchdog/deadman alert a human
  push channel (`CI_HEALTH_PUSH_URL`) about *fleet health*, never run
  conclusions; nothing carries "your branch's run failed" to any agent
  session. §2a closes this.
- Hooks facts confirmed against the official docs: PreToolUse stdin JSON has
  `tool_input.command`; exit 2 blocks with stderr fed to the model; other
  non-zero exits are non-blocking; SessionStart stdout is injected as context;
  project `.claude/settings.json` is committable and applies repo-wide.
  **Measured:** warm `vox` spawn ≈ 40 ms (fine); a **stale `vox.exe` on PATH
  exits 2 on the unknown subcommand — clap's usage-error code collides with
  the hook block code** (§5 mitigations). This harness also exposes a
  `PowerShell` tool whose input is likewise `command` — the matcher must
  cover both.
- SSOT registration: no guard hard-fails on a new `vox ci` subcommand (the
  operations-catalog orphan check iterates `command-registry.yaml`, not clap;
  `runner-scale`/`watch-run` are live-but-unregistered precedents). But
  registration in `contracts/operations/catalog.v1.yaml` is the AI-first
  discoverability surface (CLI reference + MCP planner metadata), so this
  design **requires** it (§7).

## 2. Component 1 — `vox ci queue`: the SSOT signal

New module `crates/vox-cli/src/commands/ci/queue.rs`, beside `runner_scale.rs`
(it shares that file's `gh` plumbing, which sits on the vox-cli side of the
contracts seam). Flat flags per house style:

```text
vox ci queue                     # human table (+ FAILED section) + advice line
vox ci queue --json              # full QueueSnapshot JSON
vox ci queue --brief             # ≤6-line summary (SessionStart injection)
vox ci queue --from-snapshot     # read ~/.vox/ci-queue-snapshot.json, no network
vox ci queue --clear [--dry-run] # cancel per §2 rules (live data only)
vox ci queue --ttl-mins <N>      # override stale TTL (default 45)
vox ci queue --hook-guard        # PreToolUse guard mode (§5)
```

`--clear` with `--from-snapshot` is a **hard error**: cancellation decisions
must never run against data up to 10 minutes old.

### Data model

```rust
// vox:skip (spec excerpt — lands in queue.rs)
pub enum RunClass { Active, Superseded, Stale }

pub struct QueueRun {
    pub id: u64,
    pub workflow: String,     // .path (".github/workflows/x.yml") — .name can contain tabs/collide
    pub branch: String,       // .head_branch ("null" when API null — never supersedable)
    pub repo: String,         // .head_repository.full_name — fork disambiguation
    pub event: String,
    pub status: String,       // queued | in_progress | pending | waiting
    pub run_attempt: u32,
    pub started_epoch: i64,   // run_started_at // created_at (re-runs reset it)
    pub age_secs: i64,
    pub class: RunClass,
    pub exempt: bool,
}

pub struct FailedRun {
    pub id: u64,
    pub workflow: String,     // .path
    pub branch: String,
    pub conclusion: String,   // failure | timed_out | startup_failure (cancelled EXCLUDED:
                              // auto-clear's own cancellations must not echo back as failures)
    pub head_sha: String,
    pub completed_epoch: i64,
    pub url: String,          // html_url
}

pub struct QueueSnapshot {
    pub generated_at: i64,
    pub degraded: bool,
    pub queued: u32,          // queued + pending (both are "not yet running")
    pub in_progress: u32,
    pub superseded: u32,
    pub stale: u32,
    pub fleet_alive: u32,
    pub fleet_max: u32,
    pub advice: String,       // THE machine-readable signal, always present
    pub failures: Vec<FailedRun>,   // §2a: last 24h, newest-first, cap 20, main included
    pub cancelled_last_sweep: Vec<u64>, // §3 force-cancel escalation state
    pub runs: Vec<QueueRun>,
}
```

### Exemption (audited — fail-open by construction)

A run is **exempt** (never cancelled, always `Active`) unless ALL of:

1. `event ∈ {push, pull_request}` — an **allowlist of cancellable events**.
   Everything else (`merge_group`, `schedule`, `workflow_dispatch`,
   `workflow_run`, `dynamic`, and any event GitHub adds later) is exempt by
   default. Unknown events fail open, not closed.
2. `branch != "main"` and `branch != "null"`.
3. NOT a tag push: `event == push && branch matches ^v[0-9]` is exempt
   (release workflows trigger on `tags: v*`; their runs report the tag as
   `head_branch` and would otherwise be cancellable — audit finding, live-
   verified against `release-binaries` runs).
4. `run_attempt == 1` — a re-run is an explicit human request; never
   supersede or stale-cancel it.
5. `status != "waiting"` — that is a human deployment-approval gate.

### Classification (pure functions, unit-tested)

- **Superseded**: a strictly newer (`started_epoch`, strict) non-exempt run
  exists with the same **(workflow-path, repo, branch, event)** key. The
  event is in the key: workflows triggering on both `push` and
  `pull_request` (e.g. `mobile-eas-build.yml`, `vox-mental-tracker.yml`)
  spawn same-commit siblings that must not cancel each other. The repo is in
  the key: two forks named `patch-1` must not collide. Ties (equal
  `started_epoch`) keep both.
- **Stale**: `status ∈ {queued, pending}` AND `age_secs > ttl` (default
  45 min, from `started_epoch`) — **AND `fleet_alive > 0`**. When the fleet
  is at zero runners, a deep queue is an outage, not abandonment; cancelling
  it would both destroy the async safety net and reset the health watchdog's
  `queue_age` signal so the hosted failover never trips (audit findings 1 and
  F4). The stale sweep is disabled for that tick.
- Cancellation applies to `status ∈ {queued, in_progress, pending}` only.
- At most **50 cancellations per sweep** (bounds the blast radius of any
  future classification bug and the POST burst); the remainder is logged and
  the next tick continues.

### Queries (reuse `gh_json` + `REPO_SLUG`)

One paginated call per status — `queued`, `in_progress`, `pending`,
`waiting` — run-level fields only, no per-run jobs fan-out. `--paginate`
capped at 5 pages per status (the flood case is exactly when >100 runs
exist; newest-first ordering means un-paginated truncation hides precisely
the superseded/stale tail):

```text
gh api repos/{REPO_SLUG}/actions/runs?status=queued&per_page=100 --paginate
  --jq '.workflow_runs[]|"\(.id)\t\(.path)\t\(.head_branch)\t\(.head_repository.full_name)\t\(.event)\t\((.run_started_at // .created_at)|fromdateiso8601)\t\(.status)\t\(.run_attempt)"'
```

(`fromdateiso8601` is proven in this repo — `ci-timings.yml:44` uses it inside
`gh api --jq` in production.)

Cancel via `gh api -X POST repos/{REPO_SLUG}/actions/runs/{id}/cancel`,
best-effort per run: 409 = already completed = the race resolved itself; log
and continue, never abort the sweep.

### 2a. The failure signal (closes the contract's promise)

One additional fetch per snapshot:
`status=completed&per_page=50` (single page), jq-filtered to
`conclusion ∈ {failure, timed_out, startup_failure}`, kept if completed
within 24 h, cap 20, **including main** (a red main matters more, not less).
`cancelled` is excluded so auto-clear's own work never echoes back as failure.

Surfacing:

- `--brief` resolves the current branch locally (`git rev-parse
  --abbrev-ref HEAD`; on failure, skip the branch filter) and emits, when
  applicable (budget grows to ≤6 lines):

  ```text
  FAILED on <branch>: <workflow> #<id> (<conclusion>, <age>m ago) -> <url>
  FAILED on main: <workflow> #<id> (<conclusion>) -> <url>
  ```

- `advice` **leads with the failure** when the current branch has one:
  `"CI FAILED for this branch (run <id>): read <url> or 'gh run view <id>
  --log-failed', fix locally, re-run local gates — do not push blind retries"`.
- The human table gets a `FAILED (24h)` section under the queue rows.

This is the mechanism behind the contract's "failures come back as a signal";
without it the hook-guard would leave agents with no sanctioned way to learn a
conclusion at all (audit findings F1/F2).

### Snapshot file

Every networked `vox ci queue` invocation and every autoscaler tick writes the
full `QueueSnapshot` to `~/.vox/ci-queue-snapshot.json` **atomically** (temp
file + rename — parallel agent sessions and the tick race on it).
`--from-snapshot` reads only this file; staleness in steady state is ≤2 min
(the tick), hard cap 10 min — older snapshots yield
`"queue snapshot unavailable/stale — run 'vox ci queue' for live state"`,
exit 0 (never blocks a session).

### `advice` examples

- `"queue healthy: 3 active ≤ capacity 4"`
- `"queued 14 vs capacity 4: run 'vox ci queue --clear' (would cancel 9 superseded + 3 stale)"`
- `"queue backlog: 9 active > capacity 4 with fleet at 0 — outage, not backlog; stale sweep disabled; check 'vox ci runner-status'"`
- `"CI FAILED for this branch (run 123): read <url>, fix locally — do not push blind retries"`
- `"degraded: gh unreachable; do not retry-loop — proceed on local gates and try later"`

## 3. Component 2 — auto-clear on the autoscaler tick

At the top of `run_scale(apply)` in `runner_scale.rs`, after the lock:

1. Build the snapshot (shared function from `queue.rs`).
2. If `apply`, cancel per §2 (superseded always; stale only when
   `fleet_alive > 0`; 50-cancel cap).
3. **Force-cancel escalation**: a run that appears in the *previous*
   snapshot's `cancelled_last_sweep` and is still `in_progress` now is
   shielded (e.g. `always()` post steps); escalate to
   `POST /actions/runs/{id}/force-cancel`. (Same two-tick pattern as
   `zombies_for_force_cancel`.)
4. Write the snapshot file (atomically), recording this sweep's cancelled ids.
5. Proceed with the existing demand/spawn/reap reconcile — clearing first
   means cancelled runs never count as demand.

No new scheduler, no new script. `scale_event_json` gains
`cleared_superseded` and `cleared_stale` — **actual cancellations only**
(0 on dry-run; dry-run logs clearable counts to stdout instead, so the ledger
never reports work it didn't do). On `gh` failure the clear is skipped for the
tick, the event logs `degraded:true`, and the reconcile continues as today.

Rate-limit math (audited): existing tick ≈ 50 calls worst case; this adds
~5–10 (4 paginated status lists + completed + cancels); 30 ticks/hr ≈
1,800/hr vs the PAT's 5,000/hr. Non-issue.

## 4. Component 3 — concurrency sweep + guard

**Sweep:** add to `ci-health-watchdog-test.yml` (the only push/PR-triggered
workflow missing it):

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

**Guard:** `workflow-concurrency-guard` in vox-cli-ci (exact pattern of
`runner_policy_check.rs`): every workflow whose triggers include `push` or
`pull_request` must have a top-level `concurrency:` key or a backticked-
filename row in `docs/src/ci/concurrency-exceptions.md` (exceptions:
`release-binaries.yml`, `release-gui.yml`, `release-installers.yml`,
`scorecard.yml` — cancel-in-progress is wrong for releases and main-only
pushes). Strict in the pre-push step (tree is already clean). YAML 1.1
gotcha: serde_yaml parses the bare `on:` key as `Bool(true)` — handle both.

The exceptions doc is also the SSOT for "never cancel" workflows referenced
conceptually by §2's tag-push exemption; both new docs pages join
`DOCS_SSOT_FILES` in `ci/constants.rs` so deleting them fails CI (mirrors
`github-hosted-exceptions.md`).

## 5. Component 4 — hooks: the mechanical guarantee

New **checked-in** `.claude/settings.json` (as shipped — the PreToolUse
command is the fail-open wrapper adopted in commit `1b2126f866`, not the bare
`vox ci queue --hook-guard` this spec originally proposed; see mitigation 4
below):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|PowerShell",
        "hooks": [
          {
            "type": "command",
            "command": "out=$(vox ci queue --hook-guard 2>&1); code=$?; if [ \"$code\" -eq 2 ] && printf '%s' \"$out\" | grep -q 'Local-first CI'; then printf '%s\\n' \"$out\" >&2; exit 2; fi; exit 0"
          }
        ]
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

The matcher covers **both** exec tools this harness exposes (`PowerShell` is
enabled on this machine and its input schema also uses `command` — audit
finding B1; matchers are exact-string alternations, so `Bash|PowerShell` is
correct). Residual channels (MCP process tools, write-a-script-then-run) are
accepted: the threat model is a drifting-but-cooperative agent, not an
adversary.

### `--hook-guard` (PreToolUse)

Reads the hook JSON from stdin, extracts `tool_input.command`,
**normalizes** (lowercase; collapse all whitespace runs to single spaces —
kills the `gh  pr  checks` evasion class), then blocks iff any of:

1. `gh pr checks` — one-shot included; the deny message names the substitutes.
2. `gh run watch`
3. `gh api` AND (`check-runs` OR `check_runs`)
4. `ci watch-run` — catches `vox ci watch-run`, `cargo run … -- ci watch-run`.
5. Loop heuristic: (`while ` OR `until ` OR `for `) AND `sleep` AND
   (`gh pr` OR `gh run` OR `gh api`) — hand-rolled watch loops from allowed
   one-shot primitives (audit B3). One-shot reads stay allowed.
6. `gh alias set` AND (`pr checks` OR `run watch`) — cheap alias-evasion arm.

Dropped from rev 1: the `gh run view … --watch` arm — **`gh run view` has no
`--watch` flag** (`-w` is `--web`; live-verified), so the arm guarded a
nonexistent flag and only produced false positives on compound commands like
`gh run view 9 --log && vitest --watch`.

Escape hatch: `VOX_HOOK_GUARD_DISABLE=1` in the **hook process's**
environment (session-level, e.g. exported before launching Claude Code)
short-circuits to allow — for maintainer sessions working on the guard
itself. An env assignment *inside* the guarded command string does not reach
the hook process, so this is not an agent self-bypass.

On match: exit 2, stderr carries the replacement:

```text
Local-first CI: remote check-watching is disabled.
- Verdict: run local gates (`vox ci pre-push --complete`); green = done, push and move on.
- Queue + failures: `vox ci queue --json` (the `advice` field tells you what to do).
- Read one failure's logs: `gh run list --branch <b>` then `gh run view <id> --log-failed` (allowed).
- Clear backlog: `vox ci queue --clear`.
```

No match / unparseable JSON / stdin error: exit 0 silently. Purely local —
no file IO beyond stdin, no network (measured warm cost ≈ 40 ms per call).

### Known deployment hazard: the clap exit-2 collision

A **stale `vox.exe` on PATH** exits 2 on `unrecognized subcommand 'queue'` —
the same code that means "block" — which would deny every Bash/PowerShell
call with a clap usage error (live-verified against the current installed
binary). Mitigations, all required:

1. **Landing order**: install the new binary (`cargo install --path
   crates/vox-cli --locked`, or the repo's install flow) **before** the
   settings.json commit lands; the plan sequences this.
2. **Doctor diag**: a `vox doctor` check round-trips
   `echo '{"tool_input":{"command":"gh pr checks"}}' | vox ci queue
   --hook-guard` and flags exit-code/message mismatches (existing
   `[diag id=..]` pattern).
3. **Documented recovery** in the docs page: "if every shell call is blocked
   with a clap usage error, your `vox` binary predates the guard — reinstall
   it, or temporarily remove the PreToolUse hook via /hooks."
4. **The fail-open wrapper** (amended 2026-07-02, commit `1b2126f866`): the
   shipped hook wraps the call and only propagates exit 2 when stderr carries
   the `Local-first CI` deny marker; every other outcome — stale binary,
   missing binary, crash, unknown exit code — maps to exit 0. Rev 2 of this
   spec deliberately rejected this wrapper on the theory that hook commands
   must also run on Windows shells; that concern was empirically refuted —
   Claude Code executes hook commands via a POSIX shell (Git Bash) even on
   this native-Windows host. The wrapper was adopted after a live stale-binary
   lockout on 2026-07-02 blocked every Bash/PowerShell call in a session.
   See `docs/src/ci/local-first-ci.md` § "Stale-binary hardening".

`vox` missing from PATH entirely is safe: command-not-found is a non-2 exit →
non-blocking (fail-open with a transcript notice). With the shipped wrapper
(mitigation 4) this holds for *every* non-deny outcome, not just non-2 exits.

### SessionStart

`vox ci queue --brief --from-snapshot` prints ≤6 lines (queue depth, fleet
state, **FAILED lines for the current branch and main**, advice, sanctioned
commands) from the snapshot file; stdout lands in the new session's context.
Zero network at session start. With a stale binary this exits 2 harmlessly
(SessionStart cannot block; the brief is simply absent).

## 6. Component 5 — hosted-fallback tightening + the written contract

**`ci-fallback-hosted.yml`:** give `gui-windows-build-smoke` the same gate the
`gate` job already has —
`if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'fleet-down')`
— so hosted Windows builds run on schedule/dispatch/labeled-outage only, not
on every PR push. Update the workflow header comment and the
`github-hosted-exceptions.md` row to match reality (audit F3: the row
currently describes an "outage valve" while the job runs per-push).

**AGENTS.md** gains a "Local-First CI Verification Contract (Required, SSOT)"
section with the §0 contract verbatim (tier-accurate wording), placed after
"Local CI Gate Tiers (SSOT)"; the tier table there gains the
`workflow-concurrency-guard` row.

**New `docs/src/ci/local-first-ci.md`** (frontmatter: category
"CI & Quality", same shape as `github-hosted-exceptions.md`) documenting the
queue command, §2 classification semantics, the failure signal, the hooks,
the clap-collision recovery, and the deferred roadmap.
`docs/src/ci/runner-autoscaling.md` gets a cross-link.

## 7. AI-first discoverability (SSOT registration)

Required, not optional: register `vox ci queue` and
`vox ci workflow-concurrency-guard` in
`contracts/operations/catalog.v1.yaml` (mirror an existing `vox ci` row's
shape), then regenerate the projection chain:

```text
vox ci operations-sync --target cli --write   # catalog → command-registry.yaml
vox ci command-sync --write                   # registry → cli-command-surface.generated.md
```

This is what makes the commands discoverable to agents via the generated CLI
surface and MCP planner metadata — the difference between "a command exists"
and "an AI-first surface exists". (Audit note: skipping this does NOT fail
any guard today — `runner-scale` is live-but-unregistered precedent — which
is exactly why it must be explicit here.)

## 8. Error handling

| Failure | Behavior |
| --- | --- |
| `gh` unreachable in `queue` | `degraded:true`; advice says proceed on local gates; exit 0 for `--brief`/`--from-snapshot`; exit 1 for `--clear` |
| `--clear` combined with `--from-snapshot` | hard error (never cancel from stale data) |
| `gh` unreachable in autoscaler clear | skip clear this tick, log `degraded`, reconcile continues |
| single cancel fails (409 = completed) | log, continue sweep |
| run still in_progress one tick after cancel | force-cancel escalation (§3) |
| fleet_alive == 0 | stale sweep disabled; superseded sweep still runs; advice names the outage |
| >50 cancellable | cancel 50, log remainder, next tick continues |
| snapshot file missing/stale | one-line "unavailable" message, exit 0 |
| hook JSON unparseable / stdin error | exit 0 (fail-open) |
| stale `vox.exe` on PATH | §5 hazard: blocks all exec calls with clap error — mitigations 1–3 required |
| no failures in window | brief omits FAILED lines |
| tab/newline in a workflow `name` | impossible by construction — key is `.path` |

## 9. Testing

House style: pure logic as free functions with in-module unit tests.

- `is_exempt`: event allowlist (unknown events exempt), main, "null" branch,
  tag-pattern push (`v0.6.0` yes, `very-cool-branch` no), `run_attempt > 1`,
  `waiting` status.
- `classify_runs`: supersede key includes repo AND event (fork collision;
  push/PR sibling non-cancellation); strictly-newer tie-break; stale TTL
  boundary; **stale disabled at fleet_alive == 0**; pending counts as stale-
  eligible; in_progress never stale.
- `clear_plan`: only non-exempt non-Active; 50-cap; ordering.
- `advice_for`: healthy / clearable / outage-backlog / branch-failure-leads /
  degraded phrasings.
- Failure signal: conclusion filter (cancelled excluded), 24 h cutoff, cap 20,
  brief renders FAILED lines for current branch + main, omits when clean.
- `hook_guard_matches`: every arm incl. whitespace-collapse evasions, loop
  heuristic hits (`while…sleep…gh run list`) and one-shot non-hits
  (`gh run list`, `gh run view 9 --log-failed`, `gh pr view --json
  statusCheckRollup` one-shot), alias arm, env escape.
- `workflow-concurrency-guard`: YAML-1.1 `on:`-as-Bool(true), scalar/seq/map
  trigger shapes, exceptions parsing.
- Snapshot: atomic write (temp+rename), round-trip, staleness boundary,
  `cancelled_last_sweep` persistence.

## 10. Landing order

Each component independently landable; order is load-bearing for §5's hazard:

1. `vox ci queue` read-only (snapshot + failures + brief/json/table).
2. `--clear` + autoscaler-tick integration + force-cancel escalation.
3. `--hook-guard` + **install the binary** + doctor diag.
4. Hooks commit (`.claude/settings.json`) — only after 3 is installed.
5. Concurrency sweep + guard; `ci-fallback-hosted` gate fix.
6. AGENTS.md + docs pages + `DOCS_SSOT_FILES`.
7. Catalog registration + projection regen (§7).
