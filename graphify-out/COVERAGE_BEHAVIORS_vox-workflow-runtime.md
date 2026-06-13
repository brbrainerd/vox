# Semantic Behavior Map — `vox-workflow-runtime`

Deterministically synthesized from 65 distinct proven-behavior claims (of 65 extracted) across 25 symbols. 6 symbols have an explicit error-path proof; **12 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `interpret_workflow_durable()`  (error, happy; EXTRACTED)
- [happy] interpret_workflow_durable() emits WorkflowPatch event with correct change_id, replayed=false, and version/min_supported/max_supported fields on first run  (crates/vox-workflow-runtime/tests/workflow_patch.rs)
- [error] interpret_workflow_durable() returns Err containing 'not found' message when workflow does not exist in HIR  (crates/vox-workflow-runtime/tests/workflow_tracker_tests.rs)
- [happy] interpret_workflow_durable executes a complete workflow with WorkflowStarted as first event  (crates/vox-workflow-runtime/tests/codegen_roundtrip.rs)
- [happy] interpret_workflow_durable completes with WorkflowCompleted as terminal event  (crates/vox-workflow-runtime/tests/codegen_roundtrip.rs)
- [happy] interpret_workflow_durable replays seeded activities as ActivityReplayed events from journal  (crates/vox-workflow-runtime/tests/crash_replay.rs)
- [happy] interpret_workflow_durable does not emit ActivityReplayed events when journal is empty  (crates/vox-workflow-runtime/tests/crash_replay.rs)
- [happy] interpret_workflow_durable terminates with WorkflowCompleted when running fresh against empty journal  (crates/vox-workflow-runtime/tests/crash_replay.rs)
- [happy] interpret_workflow_durable() produces WorkflowStarted as first event and WorkflowCompleted as last event in journal  (crates/vox-workflow-runtime/tests/codegen_roundtrip.rs)
- [happy] interpret_workflow_durable() replays seeded activities as ActivityReplayed event and executes unseeded activities fresh  (crates/vox-workflow-runtime/tests/crash_replay.rs)
- [happy] interpret_workflow_durable() executes all activities fresh when journal is empty (no ActivityReplayed events)  (crates/vox-workflow-runtime/tests/crash_replay.rs)

### `parse_duration_str()`  (error, happy; EXTRACTED)
- [happy] parse_duration_str correctly parses millisecond suffixes (ms) into Duration values  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str correctly parses second suffixes (s) and tolerates whitespace  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str correctly parses minute (m), hour (h), and day (d) suffixes into Duration values  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str treats bare integers without suffix as seconds  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [error] parse_duration_str returns DurationParseError::Empty for empty or whitespace-only input  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str() parses millisecond suffix (ms) and returns Duration with correct millisecond value  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str() parses second suffix (s) including whitespace-padded input and returns Duration with correct second value  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str() parses minute (m), hour (h), and day (d) suffixes and returns Duration with correct time conversion  (crates/vox-workflow-runtime/src/duration_literal.rs)
- [happy] parse_duration_str() interprets bare integer without suffix as seconds  (crates/vox-workflow-runtime/src/duration_literal.rs)

### `journal::execute()`  (error, happy, invariant; EXTRACTED)
- [happy] journal::execute runs the async body and returns its Ok result  (crates/vox-workflow-runtime/tests/journal_execute.rs)
- [happy] journal::execute records exactly one journal entry for successful execution  (crates/vox-workflow-runtime/tests/journal_execute.rs)
- [happy] journal::execute replays seeded value instead of running the body on resume  (crates/vox-workflow-runtime/tests/journal_execute.rs)
- [invariant] journal::execute does not execute the body when replaying from journal  (crates/vox-workflow-runtime/tests/journal_execute.rs)
- [error] journal::execute propagates errors from the body as Err results  (crates/vox-workflow-runtime/tests/journal_execute.rs)
- [invariant] journal::execute does not record entries for failed activity executions  (crates/vox-workflow-runtime/tests/journal_execute.rs)

### `execute_with_retry_logic`  (happy, invariant; EXTRACTED)
- [happy] execute_with_retry_logic retries the closure and records ActivityAttemptFailed and ActivityRetryScheduled events for each attempt until success  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [happy] execute_with_retry_logic returns the successful result from the closure after retry attempts succeed  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [invariant] events recorded by execute_with_retry_logic validate against the workflow-journal.v1.schema.json contract  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [happy] execute_with_retry_logic emits ActivityAttemptFailed events on closure failures  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [happy] execute_with_retry_logic emits ActivityRetryScheduled events between retry attempts  (crates/vox-workflow-runtime/src/workflow/run.rs)

### `VoxDb`  (invariant; EXTRACTED)
- [invariant] VoxDb schema for workflow_activity_log table contains all required replay contract columns: run_id, workflow_name, activity_name, activity_id, status, result_json, recorded_at_ms  (crates/vox-workflow-runtime/tests/workflow_tracker_tests.rs)
- [invariant] VoxDb schema for workflow_run_log table contains all required lease columns: run_id, workflow_name, status, planned_steps, completed_steps, plan_session_id, plan_node_id, plan_version, lease_owner, lease_until_ms, started_at_ms, updated_at_ms  (crates/vox-workflow-runtime/tests/workflow_tracker_tests.rs)
- [invariant] VoxDb schema for workflow_signal_log table contains all required columns: id, run_id, signal_key, payload_json, recorded_at_ms, consumed_at_ms  (crates/vox-workflow-runtime/tests/workflow_tracker_tests.rs)
- [invariant] VoxDb schema for workflow_activity_attempt_log table contains all required columns: run_id, workflow_name, activity_id, attempt_no, status, worker_owner, lease_until_ms, error, recorded_at_ms  (crates/vox-workflow-runtime/tests/workflow_tracker_tests.rs)

### `DurablePromise::pending()`  (error, happy; EXTRACTED)
- [happy] pending promise resolves to the Ok value sent through the oneshot channel  (crates/vox-workflow-runtime/src/durable_promise.rs)
- [error] pending promise propagates JournalError::ActivityFailed from the receiver channel  (crates/vox-workflow-runtime/src/durable_promise.rs)
- [error] pending promise resolves to JournalError::SenderDropped when the sender is dropped  (crates/vox-workflow-runtime/src/durable_promise.rs)

### `derive_activity_id`  (edge, invariant; EXTRACTED)
- [invariant] derive_activity_id produces the same activity_id given the same workflow name, activity name, and position  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [edge] derive_activity_id produces different activity_ids for different position values with same workflow and activity name  (crates/vox-workflow-runtime/src/workflow/run.rs)
- [edge] derive_activity_id produces different activity_ids for different workflow names with same activity name and position  (crates/vox-workflow-runtime/src/workflow/run.rs)

### `FileJournalTracker::on_activity_completed()`  (happy; EXTRACTED)
- [happy] FileJournalTracker records an activity completion and it persists across tracker recreation  (crates/vox-workflow-runtime/src/file_journal.rs)
- [happy] FileJournalTracker can record multiple activity completions sequentially  (crates/vox-workflow-runtime/src/file_journal.rs)

### `InMemoryTracker::load_activity_result()`  (edge, happy; EXTRACTED)
- [happy] InMemoryTracker returns the recorded activity result via load_activity_result  (crates/vox-workflow-runtime/tests/in_memory_tracker.rs)
- [edge] InMemoryTracker returns None for activity IDs that were never recorded  (crates/vox-workflow-runtime/tests/in_memory_tracker.rs)

### `InMemoryTracker::on_activity_completed()`  (happy, invariant; EXTRACTED)
- [happy] InMemoryTracker records an activity completion and can replay it  (crates/vox-workflow-runtime/tests/in_memory_tracker.rs)
- [invariant] InMemoryTracker maintains separate namespaces for different workflows  (crates/vox-workflow-runtime/tests/in_memory_tracker.rs)

### `VoxDbTracker::is_activity_completed()`  (edge, happy; EXTRACTED)
- [happy] VoxDbTracker::is_activity_completed() returns true for activities that have been recorded  (crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs)
- [edge] VoxDbTracker::is_activity_completed() returns false for activities that have not been recorded  (crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs)

### `current_hir_module()`  (happy; EXTRACTED)
- [happy] current_hir_module returns the previously set module with matching function count  (crates/vox-workflow-runtime/tests/hir_context.rs)
- [happy] current_hir_module() returns the module previously set via set_current_hir_module()  (crates/vox-workflow-runtime/tests/hir_context.rs)

### `extract_terminal_return()`  (error, happy; EXTRACTED)
- [happy] extract_terminal_return extracts i64 return value from WorkflowCompleted event in journal  (crates/vox-workflow-runtime/tests/return_extract.rs)
- [error] extract_terminal_return returns Err when return_value field type does not match target type  (crates/vox-workflow-runtime/tests/return_extract.rs)

### `scheduled::start()`  (happy, invariant; EXTRACTED)
- [happy] scheduled::start() runs registered functions at least twice within 180 seconds with 60-second interval  (crates/vox-workflow-runtime/tests/scheduled_basic.rs)
- [invariant] scheduled::start() seeds deadline from persisted next_due_at_ms, not from fresh interval, allowing callback to fire when virtual time advances past persisted deadline  (crates/vox-workflow-runtime/tests/scheduled_basic.rs)

### `DurationParseError::Empty`  (error; EXTRACTED)
- [error] parse_duration_str() returns DurationParseError::Empty when input is empty or only whitespace  (crates/vox-workflow-runtime/src/duration_literal.rs)

### `FileJournalTracker.on_activity_completed()`  (happy; EXTRACTED)
- [happy] on_activity_completed() persists activity result to disk and is_activity_completed() reflects the recorded state on reopening  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker.record_workflow_patch()`  (happy; EXTRACTED)
- [happy] record_workflow_patch() persists patch data and load_workflow_patch() retrieves it after tracker is closed and reopened  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker.suspend()`  (happy; EXTRACTED)
- [happy] suspend() accepts SuspendDeadline parameter and completes successfully on an open journal  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker::is_activity_completed()`  (happy; EXTRACTED)
- [happy] FileJournalTracker reports is_activity_completed correctly for recorded activities  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker::load_activity_result()`  (happy; EXTRACTED)
- [happy] FileJournalTracker can load a previously recorded activity result from disk  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker::record_workflow_patch()`  (happy; EXTRACTED)
- [happy] FileJournalTracker can record workflow patches and they persist across tracker recreation  (crates/vox-workflow-runtime/src/file_journal.rs)

### `FileJournalTracker::suspend()`  (happy; EXTRACTED)
- [happy] FileJournalTracker::suspend() succeeds when called on an open journal  (crates/vox-workflow-runtime/src/file_journal.rs)

### `InMemoryTracker::is_activity_completed()`  (happy; EXTRACTED)
- [happy] InMemoryTracker::is_activity_completed returns false initially and true after recording  (crates/vox-workflow-runtime/tests/in_memory_tracker.rs)

### `VoxDbTracker::load_activity_result()`  (happy; EXTRACTED)
- [happy] VoxDbTracker::load_activity_result() returns Some(json) after activity is recorded  (crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs)

### `VoxDbTracker::on_activity_completed()`  (happy; EXTRACTED)
- [happy] VoxDbTracker::on_activity_completed() records activity result to database and load_activity_result() retrieves the exact JSON value  (crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`FileJournalTracker.on_activity_completed()`** — only: _on_activity_completed() persists activity result to disk and is_activity_completed() reflects the recorded state on reopening_
- **`FileJournalTracker.record_workflow_patch()`** — only: _record_workflow_patch() persists patch data and load_workflow_patch() retrieves it after tracker is closed and reopened_
- **`FileJournalTracker.suspend()`** — only: _suspend() accepts SuspendDeadline parameter and completes successfully on an open journal_
- **`FileJournalTracker::is_activity_completed()`** — only: _FileJournalTracker reports is_activity_completed correctly for recorded activities_
- **`FileJournalTracker::load_activity_result()`** — only: _FileJournalTracker can load a previously recorded activity result from disk_
- **`FileJournalTracker::on_activity_completed()`** — only: _FileJournalTracker records an activity completion and it persists across tracker recreation_
- **`FileJournalTracker::record_workflow_patch()`** — only: _FileJournalTracker can record workflow patches and they persist across tracker recreation_
- **`FileJournalTracker::suspend()`** — only: _FileJournalTracker::suspend() succeeds when called on an open journal_
- **`InMemoryTracker::is_activity_completed()`** — only: _InMemoryTracker::is_activity_completed returns false initially and true after recording_
- **`VoxDbTracker::load_activity_result()`** — only: _VoxDbTracker::load_activity_result() returns Some(json) after activity is recorded_
- **`VoxDbTracker::on_activity_completed()`** — only: _VoxDbTracker::on_activity_completed() records activity result to database and load_activity_result() retrieves the exact JSON value_
- **`current_hir_module()`** — only: _current_hir_module returns the previously set module with matching function count_
