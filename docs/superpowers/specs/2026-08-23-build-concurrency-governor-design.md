---
title: "Build Failure Classification and Agent Build Throttling"
description: "Why the CPU-oversubscription theory of phantom build failures was wrong, and what survives it: a format-agnostic build-failure classifier, a per-agent CARGO_BUILD_JOBS cap, and a disk/RAM preflight in vox doctor."
category: "Architecture SSOTs"
---

# Build Failure Classification and Agent Build Throttling

This document replaces an earlier design ("Build Concurrency Governor", same
path, 2026-08-23) that proposed a host-wide jobserver broker. A seven-track
audit falsified the causal claim that design rested on and found the mechanism
unsafe. The broker is **rejected** — see [Rejected: the token
broker](#rejected-the-token-broker), which is kept deliberately so nobody
rebuilds it.

What survives is smaller and, unlike the broker, addresses a cause that was
actually measured.

## The original theory, and how it was falsified

**The claim.** 22 concurrent rustc processes on a 12-core host caused a family
of build failures: rustc exiting nonzero with zero diagnostics, `lld-link`
permission errors, `cc` failing on `zstd-sys`, bare `could not compile` lines.
Cap total parallelism, the reasoning went, and the failures stop.

**Why it is wrong.** CPU oversubscription has no causal path to a nonzero rustc
exit. An oversubscribed scheduler time-slices; it does not kill processes,
truncate writes, or fail `CreateProcess`. Oversubscription makes builds *slow*.
It does not make them *fail*. Every failure signature in the corpus has a
memory or disk explanation, and none has a core-count explanation.

**What the host actually looks like** (measured 2026-08-23):

| Quantity | Value |
|---|---|
| Physical RAM | 15.7 GB |
| Free physical RAM, zero rustc running | 1.6 GB |
| Commit charge / commit limit | 31.1 GB / 58.7 GB |
| Peak working set, single rustc | 1,193 MB |

A host that idles at 1.6 GB free cannot run many 1.2 GB compilers. It pages,
and under Windows commit pressure allocation failures surface as abrupt
non-diagnostic exits — exactly the observed signature. Core count is not the
binding constraint; memory is.

**The one proven cause.** ENOSPC. `os error 112` ("There is not enough space on
the disk") appears in the corpus with the disk at 916 MB free of 476 GB. That
run left truncated `.rmeta` files, and the *next* build reported
`only metadata stub found for rlib dependency 'std'` and
`E0460: found possibly newer version of crate 'windows'` — a stale-artifact
error whose true cause was the previous run's full disk. Nothing monitors free
disk today.

**The evidence is also confounded.** `jobs = 24` was set in `.cargo/config.toml`
on a 12-core host and was live for every failure in the corpus. It was removed
at 14:44 on 2026-08-23. Every captured log after that timestamp contains only
real errors with diagnostics attached, and zero contention signatures. A
misconfiguration that doubled per-cargo parallelism, and that is already gone,
is a better explanation of the corpus than any missing coordination mechanism.

## Problem, restated

Two problems remain, and they are not the one the broker addressed.

1. **Agents edit working code after misreading a build failure.** A failure with
   no diagnostics is not a code defect, but the tooling reports it identically
   to one. Two sessions independently lost time to this.
2. **Nothing watches the resources that actually break builds.** Free disk is
   the one proven cause of a corrupted-artifact cascade and is unmonitored.
   Free RAM is the plausible cause of the non-diagnostic exits and is likewise
   unmonitored.

## Non-goals

Making builds faster. Coordinating parallelism across processes or machines
(see [Rejected](#rejected-the-token-broker)). Changing what humans get: the
throttle below is agent-scoped and a human shell is unaffected.

## Design

### 1. Failure classifier

The genuinely valuable piece, and the only one that was ever load-bearing. A
pure function over build output: no I/O, no shared state, no platform
dependence, works identically on Linux and CI.

**The previous algorithm was 100% wrong on this repo's output.** It tested
`line.starts_with("error[")` to decide whether real diagnostics were present.
This repo builds with `--message-format short`, where a diagnostic reads:

```text
crates\vox-gui\src\commands\mic.rs:169:51: error[E0610]: `{integer}` is a primitive type and therefore doesn't have fields
```

The line starts with a path, not with `error[`. The predicate therefore returned
`false` on **every real failing log this host produced**, and every genuine
compile error was classified `Contention` — "retry, don't edit code". That is
precisely the dangerous direction the classifier exists to prevent. Two of the
likeliest consumers, the agent-facing cargo-check tools at
`crates/vox-orchestrator-mcp/src/compiler_tools.rs:216` and `:958`, hardcode
`--message-format=short`.

Two further defects: `Contention` was tested before `Real`, so a mixed run
(crate A a real error, crate B a linker contention failure) returned
`Contention`; and `E0460` / `os error 112` were matched as bare substrings
anywhere in the output, so a *test named after* E0460 classified the run as
`Corruption`.

**Corrected algorithm.**

1. **Extract source diagnostics, format-agnostically.** A line is a source
   diagnostic if any of: it contains an error code (`error[E`); it matches the
   short format `path:line:col: error`; it is a JSON compiler message
   (`"level":"error"`); or it is a bare `error: …` whose next non-empty line is
   a `-->` location arrow (the full format's uncoded diagnostics). Cargo's own
   summaries — `could not compile`, `build failed`, `aborting due to`,
   `failed to run custom build command`, `linking with … failed` — are *not*
   source diagnostics. They name no source location and carry no code.
2. **Truncation guard.** If the capture was truncated and no diagnostics were
   found, return `Real`. A partial capture is indistinguishable from a
   no-diagnostic build, and `Real` is the safe direction: it costs an
   investigation, where a wrong `Contention` costs an edit to working code.
3. **Corruption, anchored.** If a *diagnostic* line carries a corruption marker
   (`E0460`, `only metadata stub found for`, `found invalid metadata files for
   crate`), or the output contains `os error 112` / `not enough space on the
   disk` on an error line, return `Corruption`. Anchoring is what stops a test
   name from tripping it.
4. **Real before Contention.** Any diagnostics at all → `Real`. This ordering is
   the fix for the mixed-run defect: a real error anywhere outranks contention
   anywhere.
5. **Contention as the fallback only.** Zero diagnostics on a failed build means
   the compiler died without saying why. No marker list is needed. In
   particular `cc failed`, `Permission denied`, and `failed to run custom build
   command` are **dropped** as unconditional contention markers — each also
   names a real bug, and a build-script failure with zero diagnostics already
   reaches `Contention` through this fallback.

**It returns evidence, not a verdict.** `BuildFailure { kind, diagnostics }`, so
a caller prints `classified as contention: 0 source diagnostics found` and a
human overrules it at a glance. A bare enum makes a wrong classification
invisible; the diagnostics list makes it a one-line check.

### 2. `CARGO_BUILD_JOBS = 4` for agent shells

This replaces the broker entirely. One line in `.claude/settings.json` under
`env`:

```json
"env": { "CARGO_BUILD_JOBS": "4" }
```

Agent-scoped (a human shell never sees it), no shared mutable resource, nothing
to keep alive, nothing to leak, nothing that can deadlock, and it degrades to
"the setting is absent" rather than to a hang.

**Why 4, and why not a core-derived number.** The measurements above make core
count the wrong basis. Peak rustc working set is 1,193 MB, and the host idles
at 1.6 GB free of 15.7 GB. A CPU-derived cap of 12 implies a worst case near
14 GB of compiler resident set on a machine that has less than that available —
it is a paging plan, not a build plan. Four rustc at peak is roughly 4.8 GB,
which fits within the reclaimable headroom on this host with room for the OS,
the editor, and the agent harness. Four also keeps a two- or three-worktree
agent host inside the commit budget, because the cap is per cargo process and
agent sessions are few.

Treat 4 as a memory-derived value with a calibration knob, not a constant of
nature: the right number on a 64 GB host is larger, and the correct way to
re-derive it is `available_bytes / peak_rustc_working_set`, never
`nproc`.

### 3. Disk and memory preflight in `vox doctor`

ENOSPC is the only *proven* cause in the corpus and nothing watches it. Two
checks in `vox doctor`'s build-health group, both cheap and on-demand:

- **Free disk on the target-directory volume.** Fail below 5 GB, warn below
  20 GB. A full-disk build does not merely fail; it poisons the target
  directory for subsequent builds, which is how a disk problem gets
  misdiagnosed as a compiler problem.
- **Free physical RAM.** Fail below 1 GB, warn below 2 GB, and always report
  the number alongside `CARGO_BUILD_JOBS` so the ratio is visible. On this host
  the check is expected to warn at rest — that is the finding, not a false
  positive.

Both emit the existing `[diag id=… sev=… heal=…]` contract so agents can act on
`FIX:` directly.

## Rejected: the token broker

Recorded so it is not proposed again.

A resident `vox build-broker` was designed to hold a fixed-name Windows named
semaphore of N GNU-make jobserver tokens, with `CARGO_MAKEFLAGS` in
`.claude/settings.json` pointing every cargo on the host at one pool. It was
audited and rejected on four independent grounds; any one of them is fatal.

1. **It addresses a non-cause.** Per [the falsification](#the-original-theory-and-how-it-was-falsified),
   capping CPU parallelism cannot prevent any observed failure. The broker
   would have been maintained forever against a class of failures it does not
   touch.
2. **No owner-death recovery.** A Windows named semaphore does not return
   tokens held by a process that was hard-killed. Every killed cargo leaks its
   tokens permanently, and the pool ratchets monotonically toward zero. The
   design's own recovery tool, `vox ci kill-stuck-tests`, kills cargo — so the
   remedy fed the failure. The end state is every build on the host blocking
   forever, producing no output. That trades loud, reproducible-clean failures
   for a silent hang, which is strictly worse.
3. **Its acceptance test cannot pass.** The proposed integration test — one
   token, two builds, assert they serialize — is unpassable by construction:
   each jobserver client holds one *implicit* token it never requests, so two
   cargos against a one-token pool run two rustc, not one.
4. **Its arithmetic was wrong.** Peak compiler processes is `M + N + B`, not
   `M + N`: M cargo processes each contribute an implicit token, N is the pool,
   and B is the count of build scripts running `cc`, since the `cc` crate
   grants itself its own implicit token. Any N chosen from the stated formula
   under-counts.

The correct shape of this problem is a per-process cap with no shared state —
[§2](#2-cargo_build_jobs--4-for-agent-shells).

## Testing

The classifier gets unit tests over captured output, and the corpus must include
short-format, full-format, and JSON-format diagnostics, because format-blindness
is the exact defect that shipped. Required cases: a short-format `E0610`
classifies `Real`; a mixed run (real error + linker failure) classifies `Real`;
output containing the literal `E0460` in a *test name* rather than a diagnostic
does not classify `Corruption`; a truncated capture with no diagnostics
classifies `Real`; a zero-diagnostic failure classifies `Contention` and reports
zero diagnostics as evidence.

The doctor checks get tests against a temp directory (disk) and against injected
threshold values (RAM), not against the live host, so they are deterministic.

There is nothing else to test, because there is nothing else with state.

## Risks

**`CARGO_BUILD_JOBS = 4` is slower on an idle host.** Accepted. It is one
JSON line to raise, affects agents only, and a wrong value costs minutes rather
than a hung host.

**The classifier can still call a real failure `Contention`** when the
compiler emits no diagnostics — a genuine duplicate-symbol linker error, for
instance. Mitigated, not eliminated, by returning the evidence: the verdict is
always printed with the diagnostic count that produced it.

## Open questions

Whether 4 is right under three concurrent agent worktrees on this host is
unmeasured. The preflight in §3 provides the instrument; re-derive from
`available_bytes / 1.2 GB` once there is data under load.

## Appendix A: verified jobserver facts

These were established by measurement during the rejected design's work and are
correct. They are retained because they were expensive to obtain and because
the first two are non-obvious enough to be re-litigated otherwise.

1. **Cargo is a jobserver client and honors `CARGO_MAKEFLAGS`.** It reads the
   variable at startup, connects, and takes a token per rustc it spawns.
2. **A missing jobserver warns and proceeds.** With `CARGO_MAKEFLAGS` pointing
   at a nonexistent semaphore, cargo prints
   `warning: failed to connect to jobserver …` and completes a normal
   successful build. It never hard-fails — unlike a committed
   `rustc-wrapper = "sccache"` line, which kills cargo outright on any host
   lacking the binary (already documented in `.cargo/config.toml`).
3. **`.cargo/config.toml [env]` cannot deliver `CARGO_MAKEFLAGS`.** Cargo reads
   the jobserver from the process environment at startup, *before* it applies
   config `[env]`. Verified: an `[env] CARGO_MAKEFLAGS` pointing at a
   nonexistent semaphore produced no connect warning at all, proving cargo
   never read it. Any future jobserver work must deliver the variable from the
   parent environment.
4. **Windows named semaphores have no owner-death recovery**, which is
   rejection ground 2 above.

## Appendix B: the `jobs = 24` misconfiguration

`.cargo/config.toml` carried `jobs = 24` on a 12-core host. It was live for
every failure in the failure corpus and was removed at 14:44 on 2026-08-23.
Recorded here because it is the confound that made the CPU-oversubscription
theory look plausible, and because any future analysis of logs from before that
timestamp must account for it.
