# Semantic Behavior Map — `vox-runtime-rn`

Deterministically synthesized from 24 distinct proven-behavior claims (of 24 extracted) across 14 symbols. 1 symbols have an explicit error-path proof; **12 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `VoxRuntimeHandle::requires_suspend_hooks()`  (happy; EXTRACTED)
- [happy] requires_suspend_hooks() returns true for Mobile RuntimeProfile  (crates/vox-runtime-rn/src/lib.rs)
- [happy] requires_suspend_hooks() returns false for Desktop RuntimeProfile  (crates/vox-runtime-rn/src/lib.rs)
- [happy] requires_suspend_hooks() returns true for mobile configuration  (crates/vox-runtime-rn/tests/bridge_integration.rs)
- [happy] requires_suspend_hooks() returns false for desktop configuration  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `VoxRuntimeHandle::data_dir()`  (happy; EXTRACTED)
- [happy] data_dir() returns the data_dir string from VoxConfig  (crates/vox-runtime-rn/src/lib.rs)
- [happy] data_dir() from mobile config returns path appended with /data  (crates/vox-runtime-rn/tests/bridge_integration.rs)
- [happy] data_dir() returns custom data_dir from caller-provided VoxConfig  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `FileJournal::append()`  (error, happy; EXTRACTED)
- [happy] append() successfully writes JournalLine entries to file journal  (crates/vox-runtime-rn/src/lib.rs)
- [error] append() returns FileJournalError::InvalidJson when json field is invalid JSON  (crates/vox-runtime-rn/src/lib.rs)

### `VoxRuntimeHandle::profile()`  (happy; EXTRACTED)
- [happy] profile() returns the RuntimeProfile passed to VoxRuntimeHandle::new()  (crates/vox-runtime-rn/src/lib.rs)
- [happy] profile() from mobile config returns Mobile RuntimeProfile  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `default_desktop_config()`  (happy; EXTRACTED)
- [happy] default_desktop_config() returns VoxConfig with Desktop profile and info log level  (crates/vox-runtime-rn/src/lib.rs)
- [happy] default_desktop_config() returns VoxConfig with Desktop profile  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `default_mobile_config()`  (happy; EXTRACTED)
- [happy] default_mobile_config(root) constructs data_dir as root/data and model_dir as root/models with Mobile profile  (crates/vox-runtime-rn/src/lib.rs)
- [happy] default_mobile_config(root) returns VoxConfig with Mobile profile  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `open_file_journal()`  (happy, invariant; EXTRACTED)
- [happy] open_file_journal() successfully opens a file journal and returns a handle  (crates/vox-runtime-rn/src/lib.rs)
- [invariant] Journal entries persist across file close and re-open of the same journal file  (crates/vox-runtime-rn/src/lib.rs)

### `FileJournal::flush()`  (happy; EXTRACTED)
- [happy] flush() succeeds on an open journal handle without error  (crates/vox-runtime-rn/src/lib.rs)

### `FileJournal::path()`  (happy; EXTRACTED)
- [happy] path() returns the file path passed to open_file_journal()  (crates/vox-runtime-rn/src/lib.rs)

### `FileJournal::replay_all()`  (happy; EXTRACTED)
- [happy] replay_all() returns all appended JournalLine entries with correct json content  (crates/vox-runtime-rn/src/lib.rs)

### `VoxRnError::Display`  (happy; EXTRACTED)
- [happy] VoxRnError::NotInitialized formats to string containing 'not initialized'  (crates/vox-runtime-rn/src/lib.rs)

### `VoxRnError::Internal`  (happy; EXTRACTED)
- [happy] VoxRnError::Internal(msg) formats to string containing the provided message  (crates/vox-runtime-rn/src/lib.rs)

### `VoxRuntimeHandle::log()`  (happy; EXTRACTED)
- [happy] log() accepts any log level string without panicking  (crates/vox-runtime-rn/tests/bridge_integration.rs)

### `VoxRuntimeHandle::model_dir()`  (happy; EXTRACTED)
- [happy] model_dir() from mobile config returns path appended with /models  (crates/vox-runtime-rn/tests/bridge_integration.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`FileJournal::flush()`** — only: _flush() succeeds on an open journal handle without error_
- **`FileJournal::path()`** — only: _path() returns the file path passed to open_file_journal()_
- **`FileJournal::replay_all()`** — only: _replay_all() returns all appended JournalLine entries with correct json content_
- **`VoxRnError::Display`** — only: _VoxRnError::NotInitialized formats to string containing 'not initialized'_
- **`VoxRnError::Internal`** — only: _VoxRnError::Internal(msg) formats to string containing the provided message_
- **`VoxRuntimeHandle::data_dir()`** — only: _data_dir() returns the data_dir string from VoxConfig_
- **`VoxRuntimeHandle::log()`** — only: _log() accepts any log level string without panicking_
- **`VoxRuntimeHandle::model_dir()`** — only: _model_dir() from mobile config returns path appended with /models_
- **`VoxRuntimeHandle::profile()`** — only: _profile() returns the RuntimeProfile passed to VoxRuntimeHandle::new()_
- **`VoxRuntimeHandle::requires_suspend_hooks()`** — only: _requires_suspend_hooks() returns true for Mobile RuntimeProfile_
- **`default_desktop_config()`** — only: _default_desktop_config() returns VoxConfig with Desktop profile and info log level_
- **`default_mobile_config()`** — only: _default_mobile_config(root) constructs data_dir as root/data and model_dir as root/models with Mobile profile_
