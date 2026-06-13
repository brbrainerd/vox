# Semantic Behavior Map — `vox-audit`

Deterministically synthesized from 176 distinct proven-behavior claims (of 176 extracted) across 66 symbols. 8 symbols have an explicit error-path proof; **19 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `GaSnapshot`  (edge, error, happy; EXTRACTED, INFERRED)
- [happy] When foundation tier gate is red, downstream product-tier gates are marked as blocked_by_foundation  (crates/vox-audit/src/ga.rs)
- [error] When foundation tier gate is red, ga_met evaluates to false  (crates/vox-audit/src/ga.rs)
- [error] When foundation tier gate is red, exit_code is non-zero  (crates/vox-audit/src/ga.rs)
- [happy] When all gates pass, foundation_red evaluates to false  (crates/vox-audit/src/ga.rs)
- [happy] When all gates pass, ga_met evaluates to true  (crates/vox-audit/src/ga.rs)
- [happy] When all gates pass, exit_code is zero  (crates/vox-audit/src/ga.rs)
- [edge] With non-strict mode, red external_infra gate does not block ga_met  (crates/vox-audit/src/ga.rs)
- [edge] With non-strict mode, red external_infra gate results in exit_code zero  (crates/vox-audit/src/ga.rs)
- [error] When foundation tier gate is red in strict mode, all downstream gates are marked blocked_by_foundation and GA fails  (crates/vox-audit/src/ga.rs)
- [happy] When all gates pass, GaSnapshot.ga_met is true and exit_code is 0  (crates/vox-audit/src/ga.rs)
- [happy] In non-strict mode, external_infra red gates do not fail GA (exit_code remains 0)  (crates/vox-audit/src/ga.rs)
- [edge] external_infra gates that are red do not block the build in non-strict mode  (crates/vox-audit/src/ga.rs)
- … +7 more claims

### `Report`  (edge, happy, invariant; EXTRACTED)
- [happy] Aggregating empty events yields a report with schema_version of 1 and zero counts for all metrics  (crates/vox-audit/src/aggregator.rs)
- [happy] Report counts total lint findings across all rules and ranks diagnostics by finding count  (crates/vox-audit/src/aggregator.rs)
- [edge] Report.top_50_diagnostics caps at 50 entries even with more than 50 distinct rules  (crates/vox-audit/src/aggregator.rs)
- [happy] autofix_applied_rate is None when there are lint findings but no autofix observations  (crates/vox-audit/src/aggregator.rs)
- [edge] Unknown autofix outcomes increment total_autofix_observations but do not contribute to rate calculation  (crates/vox-audit/src/aggregator.rs)
- [happy] Report accurately counts repair outcome states (success, partial, abandoned, infra_error) and computes success_rate  (crates/vox-audit/src/aggregator.rs)
- [edge] Unknown repair outcome states are bucketed into the 'other' field of RepairOutcomeHistogram  (crates/vox-audit/src/aggregator.rs)
- [invariant] RepairAttempt events do not increment total_repair_sessions; only RepairOutcome events count as sessions  (crates/vox-audit/src/aggregator.rs)
- [happy] Report can serialize to JSON and deserialize back to an equal report instance  (crates/vox-audit/src/aggregator.rs)
- [happy] Report buckets events by repository_id in by_repository BTreeMap while maintaining workspace-wide totals  (crates/vox-audit/src/aggregator.rs)
- [happy] Events without repository_id bucket to the unattributed key in by_repository  (crates/vox-audit/src/aggregator.rs)
- [edge] Per-repository top_10_diagnostics caps at 10 entries even with more than 10 distinct rules per repository  (crates/vox-audit/src/aggregator.rs)
- … +7 more claims

### `CrlGate`  (invariant; EXTRACTED)
- [invariant] there are exactly 11 gates: 1 foundation + 9 CR-L + 1 tooling  (crates/vox-audit/src/lib.rs)
- [invariant] There are exactly 11 gates: 1 foundation, 9 L-tier gates, and 1 tooling gate  (crates/vox-audit/src/lib.rs)
- [invariant] CrlGate::all() yields gates in non-decreasing Tier order (foundation before distribution before product)  (crates/vox-audit/src/lib.rs)
- [invariant] registry() and CrlGate::all() yield gates in the same order  (crates/vox-audit/src/lib.rs)
- [invariant] Every CrlGate variant has a corresponding registered Subcommand in registry()  (crates/vox-audit/src/lib.rs)
- [invariant] registry() contains exactly 11 entries: 1 foundation, 9 product-tier, 1 tooling gate  (crates/vox-audit/src/lib.rs)
- [invariant] thing_name() is unique across all CrlGate variants with no collisions  (crates/vox-audit/src/lib.rs)
- [invariant] all() yields gates in non-decreasing Tier order with foundation tier first  (crates/vox-audit/src/lib.rs)
- [invariant] every gate yielded by all() has a corresponding registered subcommand in registry()  (crates/vox-audit/src/lib.rs)

### `aggregate()`  (edge, happy, invariant; EXTRACTED)
- [invariant] aggregate produces deterministic JSON output when called twice with identical inputs  (crates/vox-audit/src/aggregator.rs)
- [happy] aggregate() correctly partitions events into per-repository buckets by repository_id and maintains workspace-wide totals independently  (crates/vox-audit/src/aggregator.rs)
- [happy] aggregate() buckets events without a repository_id to the UNATTRIBUTED_REPO_KEY special bucket  (crates/vox-audit/src/aggregator.rs)
- [edge] Per-repository top diagnostics list in aggregate() output is capped at 10 entries even when more rules are observed  (crates/vox-audit/src/aggregator.rs)
- [invariant] aggregate() output by_repository field serializes with deterministic JSON key ordering (BTreeMap sorted order)  (crates/vox-audit/src/aggregator.rs)
- [happy] aggregate() breaks ties in top diagnostics by rule_id in lexicographic order when counts are equal  (crates/vox-audit/src/aggregator.rs)

### `product_binary_descriptors`  (happy, invariant; EXTRACTED)
- [invariant] product_binary_descriptors() includes descriptors for cr-a1, cr-a2, cr-a4, cr-d3, cr-e1, cr-e2, cr-p1, cr-p2 binaries  (crates/vox-audit/src/ga.rs)
- [invariant] cr-p1 binary descriptor has external_infra flag set to true  (crates/vox-audit/src/ga.rs)
- [invariant] cr-a1 binary descriptor has external_infra flag set to false  (crates/vox-audit/src/ga.rs)
- [invariant] product_binary_descriptors() contains entries for all standard product binary names including cr-a1, cr-a2, cr-a4, cr-d3, cr-e1, cr-e2, cr-p1, cr-p2  (crates/vox-audit/src/ga.rs)
- [invariant] includes descriptors for all expected product binaries  (crates/vox-audit/src/ga.rs)
- [happy] returns descriptors covering all product binary gates including cr-a1, cr-a2, cr-a4, cr-d3, cr-e1, cr-e2, cr-p1, cr-p2  (crates/vox-audit/src/ga.rs)

### `run_gate`  (invariant; EXTRACTED)
- [invariant] run_gate() emits exactly one AuditRun telemetry event per invocation  (crates/vox-audit/src/lib.rs)
- [invariant] Emitted AuditRun event contains the gate thing name matching the invoked gate  (crates/vox-audit/src/lib.rs)
- [invariant] Emitted AuditRun event outcome field matches the returned RunOutcome report  (crates/vox-audit/src/lib.rs)
- [invariant] Emitted AuditRun event corpus_hash flows from the returned outcome report  (crates/vox-audit/src/lib.rs)
- [invariant] Emitted AuditRun event corpus_size flows from the returned outcome report  (crates/vox-audit/src/lib.rs)
- [invariant] run_gate() produces exactly one AuditRun telemetry event with thing, outcome, duration_seconds, corpus_hash, and corpus_size fields  (crates/vox-audit/src/lib.rs)

### `AuditReport`  (happy, invariant; EXTRACTED)
- [happy] AuditReport infra_error constructor sets thing, incomplete flag, note, and corpus_size correctly and serializes/deserializes through JSON  (crates/vox-audit/src/report.rs)
- [invariant] AuditReport::complete() does not serialize the 'incomplete' field when it is false (default omitted)  (crates/vox-audit/src/report.rs)
- [happy] canonical_report_path() returns path under contracts/reports/ with thing name and .json extension  (crates/vox-audit/src/report.rs)
- [happy] write_json_atomic() creates parent directories and writes AuditReport atomically so it round-trips through JSON  (crates/vox-audit/src/report.rs)
- [happy] AuditReport serializes to JSON and deserializes correctly preserving thing name, incomplete flag, note, and corpus_size  (crates/vox-audit/src/report.rs)

### `GaSnapshot.exit_code`  (error, happy; EXTRACTED)
- [happy] exit code is non-zero when GA is not met  (crates/vox-audit/src/ga.rs)
- [happy] exit code is zero when all gates are green  (crates/vox-audit/src/ga.rs)
- [error] exit code is non-zero when foundation gate is red  (crates/vox-audit/src/ga.rs)
- [happy] exit code is 0 when all gates pass  (crates/vox-audit/src/ga.rs)
- [happy] exit code is 0 in non-strict mode regardless of external_infra gate status  (crates/vox-audit/src/ga.rs)

### `registry`  (invariant; EXTRACTED)
- [invariant] registry() returns gates in the same order as CrlGate::all()  (crates/vox-audit/src/lib.rs)
- [invariant] Every CrlGate enum variant has a corresponding subcommand in registry()  (crates/vox-audit/src/lib.rs)
- [invariant] Every subcommand in registry() corresponds to a known CrlGate variant  (crates/vox-audit/src/lib.rs)
- [invariant] registry().len() equals CrlGate::all().count()  (crates/vox-audit/src/lib.rs)
- [invariant] returns subcommands in same order and containing same gates as CrlGate::all()  (crates/vox-audit/src/lib.rs)

### `CorpusFeedbackReport`  (happy, invariant; EXTRACTED)
- [invariant] CorpusFeedbackReport omits optional-None fields in JSON serialization for forward compatibility  (crates/vox-audit/src/aggregator.rs)
- [happy] serializes to JSON and deserializes back to an equivalent instance  (crates/vox-audit/src/aggregator.rs)
- [happy] CorpusFeedbackReport serializes to JSON and deserializes back to an identical report via serde_json round-trip  (crates/vox-audit/src/aggregator.rs)
- [happy] Optional-None fields in CorpusFeedbackReport are skipped during JSON serialization (success_rate field omitted when null)  (crates/vox-audit/src/aggregator.rs)

### `JsonlFileRecorder`  (happy; EXTRACTED)
- [happy] Records events to JSONL file and load_events_from_jsonl retrieves them in original order  (crates/vox-audit/src/recorder.rs)
- [happy] Automatically creates parent directories when recording events to a nested path  (crates/vox-audit/src/recorder.rs)
- [happy] JsonlFileRecorder appends multiple events to a JSONL file and preserves order through round-trip  (crates/vox-audit/src/recorder.rs)
- [happy] JsonlFileRecorder lazily creates parent directories when recording to nested paths  (crates/vox-audit/src/recorder.rs)

### `ProtectedPanelClient`  (error, happy; EXTRACTED)
- [happy] Retries on 429 BadStatus error with exponential backoff (2^n multiplier) until success  (crates/vox-audit/src/panel.rs)
- [error] Does not retry on MissingApiKey error; propagates immediately  (crates/vox-audit/src/panel.rs)
- [error] Stops retrying after max_retries attempts and returns final error  (crates/vox-audit/src/panel.rs)
- [error] Does not retry on MalformedResponse error; consumes only one inner response  (crates/vox-audit/src/panel.rs)

### `ReportFormat.parse`  (error, happy; EXTRACTED)
- [happy] ReportFormat.parse correctly parses json string to ReportFormat::Json  (crates/vox-audit/src/report.rs)
- [happy] ReportFormat.parse correctly parses md and markdown strings to ReportFormat::Markdown  (crates/vox-audit/src/report.rs)
- [happy] ReportFormat.parse correctly parses html string to ReportFormat::Html  (crates/vox-audit/src/report.rs)
- [error] ReportFormat.parse returns error for unsupported formats like xml  (crates/vox-audit/src/report.rs)

### `load_events_from_jsonl`  (edge, happy; EXTRACTED)
- [edge] Returns empty vector when JSONL file does not exist instead of erroring  (crates/vox-audit/src/recorder.rs)
- [happy] load_events_from_jsonl correctly deserializes all appended events preserving their order  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_jsonl returns empty collection when file does not exist  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_jsonl silently skips malformed JSON lines and returns only valid events  (crates/vox-audit/src/recorder.rs)

### `load_events_from_jsonl()`  (edge, happy; EXTRACTED)
- [happy] load_events_from_jsonl() round-trips events written by JsonlFileRecorder and preserves order and rule_id  (crates/vox-audit/src/recorder.rs)
- [happy] JsonlFileRecorder creates parent directories lazily when recording and subsequent load succeeds  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_jsonl() returns empty vector when file does not exist  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_jsonl() skips malformed JSON lines and returns only valid events  (crates/vox-audit/src/recorder.rs)

### `CachingPanelClient`  (happy, invariant; EXTRACTED)
- [happy] Repeated calls with same member and prompts serve cached response from disk without calling inner client  (crates/vox-audit/src/panel.rs)
- [happy] Cache key includes user_prompt; different prompts retrieve different cached responses  (crates/vox-audit/src/panel.rs)
- [invariant] When ttl_days=0, cached entries are never expired on subsequent calls  (crates/vox-audit/src/panel.rs)

### `GaSnapshot.ga_met`  (error, happy; EXTRACTED)
- [happy] ga_met is true when all gates pass  (crates/vox-audit/src/ga.rs)
- [error] GA fails when foundation gate is red  (crates/vox-audit/src/ga.rs)
- [happy] all-green gate status results in GA being met  (crates/vox-audit/src/ga.rs)

### `gate_from_name`  (error, happy; EXTRACTED)
- [happy] gate_from_name(gate.thing_name()) round-trips to the original gate for all CrlGate variants  (crates/vox-audit/src/lib.rs)
- [error] gate_from_name() returns None for non-existent gate names  (crates/vox-audit/src/lib.rs)
- [happy] gate_from_name(gate.thing_name()) round-trips to the original gate; returns None for unknown names  (crates/vox-audit/src/lib.rs)

### `product_binary_descriptors()`  (happy; EXTRACTED)
- [happy] product_binary_descriptors contains descriptors for all expected binary names  (crates/vox-audit/src/ga.rs)
- [happy] product_binary_descriptors() includes descriptors for all product binary gates (cr-a1, cr-a2, cr-a4, cr-d3, cr-e1, cr-e2, cr-p1, cr-p2)  (crates/vox-audit/src/ga.rs)
- [happy] product_binary_descriptors() marks cr-p1 as external_infra=true and cr-a1 as external_infra=false  (crates/vox-audit/src/ga.rs)

### `registry()`  (invariant; EXTRACTED)
- [invariant] registry() gates appear in the same order as CrlGate::all()  (crates/vox-audit/src/lib.rs)
- [invariant] every CrlGate has a corresponding registered subcommand  (crates/vox-audit/src/lib.rs)
- [invariant] registry size equals CrlGate::all().count()  (crates/vox-audit/src/lib.rs)

### `BufferedRecorder`  (happy; EXTRACTED)
- [happy] snapshot() returns all recorded events with correct count  (crates/vox-audit/src/recorder.rs)
- [happy] BufferedRecorder.snapshot() returns a snapshot with the recorded events  (crates/vox-audit/src/recorder.rs)

### `CrlGate::all()`  (invariant; EXTRACTED)
- [invariant] CrlGate::all() yields gates in non-decreasing tier order (foundation first)  (crates/vox-audit/src/lib.rs)
- [invariant] CrlGate::all() yields gates in non-decreasing Tier order with Foundation gates before Distribution/Gui/Product/Tooling  (crates/vox-audit/src/lib.rs)

### `GaSnapshot.blocked_by_foundation`  (happy; EXTRACTED)
- [happy] downstream gates are marked blocked_by_foundation when foundation gate is red  (crates/vox-audit/src/ga.rs)
- [happy] downstream gates are marked as blocked when foundation tier gate is red  (crates/vox-audit/src/ga.rs)

### `RepairOutcomeHistogram`  (happy; EXTRACTED)
- [happy] RepairOutcomeHistogram defaults to match the repair_outcomes of an empty report  (crates/vox-audit/src/aggregator.rs)
- [happy] unknown repair outcome states bucket to the other field  (crates/vox-audit/src/aggregator.rs)

### `Report.by_repository`  (edge, happy; EXTRACTED)
- [happy] events are bucketed by repository_id into separate per-repository entries  (crates/vox-audit/src/aggregator.rs)
- [edge] events without repository_id bucket to UNATTRIBUTED_REPO_KEY  (crates/vox-audit/src/aggregator.rs)

### `ReportFormat`  (error, happy; EXTRACTED)
- [happy] ReportFormat::parse() round-trips between string and enum for json, md, markdown, and html  (crates/vox-audit/src/report.rs)
- [error] ReportFormat::parse() returns error for unsupported format string like 'xml'  (crates/vox-audit/src/report.rs)

### `ScriptedPanelClient`  (happy; EXTRACTED)
- [happy] Returns responses in LIFO order from the script vector and errors after script exhaustion  (crates/vox-audit/src/panel.rs)
- [happy] ScriptedPanelClient returns queued responses in LIFO order (last pushed is first returned)  (crates/vox-audit/src/panel.rs)

### `Subcommand`  (invariant; EXTRACTED)
- [invariant] every subcommand in registry() corresponds to a known CrlGate  (crates/vox-audit/src/lib.rs)
- [invariant] Every Subcommand in registry() maps to a valid CrlGate variant  (crates/vox-audit/src/lib.rs)

### `load_events_from_dir`  (edge, happy; EXTRACTED)
- [happy] load_events_from_dir aggregates events from multiple JSONL files in a directory  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_dir returns empty collection when directory does not exist  (crates/vox-audit/src/recorder.rs)

### `load_events_from_dir()`  (edge, happy; EXTRACTED)
- [happy] load_events_from_dir() aggregates events from multiple JSONL files in the directory  (crates/vox-audit/src/recorder.rs)
- [edge] load_events_from_dir() returns empty vector when directory does not exist  (crates/vox-audit/src/recorder.rs)

### `product_binary_descriptors().external_infra`  (happy; EXTRACTED)
- [happy] cr-p1 binary descriptor is marked as external_infra  (crates/vox-audit/src/ga.rs)
- [happy] cr-a1 binary descriptor is not marked as external_infra  (crates/vox-audit/src/ga.rs)

### `AuditReport.canonical_report_path`  (invariant; EXTRACTED)
- [invariant] AuditReport.canonical_report_path returns path starting with contracts/reports/<thing> and ending with .json  (crates/vox-audit/src/report.rs)

### `AuditReport.complete`  (invariant; EXTRACTED)
- [invariant] AuditReport.complete does not serialize the incomplete field when value is false (default)  (crates/vox-audit/src/report.rs)

### `AuditReport.infra_error`  (happy; EXTRACTED)
- [happy] AuditReport.infra_error creates an incomplete report with specified thing name and note  (crates/vox-audit/src/report.rs)

### `BTreeMap serialization`  (happy; EXTRACTED)
- [happy] BTreeMap keys serialize in sorted order in JSON representation  (crates/vox-audit/src/aggregator.rs)

### `BufferedRecorder.snapshot`  (happy; EXTRACTED)
- [happy] BufferedRecorder.snapshot returns a snapshot of all recorded events in their captured order  (crates/vox-audit/src/recorder.rs)

### `CorpusFeedbackReport serialization`  (invariant; EXTRACTED)
- [invariant] CorpusFeedbackReport serializes to JSON via serde_json and deserializes back to identical value; None fields are omitted from JSON  (crates/vox-audit/src/aggregator.rs)

### `CrlGate.thing_name()`  (invariant; EXTRACTED)
- [invariant] all CrlGate thing_name values are unique across all gates  (crates/vox-audit/src/lib.rs)

### `CrlGate::all`  (invariant; EXTRACTED)
- [invariant] CrlGate::all() returns gates in non-decreasing tier order with foundation first  (crates/vox-audit/src/lib.rs)

### `CrlGate::thing_name`  (invariant; EXTRACTED)
- [invariant] All CrlGate thing_name values are unique with no collisions  (crates/vox-audit/src/lib.rs)

### `GaSnapshot non-strict mode`  (happy; EXTRACTED)
- [happy] non-strict GA run exits 0 even when external_infra gate is red  (crates/vox-audit/src/ga.rs)

### `GaSnapshot with foundation_red tier`  (happy; EXTRACTED)
- [happy] GaSnapshot.from_rows() sets foundation_red=true and blocks_by_foundation=true on downstream gates when a Foundation-tier gate is red, with non-zero exit_code and ga_met=false  (crates/vox-audit/src/ga.rs)

### `GaSnapshot.foundation_red`  (happy; EXTRACTED)
- [happy] GaSnapshot detects when a foundation tier gate is red  (crates/vox-audit/src/ga.rs)

### `RepairOutcomeHistogram field population`  (happy; EXTRACTED)
- [happy] aggregate() populates RepairOutcomeHistogram.success, partial, abandoned, and infra_error fields correctly and computes success_rate when canonical outcome states present  (crates/vox-audit/src/aggregator.rs)

### `RepairOutcomeHistogram.other field`  (edge; EXTRACTED)
- [edge] aggregate() buckets unknown repair outcome states into RepairOutcomeHistogram.other field  (crates/vox-audit/src/aggregator.rs)

### `Report autofix observation handling`  (edge; EXTRACTED)
- [edge] aggregate() counts unknown autofix outcomes in total_autofix_observations but excludes them from autofix rate denominator  (crates/vox-audit/src/aggregator.rs)

### `Report autofix rate computation`  (edge; EXTRACTED)
- [edge] aggregate() sets autofix_applied_rate to None when no autofix observation events exist  (crates/vox-audit/src/aggregator.rs)

### `Report by_repository unattributed bucket`  (edge; EXTRACTED)
- [edge] aggregate() assigns events without repository_id to UNATTRIBUTED_REPO_KEY in by_repository  (crates/vox-audit/src/aggregator.rs)

### `Report.by_repository field`  (happy; EXTRACTED)
- [happy] aggregate() partitions events into by_repository BTreeMap keyed by repository_id, with correct per-repo totals, repair_outcomes, and top_10_diagnostics (capped at 10)  (crates/vox-audit/src/aggregator.rs)

### `Report.by_repository top_10_diagnostics field`  (edge; EXTRACTED)
- [edge] aggregate() limits per-repository top_10_diagnostics to maximum 10 entries even when more than 10 distinct rules exist  (crates/vox-audit/src/aggregator.rs)

### `Report.top_50_diagnostics`  (happy; EXTRACTED)
- [happy] diagnostics with equal counts are sorted lexicographically by rule_id  (crates/vox-audit/src/aggregator.rs)

### `Report.top_50_diagnostics field`  (edge; EXTRACTED)
- [edge] aggregate() limits top_50_diagnostics to maximum 50 entries even when more than 50 distinct rules exist  (crates/vox-audit/src/aggregator.rs)

### `Report.total_repair_sessions counting`  (invariant; EXTRACTED)
- [invariant] aggregate() counts only RepairOutcome events toward total_repair_sessions, not RepairAttempt events  (crates/vox-audit/src/aggregator.rs)

### `ScriptedPanelClient.complete`  (error; EXTRACTED)
- [error] ScriptedPanelClient.complete returns error when response queue is exhausted  (crates/vox-audit/src/panel.rs)

### `Success`  (happy; EXTRACTED)
- [happy] repair_outcomes.success counts repair outcome events with final_state=success  (crates/vox-audit/src/aggregator.rs)

### `Tier`  (invariant; INFERRED)
- [invariant] Tier ordering is foundation < distribution < gui < product < tooling  (crates/vox-audit/src/lib.rs)

### `Workspace`  (invariant; EXTRACTED)
- [invariant] workspace_root() returns a directory containing Cargo.toml and AGENTS.md marker files  (crates/vox-audit/src/lib.rs)

### `aggregate function with empty event list`  (happy; EXTRACTED)
- [happy] aggregate() produces Report with schema_version=1 and zero counts (total_lint_findings=0, total_autofix_observations=0, total_repair_sessions=0, empty top_50_diagnostics, default RepairOutcomeHistogram)  (crates/vox-audit/src/aggregator.rs)

### `aggregate function with multiple events`  (happy; EXTRACTED)
- [happy] aggregate() counts total_lint_findings correctly across events and populates top_50_diagnostics sorted by finding_count descending, with rule_id as secondary key  (crates/vox-audit/src/aggregator.rs)

### `aggregate_exit_code`  (invariant; EXTRACTED)
- [invariant] aggregate_exit_code selects the worst-case ExitCode using priority: InvalidInput > InfrastructureError > BarMissed > Ok  (crates/vox-audit/src/lib.rs)

### `by_repository`  (invariant; EXTRACTED)
- [invariant] BTreeMap keys serialize in lexicographically sorted order in JSON  (crates/vox-audit/src/aggregator.rs)

### `by_repository BTreeMap serialization`  (invariant; EXTRACTED)
- [invariant] produces byte-identical JSON across multiple aggregations of the same event set  (crates/vox-audit/src/aggregator.rs)

### `success_rate field`  (invariant; EXTRACTED)
- [invariant] null success_rate fields are omitted from JSON serialization for forward compatibility  (crates/vox-audit/src/aggregator.rs)

### `top_10_diagnostics`  (invariant; EXTRACTED)
- [invariant] per-repository diagnostic list is capped at 10 entries regardless of input count  (crates/vox-audit/src/aggregator.rs)

### `top_50_diagnostics`  (invariant; EXTRACTED)
- [invariant] rules with equal counts are ordered lexicographically by rule_id  (crates/vox-audit/src/aggregator.rs)

### `total_repair_sessions`  (invariant; EXTRACTED)
- [invariant] RepairAttempt events do not increment total_repair_sessions count  (crates/vox-audit/src/aggregator.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AuditReport.infra_error`** — only: _AuditReport.infra_error creates an incomplete report with specified thing name and note_
- **`BTreeMap serialization`** — only: _BTreeMap keys serialize in sorted order in JSON representation_
- **`BufferedRecorder`** — only: _snapshot() returns all recorded events with correct count_
- **`BufferedRecorder.snapshot`** — only: _BufferedRecorder.snapshot returns a snapshot of all recorded events in their captured order_
- **`GaSnapshot non-strict mode`** — only: _non-strict GA run exits 0 even when external_infra gate is red_
- **`GaSnapshot with foundation_red tier`** — only: _GaSnapshot.from_rows() sets foundation_red=true and blocks_by_foundation=true on downstream gates when a Foundation-tier gate is red, with non-zero exit_code and ga_met=false_
- **`GaSnapshot.blocked_by_foundation`** — only: _downstream gates are marked blocked_by_foundation when foundation gate is red_
- **`GaSnapshot.foundation_red`** — only: _GaSnapshot detects when a foundation tier gate is red_
- **`JsonlFileRecorder`** — only: _Records events to JSONL file and load_events_from_jsonl retrieves them in original order_
- **`RepairOutcomeHistogram`** — only: _RepairOutcomeHistogram defaults to match the repair_outcomes of an empty report_
- **`RepairOutcomeHistogram field population`** — only: _aggregate() populates RepairOutcomeHistogram.success, partial, abandoned, and infra_error fields correctly and computes success_rate when canonical outcome states present_
- **`Report.by_repository field`** — only: _aggregate() partitions events into by_repository BTreeMap keyed by repository_id, with correct per-repo totals, repair_outcomes, and top_10_diagnostics (capped at 10)_
- **`Report.top_50_diagnostics`** — only: _diagnostics with equal counts are sorted lexicographically by rule_id_
- **`ScriptedPanelClient`** — only: _Returns responses in LIFO order from the script vector and errors after script exhaustion_
- **`Success`** — only: _repair_outcomes.success counts repair outcome events with final_state=success_
- **`aggregate function with empty event list`** — only: _aggregate() produces Report with schema_version=1 and zero counts (total_lint_findings=0, total_autofix_observations=0, total_repair_sessions=0, empty top_50_diagnostics, default RepairOutcomeHistogram)_
- **`aggregate function with multiple events`** — only: _aggregate() counts total_lint_findings correctly across events and populates top_50_diagnostics sorted by finding_count descending, with rule_id as secondary key_
- **`product_binary_descriptors()`** — only: _product_binary_descriptors contains descriptors for all expected binary names_
- **`product_binary_descriptors().external_infra`** — only: _cr-p1 binary descriptor is marked as external_infra_
