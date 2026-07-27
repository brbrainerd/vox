@echo off
REM Versioned copy of the VoxCIRunnerScale scheduled-task command.
REM Previously lived only as an untracked C:\Users\Owner\vox-runner-scale-task.cmd,
REM which drifted from the checked-in Task Scheduler XML and couldn't survive a
REM machine re-image. Now the XML points here directly.
REM
REM Skip the freshness guard so the task survives a stale installed vox binary
REM (main advances faster than reinstalls). Set up 2026-07-02.
set VOX_SKIP_FRESHNESS_CHECK=1
REM Cap runners at 2. History: capped at 4 (was 6) on 2026-07-17 because six
REM concurrent heavy jobs OOM-killed rustdoc/clippy on the 31GB WSL VM at the
REM per-runner memory budget then in effect (5000m). That budget was later
REM raised to 14000m (crates/vox-cli/src/commands/ci/runner_scale.rs
REM MEM_PER_RUNNER, see docs/src/ci/runner-autoscaling.md) after a real build
REM measured peaking at ~12GB RSS — but this cap was never re-checked against
REM the new budget: 4 x 14000m = 56GB blows well past the 32GB WSL ceiling.
REM 2 x 14000m = 28GB fits with headroom for Windows; this is what the docs'
REM own sizing math already assumes. Fixed 2026-07-27.
set VOX_RUNNER_MAX=2
cd /d C:\Users\Owner\vox

REM Task Scheduler discards stdout/stderr by default, so every tick's reap
REM reasoning (`runner-scale: scale-down reap of X blocked (...)`, `[reap] X
REM (idle > reap grace)`, etc.) was vanishing silently — the only reason a
REM 2026-07-27 incident (containers dying ~15s after spawn, before any reap
REM path's own eligibility gates should allow it) couldn't be root-caused
REM after the fact. Capture it. Rotate at ~5MB so a runaway tick can't grow
REM this unbounded; one prior generation is kept for cross-rotation context.
REM
REM Write to a per-invocation temp file, THEN best-effort-merge into the
REM shared log, rather than redirecting `vox` straight into the shared log.
REM A prior version of this script did the direct redirect and, when the
REM shared log was transiently locked (antivirus scan, a concurrent manual
REM debug invocation, Windows Search indexing), cmd.exe's failure to open
REM that redirect aborted the WHOLE script before `vox` ever ran — silently
REM stalling the entire reconcile loop for the tick, not just its logging
REM (2026-07-27 incident: found via a stuck nightly re-run and `Last Result:
REM 1` from schtasks). `%RANDOM%` makes the temp path collision-proof enough
REM that this can never happen to the actual reconcile again.
set LOG=C:\Users\Owner\vox\.ci-runner-logs\runner-scale.log
set TMPLOG=C:\Users\Owner\vox\.ci-runner-logs\runner-scale.%RANDOM%.tmp
if not exist "C:\Users\Owner\vox\.ci-runner-logs" mkdir "C:\Users\Owner\vox\.ci-runner-logs"
for %%F in ("%LOG%") do if %%~zF GTR 5242880 (move /y "%LOG%" "%LOG%.old" >nul 2>&1)

echo [%date% %time%] tick start > "%TMPLOG%"
vox ci runner-scale --apply >> "%TMPLOG%" 2>&1

REM Best-effort merge; a lock here is now harmless (last thing the script
REM does), unlike the direct-redirect version above.
type "%TMPLOG%" >> "%LOG%" 2>nul
del "%TMPLOG%" >nul 2>&1
