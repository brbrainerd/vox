---
title: "Local-first CI: queue signal, failure signal, and agent contract"
description: "The vox ci queue SSOT signal, superseded/stale auto-clearing, the async failure signal, and the hooks that keep agents on the local runner fleet."
category: "CI & Quality"
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

Push/PR-triggered workflows must declare a top-level `concurrency:` mapping
containing `cancel-in-progress: true` (a bare group string or a non-cancelling
group does not count) — enforced strictly in pre-push by
`vox ci workflow-concurrency-guard`, with exceptions in
[concurrency-exceptions](concurrency-exceptions.md). The hosted fallback's
Windows smoke runs only on schedule/dispatch/`fleet-down`-labelled PRs.

## Stale-binary hardening (why one shell call can never lock out a whole session)

A stale, missing, or crashed `vox` on PATH would otherwise exit 2 on the
unknown `queue` subcommand (or any other error) — the same code the hook
uses to block. The hook `command` in `.claude/settings.json` is not the bare
`vox ci queue --hook-guard` invocation; it wraps it:

```sh
out=$(vox ci queue --hook-guard 2>&1); code=$?
if [ "$code" -eq 2 ] && printf '%s' "$out" | grep -q 'Local-first CI'; then
  printf '%s\n' "$out" >&2; exit 2
fi
exit 0
```

Only an exit-2-**and**-deny-marker combination blocks. Every other outcome
(missing binary, wrong/stale binary, crash, a future unrelated exit code)
falls through to `exit 0` — fail-open on infrastructure, fail-closed only on
a genuine, confirmed deny. This was hardened after a stale binary briefly
blocked every Bash/PowerShell call in a live session (2026-07-02); `vox
doctor`'s `ci.hook_guard_stale_binary` diag still exists as an advisory
signal, but the hook no longer depends on catching it in time.

If the installed `vox` genuinely needs refreshing: `cargo install --path
crates/vox-cli --locked --debug` is fast (~30s, no LTO) and reliable;
plain `--locked` (release, LTO) has been observed to crash the linker
transiently on this host — retry or fall back to `--debug` if it does.

## Deferred roadmap

Local verdict ledger (`vox ci verdict <sha>`), a local orchestration plane
bypassing the Actions queue, and hosted-job migration
(`vox ci runner-policy-check --strict` flip) — see the design spec
`docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md`.
