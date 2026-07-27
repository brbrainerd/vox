@echo off
REM Versioned copy of the VoxCIRunnerScale scheduled-task command.
REM Previously lived only as an untracked C:\Users\Owner\vox-runner-scale-task.cmd,
REM which drifted from the checked-in Task Scheduler XML and couldn't survive a
REM machine re-image. Now the XML points here directly.
REM
REM Skip the freshness guard so the task survives a stale installed vox binary
REM (main advances faster than reinstalls). Set up 2026-07-02.
set VOX_SKIP_FRESHNESS_CHECK=1
REM Cap runners at 4 (was 6): six concurrent heavy jobs OOM-kill rustdoc/clippy
REM on the 31GB WSL VM (sccache 'Compiler killed by signal 9'). Set 2026-07-17.
set VOX_RUNNER_MAX=4
cd /d C:\Users\Owner\vox

REM Task Scheduler discards stdout/stderr by default, so every tick's reap
REM reasoning (`runner-scale: scale-down reap of X blocked (...)`, `[reap] X
REM (idle > reap grace)`, etc.) was vanishing silently — the only reason a
REM 2026-07-27 incident (containers dying ~15s after spawn, before any reap
REM path's own eligibility gates should allow it) couldn't be root-caused
REM after the fact. Capture it. Rotate at ~5MB so a runaway tick can't grow
REM this unbounded; one prior generation is kept for cross-rotation context.
set LOG=C:\Users\Owner\vox\.ci-runner-logs\runner-scale.log
if not exist "C:\Users\Owner\vox\.ci-runner-logs" mkdir "C:\Users\Owner\vox\.ci-runner-logs"
for %%F in ("%LOG%") do if %%~zF GTR 5242880 move /y "%LOG%" "%LOG%.old" >nul 2>&1

echo [%date% %time%] tick start >> "%LOG%"
vox ci runner-scale --apply >> "%LOG%" 2>&1
