---
title: "Local-first CI: queue signal, failure signal, and agent contract"
description: "The vox ci queue SSOT signal, superseded/stale auto-clearing, the async failure signal, and the hooks that keep agents on the local runner fleet."
category: "CI & Quality"
last_updated: "2026-07-02"
training_eligible: true

schema_type: "TechArticle"
---

# Local-first CI: queue signal, failure signal, and agent contract

The local runner fleet is the CI plane; GitHub Actions remains the job queue it
consumes. This page documents the machinery that keeps every harness call and
every agent on that plane mechanically.

## The contract

Local gates green (`vox ci pre-push --complete`, or `--full` when code/tests
changed) is the verdict for what they cover: push and move on. Fleet CI is
authoritative for everything without a local equivalent; its verdicts arrive
asynchronously through the failure signal below — never through an agent
sitting in a watch loop. Remote check-watching (`gh pr checks`,
`gh run watch`, check-runs polling, `vox ci watch-run`, hand-rolled gh+sleep
loops) is blocked for agent sessions by the PreToolUse hook in
`.claude/settings.json`. Reading a specific failure's logs stays allowed:
`gh run list --branch <b>`, then `gh run view <id> --log-failed`.

## `vox ci queue`

Run-centric queue snapshot. A run is cancellable only when ALL of: event is
`push` or `pull_request` (all other events — merge_group, schedule,
workflow_dispatch, workflow_run, anything unknown — are exempt by default);
branch is not `main`, not a `v<digit>…` tag, not null; first attempt
(re-runs are explicit human requests); not `waiting` (deployment approval).

- **superseded** — a strictly newer run exists for the same
  (workflow-path, head-repo, branch, event); only the newest survives.
- **stale** — `queued`/`pending` past the TTL (default 45 min,
  `--ttl-mins`) — only while the fleet has live runners. A deep queue at
  fleet zero is an outage, not abandonment; the stale sweep self-disables.

Flags: `--json`, `--brief` (SessionStart injection), `--from-snapshot`
(no network; reads `~/.vox/ci-queue-snapshot.json`, ≤2 min stale in steady
state, hard cap 10 min), `--clear [--dry-run]` (live data only; ≤50
cancellations per sweep), `--hook-guard` (PreToolUse mode;
`VOX_HOOK_GUARD_DISABLE=1` session env is the maintainer escape).

## The failure signal

Every snapshot also records completed runs from the last 24 h with conclusion
`failure`/`timed_out`/`startup_failure` (cap 20, `cancelled` excluded so
auto-clear's own work never echoes as failure). The SessionStart brief and
`vox ci queue` surface FAILED lines for the current branch and for main, and
the `advice` field leads with the fix path. This is the mechanism behind
"failures come back as a signal".

## Auto-heal

Every `vox ci runner-scale` tick (~2 min) auto-clears per the rules above,
escalates to `force-cancel` any run still in_progress one tick after being
cancelled (shielded post-steps), rewrites the snapshot atomically, and logs
`cleared_superseded`/`cleared_stale` (actual cancellations only) to the
scale-event ledger.

## Flood prevention at the source

Push/PR-triggered workflows must declare
`concurrency: { group: workflow-ref, cancel-in-progress: true }` — enforced
strictly in pre-push by `vox ci workflow-concurrency-guard`, with exceptions
in [concurrency-exceptions](concurrency-exceptions.md). The hosted fallback's
Windows smoke runs only on schedule/dispatch/`fleet-down`-labelled PRs.

## If every shell call is suddenly blocked

A stale `vox` binary on PATH exits 2 (clap usage error) on the unknown
`queue` subcommand — the same exit code the hook uses to block. Fix:
`cargo install --path crates/vox-cli --locked` (rename/stop a locked
`vox.exe` first), or temporarily remove the PreToolUse hook. `vox doctor`
detects this state (`ci.hook_guard_stale_binary`).

## Deferred roadmap

Local verdict ledger (`vox ci verdict <sha>`), a local orchestration plane
bypassing the Actions queue, and hosted-job migration
(`vox ci runner-policy-check --strict` flip) — see the design spec
`docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md`.
