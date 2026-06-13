# Semantic Behavior Map — `vox-runtime`

Deterministically synthesized from 41 distinct proven-behavior claims (of 41 extracted) across 26 symbols. 0 symbols have an explicit error-path proof; **21 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `VoxConfig`  (happy, invariant; EXTRACTED)
- [happy] VoxConfig::desktop() sets log_level to 'info'  (crates/vox-runtime/src/config.rs)
- [happy] VoxConfig::desktop() sets data_dir to end with 'data' subdirectory  (crates/vox-runtime/src/config.rs)
- [happy] VoxConfig::desktop() sets model_dir to end with 'models' subdirectory  (crates/vox-runtime/src/config.rs)
- [happy] VoxConfig::mobile() appends '/data' to the provided data_dir parameter  (crates/vox-runtime/src/config.rs)
- [happy] VoxConfig::mobile() appends '/models' to the provided data_dir parameter  (crates/vox-runtime/src/config.rs)
- [invariant] VoxConfig survives serialize/deserialize round-trip through serde_json  (crates/vox-runtime/src/config.rs)

### `RuntimeProfile::Desktop in integrated config`  (happy; EXTRACTED)
- [happy] Journal strategy is Periodic with 5000ms interval when obtained from desktop config  (crates/vox-runtime/tests/profile_integration.rs)
- [happy] Model loading strategy is Eager when obtained from desktop config  (crates/vox-runtime/tests/profile_integration.rs)
- [happy] Does not require suspend hooks when obtained from desktop config  (crates/vox-runtime/tests/profile_integration.rs)

### `RuntimeProfile::Mobile in integrated config`  (happy; EXTRACTED)
- [happy] Journal strategy is OnLifecycle when obtained from mobile config  (crates/vox-runtime/tests/profile_integration.rs)
- [happy] Model loading strategy is Lazy with unload_on_memory_pressure: true when obtained from mobile config  (crates/vox-runtime/tests/profile_integration.rs)
- [happy] Requires suspend hooks when obtained from mobile config  (crates/vox-runtime/tests/profile_integration.rs)

### `SuspendDeadline::mobile_default()`  (happy; EXTRACTED)
- [happy] mobile_default() returns a Strict SuspendDeadline variant  (crates/vox-runtime/src/lifecycle.rs)
- [happy] Provides a valid deadline that can be passed to suspend operations  (crates/vox-runtime/tests/profile_integration.rs)
- [happy] Returns variant matching Strict pattern  (crates/vox-runtime/tests/profile_integration.rs)

### `RuntimeProfile::default_scheduler_threads()`  (happy; EXTRACTED)
- [happy] Desktop profile returns SchedulerThreads::Auto for default_scheduler_threads()  (crates/vox-runtime/src/profile.rs)
- [happy] Mobile profile returns SchedulerThreads::Single for default_scheduler_threads()  (crates/vox-runtime/src/profile.rs)

### `SuspendDeadline::desktop_default()`  (happy; EXTRACTED)
- [happy] desktop_default() returns an Advisory SuspendDeadline variant  (crates/vox-runtime/src/lifecycle.rs)
- [happy] Returns variant matching Advisory pattern  (crates/vox-runtime/tests/profile_integration.rs)

### `VoxConfig::desktop()`  (happy; EXTRACTED)
- [happy] VoxConfig::desktop() constructs a config with RuntimeProfile::Desktop  (crates/vox-runtime/src/config.rs)
- [happy] Returns configuration with RuntimeProfile::Desktop variant  (crates/vox-runtime/tests/profile_integration.rs)

### `VoxConfig::mobile()`  (happy; EXTRACTED)
- [happy] VoxConfig::mobile() constructs a config with RuntimeProfile::Mobile  (crates/vox-runtime/src/config.rs)
- [happy] Returns configuration with RuntimeProfile::Mobile variant  (crates/vox-runtime/tests/profile_integration.rs)

### `IOS_SUSPEND_GRACE`  (invariant; EXTRACTED)
- [invariant] IOS_SUSPEND_GRACE is exactly 30 seconds  (crates/vox-runtime/src/lifecycle.rs)

### `Resumable`  (happy; EXTRACTED)
- [happy] Resumable::resume() can be implemented to restore flushed state  (crates/vox-runtime/src/lifecycle.rs)

### `Resumable::resume()`  (happy; EXTRACTED)
- [happy] Executes successfully and restores previously suspended state when called after suspend  (crates/vox-runtime/tests/profile_integration.rs)

### `RuntimeProfile`  (happy; EXTRACTED)
- [happy] RuntimeProfile::default() returns Desktop variant  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile serde round-trip`  (invariant; EXTRACTED)
- [invariant] Survives JSON serialization and deserialization without mutation for all variants  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Desktop serde serialization`  (happy; EXTRACTED)
- [happy] Serializes to lowercase JSON string "desktop"  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Desktop.model_loading_strategy()`  (happy; EXTRACTED)
- [happy] Returns ModelLoadingStrategy::Eager for desktop profile  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Desktop.requires_suspend_hooks()`  (happy; EXTRACTED)
- [happy] Returns false for desktop profile  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Mobile serde serialization`  (happy; EXTRACTED)
- [happy] Serializes to lowercase JSON string "mobile"  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Mobile.journal_flush_strategy()`  (happy; EXTRACTED)
- [happy] Returns JournalFlushStrategy::OnLifecycle for mobile profile  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Mobile.model_loading_strategy()`  (happy; EXTRACTED)
- [happy] Returns ModelLoadingStrategy::Lazy with unload_on_memory_pressure: true for mobile profile  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::Mobile.requires_suspend_hooks()`  (happy; EXTRACTED)
- [happy] Returns true for mobile profile  (crates/vox-runtime/src/profile.rs)

### `RuntimeProfile::journal_flush_strategy()`  (happy; EXTRACTED)
- [happy] Desktop profile returns JournalFlushStrategy::Periodic with 5000ms interval  (crates/vox-runtime/src/profile.rs)

### `SuspendDeadline`  (happy; EXTRACTED)
- [happy] SuspendDeadline::mobile_default() uses DEFAULT_SUSPEND_DEADLINE duration  (crates/vox-runtime/src/lifecycle.rs)

### `SuspendDeadline duration comparison`  (invariant; EXTRACTED)
- [invariant] Advisory deadline duration is strictly greater than strict deadline duration  (crates/vox-runtime/tests/profile_integration.rs)

### `Suspendable`  (happy; EXTRACTED)
- [happy] Suspendable::suspend() can be implemented to flush state without error  (crates/vox-runtime/src/lifecycle.rs)

### `Suspendable::suspend()`  (happy; EXTRACTED)
- [happy] Executes successfully and allows state flush to succeed when called on implementing type  (crates/vox-runtime/tests/profile_integration.rs)

### `VoxConfig::log_level_parsed()`  (edge; EXTRACTED)
- [edge] log_level_parsed() returns tracing::Level::INFO for unrecognized log level strings  (crates/vox-runtime/src/config.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Resumable`** — only: _Resumable::resume() can be implemented to restore flushed state_
- **`Resumable::resume()`** — only: _Executes successfully and restores previously suspended state when called after suspend_
- **`RuntimeProfile`** — only: _RuntimeProfile::default() returns Desktop variant_
- **`RuntimeProfile::Desktop in integrated config`** — only: _Journal strategy is Periodic with 5000ms interval when obtained from desktop config_
- **`RuntimeProfile::Desktop serde serialization`** — only: _Serializes to lowercase JSON string "desktop"_
- **`RuntimeProfile::Desktop.model_loading_strategy()`** — only: _Returns ModelLoadingStrategy::Eager for desktop profile_
- **`RuntimeProfile::Desktop.requires_suspend_hooks()`** — only: _Returns false for desktop profile_
- **`RuntimeProfile::Mobile in integrated config`** — only: _Journal strategy is OnLifecycle when obtained from mobile config_
- **`RuntimeProfile::Mobile serde serialization`** — only: _Serializes to lowercase JSON string "mobile"_
- **`RuntimeProfile::Mobile.journal_flush_strategy()`** — only: _Returns JournalFlushStrategy::OnLifecycle for mobile profile_
- **`RuntimeProfile::Mobile.model_loading_strategy()`** — only: _Returns ModelLoadingStrategy::Lazy with unload_on_memory_pressure: true for mobile profile_
- **`RuntimeProfile::Mobile.requires_suspend_hooks()`** — only: _Returns true for mobile profile_
- **`RuntimeProfile::default_scheduler_threads()`** — only: _Desktop profile returns SchedulerThreads::Auto for default_scheduler_threads()_
- **`RuntimeProfile::journal_flush_strategy()`** — only: _Desktop profile returns JournalFlushStrategy::Periodic with 5000ms interval_
- **`SuspendDeadline`** — only: _SuspendDeadline::mobile_default() uses DEFAULT_SUSPEND_DEADLINE duration_
- **`SuspendDeadline::desktop_default()`** — only: _desktop_default() returns an Advisory SuspendDeadline variant_
- **`SuspendDeadline::mobile_default()`** — only: _mobile_default() returns a Strict SuspendDeadline variant_
- **`Suspendable`** — only: _Suspendable::suspend() can be implemented to flush state without error_
- **`Suspendable::suspend()`** — only: _Executes successfully and allows state flush to succeed when called on implementing type_
- **`VoxConfig::desktop()`** — only: _VoxConfig::desktop() constructs a config with RuntimeProfile::Desktop_
- **`VoxConfig::mobile()`** — only: _VoxConfig::mobile() constructs a config with RuntimeProfile::Mobile_
