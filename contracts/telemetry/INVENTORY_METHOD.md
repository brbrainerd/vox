---
category: "Telemetry Contracts"
---

# Telemetry Emit-Site Inventory — Method

Produced by Track A (2026-06-19). Re-run to update.

## How the inventory was built

1. **grep sweep** across `crates/` for `record_event!`, `TelemetryEvent::`, `record_task_started`,
   and `fill_task_root_summary` — excluding test files:
   ```
   grep -rn "record_event!\|TelemetryEvent::\|record_task_started\|fill_task_root_summary" \
     crates/ --include="*.rs" -l | grep -v /tests/
   ```
   Yielded **37 source files** across 10 crates.

2. **Parallel Explore subagents** (4 clusters) read each file and extracted one CSV row per call site,
   noting: crate, relative file path, line number, event_type (TelemetryEvent variant), category
   (existing collection category), and fields_used.

3. **Cross-check** against `crates/vox-telemetry/src/types.rs`:
   - All declared `TelemetryEvent` variants are represented in the inventory (or noted as unused below).
   - METRIC_TYPE_* constants in ResearchMetric calls are noted where relevant.

4. **Proposed sites** (status=proposed) added by A2 analysis — one per new product category.
   These are the recommended chokepoints for the new `command_usage`, `skill_activation`,
   `edit_pattern`, `harness_usage`, and `error_surface` emit sites.

## Crate clusters used

| Cluster | Crates | Agents |
|---------|--------|--------|
| 1 | vox-actor-runtime, vox-audit | 1 |
| 2 | vox-cli, vox-code-audit | 1 |
| 3 | vox-codegen, vox-db, vox-orchestrator | 1 |
| 4 | vox-orchestrator-mcp, vox-plugin-host, vox-telemetry | 1 |

## TelemetryEvent variants with no emit sites (unused as emitters)

These variants are defined in `types.rs` but have no production `record_event!` calls
(only test fixtures or type definitions):

- `LintAutofix` — constructed in vox-audit/src/aggregator.rs for test/aggregation purposes only
- `RepairAttempt` / `RepairOutcome` — constructed in vox-audit/recorder.rs for aggregation
- `HoleObserved` — defined but no production emit site found
- `PromptDispatch` — AiFixture-related; constructed in cascade.rs context

## Reproduction command

```bash
grep -rn "record_event!\|TelemetryEvent::\|record_task_started\|fill_task_root_summary" \
  crates/ --include="*.rs" | grep -v "/tests/" | grep -v "^Binary"
```

Then re-run the 4-agent sweep over the resulting file list.
