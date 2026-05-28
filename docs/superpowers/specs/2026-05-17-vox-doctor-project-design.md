# `vox doctor --project` — CR-L7 Project Health Check

**Date:** 2026-05-17
**Closes:** CR-L7's third leg (`vox doctor`) at minimum-viable scope.
**Pegs to:** [`v1-llm-target-implementation-plan-2026.md`](docs/src/architecture/v1-llm-target-implementation-plan-2026.md) §1.5 P4.6.

## Goal

Ship the missing third leg of `vox new → vox deploy → vox doctor` (CR-L7) with structured JSON output and `vox.doctor.*` telemetry. The existing `vox doctor` is environment-focused (cross-compile toolchains, CUDA, build perf) and does not check project health. CR-L7's integration test needs a project-scope health check that the deploy step can chain into.

## Scope

Add `--project [PATH]` to the existing [`DoctorArgs`](crates/vox-cli/src/cli_args.rs:287). When set, the doctor runs project-health mode instead of environment-check mode and ignores the environment flags. PATH defaults to `.`.

**Project-health = "all `.vox` files under PATH compile clean."** This is the deterministic, real, in-process check that the deploy integration test consumes. Aggregate outcome is `green` or `red`.

## Out of scope

- Running `@test` blocks. That requires a daemon RPC ([`vox-cli/src/commands/runtime/run/test.rs`](crates/vox-cli/src/commands/runtime/run/test.rs)) and is deferred until either an in-process Vox test runner exists or `vox doctor` is allowed to assume a daemon.
- Calling out to deployed-app health endpoints (HTTP GET /health). The CR-L7 spec is silent on remote health; we limit to on-disk artifact health.
- Walking outside the project root.

## Implementation seams

- `crates/vox-cli/src/cli_args.rs:287` — add `--project` to `DoctorArgs`.
- `crates/vox-cli/src/cli_dispatch/lanes.rs:8` — dispatch to project mode when `--project` is set; environment mode unchanged.
- `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs` — new module `project_check`.
- `crates/vox-cli/src/commands/diagnostics/doctor/project_check.rs` — `run(&Path, json: bool)`.
- `crates/vox-telemetry/src/types.rs` — new `TelemetryEvent::DoctorProjectCheck(DoctorProjectCheckEvent)` variant + `METRIC_TYPE_DOCTOR_PROJECT_CHECK` constant.

## Algorithm

```
fn run(project_root, json):
    canonical_root = canonicalize(project_root)  # bail if missing
    files = walk(canonical_root, .vox extension, skip target/node_modules/.git/...)
    for path in files:
        source = read(path)
        diags = vox_compiler::pipeline::check_file(source, path)
        record (path, errors, warnings)
    files_passing = count(error_count == 0)
    files_failing = count(error_count > 0)
    outcome = "green" if files_failing == 0 else "red"
    emit DoctorProjectCheckEvent
    print report (json or human)
    bail if files_failing > 0
```

## Skipped directories

`target`, `node_modules`, `.git`, `.cargo`, `dist`, `build`, `archive`. These are build outputs, dep caches, or tombstoned per AGENTS.md §Archival Protocol. Including them would surface stale or non-source `.vox` files and pollute the health signal.

## Tests (TDD)

1. `green_when_all_files_compile_clean` — happy path on a 2-file tempdir.
2. `red_when_a_file_fails_to_compile` — exit-error path.
3. `missing_project_root_returns_error` — `--project` to a non-existent path.
4. `empty_project_is_green` — vacuously green when no `.vox` files exist.
5. `skipped_dirs_are_not_walked` — broken file under `target/` does not fail the run.

## Why this shape

- **No stub.** The runner does real work today and reports real outcomes.
- **No daemon dependency.** Uses the in-process `vox_compiler::pipeline::check_file` API.
- **CR-L7 deploy test consumable.** Structured JSON with stable schema_version means the integration test can `jq '.outcome == "green"'`.
- **Telemetry observable.** Every run emits one `vox.doctor.project_check` event, so post-v1.0 dashboards can chart project-health rates across CI runs.

## Follow-on (not in this commit)

- Wire the deploy integration test once Marquee slots 2 (`todo-auth`) and 3 (`chat`) land their fixture apps (per [`contracts/marquee/manifest.v1.yaml`](../../../contracts/marquee/manifest.v1.yaml)).
- Add `--for-llm` to mirror `vox check`'s minimal-repro output mode for agent consumption.
- Add a config-aware mode that reads `Vox.toml` to pick the source root rather than walking from `.`.
