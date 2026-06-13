# Semantic Behavior Map — `vox-journal`

`vox-journal` provides `FileJournal<E>`, an append-only JSON Lines journal with a documented crash-safety durability contract (per-record `fsync` via `sync_data`). The 7 extracted claims cover 5 distinct symbols and prove the crate's roundtrip and replay-resilience behaviors, but **all proofs are success-oriented**: `open()` has genuine edge coverage (malformed and blank lines), while `append()`, `suspend()`, and `replay_all()` are proven happy-path only. None of the documented error variants (`JournalError::Io`, `JournalError::Serde`, `JournalError::Poisoned`, `SuspendError::FlushFailed`) are exercised, and the durability contract that is the crate's reason to exist has no behavioral test.

## `FileJournal::open`
Proven behaviors (4 claims, deduped):
- Opening a fresh file returns `Opened` with an empty `replayed` vec. *(happy)*
- Reopening replays all previously appended entries in order with correct values (roundtrip). *(happy)*
- Opening a file containing malformed JSON lines succeeds and skips them during replay. *(edge)*
- Opening a file with blank lines succeeds and ignores them during replay. *(edge)*

Error path: **none.** Edge/invariant: **yes** (malformed-skip, blank-ignore, order preservation).

## `FileJournal::append`
Proven behaviors:
- Appending entries to an open journal succeeds without error. *(happy)*

Error path: **none.** Edge/invariant: **none.**

## `FileJournal::replay_all`
Proven behaviors:
- Returns entries currently on disk, matching appended entries in order. *(happy)*

Error path: **none.** Edge/invariant: **none.**

## `FileJournal::suspend` (`Suspendable`)
Proven behaviors:
- `suspend(SuspendDeadline::mobile_default())` on an open journal succeeds. *(happy)*

Error path: **none.** Edge/invariant: **none.**

## Semantic gaps

Symbols whose contract has an obvious failure mode proven only on the happy path:

- **`FileJournal::append` — mutator with no failure path.** The most actionable gap. `append` documents both `JournalError::Io` (write/flush/`sync_data` failure) and `JournalError::Poisoned` (writer mutex poisoned by a panicking thread), and its doc comment states the **crash-safety contract**: when `append` returns `Ok`, bytes are on the device and the file contains zero partial lines. The mutex-poison branch is trivially testable (poison the lock from a panicking thread, assert `Err(Poisoned)`); the durability/partial-line invariant — the crate's whole purpose — is entirely unproven.

- **`FileJournal::suspend` — integrity/durability surface with no failure path.** Maps `sync_data` failure to `SuspendError::FlushFailed` and writer-poison to `SuspendError::Other`. Both error branches are untested; only the no-op success case is covered.

- **`FileJournal::replay_all` — read surface with no error path.** Re-opens the file by path on every call, so a missing/deleted file yields `JournalError::Io`. Untested; only the present-and-well-formed case is proven.

- **`FileJournal::open` — partial error coverage.** Edge cases are well covered, but two failure branches are not: the I/O error path (`create_dir_all` / `OpenOptions::open` failure → `JournalError::Io`) and the mid-replay read-error branch that logs a warning and **halts replay via `break`** (truncating the returned entries) — a silent-data-loss behavior worth pinning down.