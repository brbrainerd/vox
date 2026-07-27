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
REM after the fact. Capture it.
REM
REM One file per tick (LOGDIR\runner-scale.<timestamp>.<RANDOM>.log), not one
REM shared appended log. Two earlier designs both failed under real lock
REM contention (found live this session, both via a stuck nightly re-run):
REM   1. `vox ... >> shared-log` directly: a lock on the shared log aborted
REM      the WHOLE script before `vox` ever ran, stalling the entire
REM      reconcile, not just its logging.
REM   2. `vox ... >> tmp-file`, then best-effort `type tmp >> shared-log`:
REM      survived (1), but cmd.exe does not reliably set ERRORLEVEL when a
REM      redirect's target is locked (the failure happens at shell-level
REM      redirect setup, before the command it's attached to even runs) — so
REM      `if errorlevel 1` after the merge silently didn't detect it, the
REM      "successful" branch ran, and the tick's output was lost anyway.
REM Writing straight to a fresh, guaranteed-unique-enough path sidesteps
REM shared-write contention entirely — nothing else is ever writing to THIS
REM tick's file, so there's nothing to lock. `%RANDOM%` on top of the
REM timestamp is redundant insurance, not the sole uniqueness guarantee.
set LOGDIR=C:\Users\Owner\vox\.ci-runner-logs
if not exist "%LOGDIR%" mkdir "%LOGDIR%"
set TICKSTAMP=%date:~-4%%date:~4,2%%date:~7,2%-%time:~0,2%%time:~3,2%%time:~6,2%
set TICKSTAMP=%TICKSTAMP: =0%
set TICKLOG=%LOGDIR%\runner-scale.%TICKSTAMP%.%RANDOM%.log

echo [%date% %time%] tick start > "%TICKLOG%"
vox ci runner-scale --apply >> "%TICKLOG%" 2>&1
set VOX_EXIT=%errorlevel%

REM Best-effort prune to the newest 500 tick files (~1.7 days at the 2-min
REM schedule interval) so this directory doesn't grow unbounded. A prune
REM failure (one file locked, e.g. someone has it open for inspection) only
REM leaves that one file past its turn — it never touches the tick log just
REM written above, so a lock here can never lose this tick's diagnostics.
for /f "skip=500 delims=" %%F in ('dir /b /o-d "%LOGDIR%\runner-scale.*.log" 2^>nul') do del "%LOGDIR%\%%F" >nul 2>&1

REM `for /f "skip=N"` leaves ERRORLEVEL 1 when fewer than N lines exist to
REM skip past (found live: this made schtasks report every tick as failed
REM until 500 tick-log files accumulate — a purely cosmetic prune-loop
REM artifact, not a real failure, but one that poisons the `Last Result`
REM health signal this whole investigation relied on). Report the ACTUAL
REM vox exit code captured above, not whatever the prune loop happened to
REM leave in ERRORLEVEL — a real vox failure must still surface.
exit /b %VOX_EXIT%
