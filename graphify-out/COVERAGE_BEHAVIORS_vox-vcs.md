# Semantic Behavior Map — `vox-vcs`

Deterministically synthesized from 29 distinct proven-behavior claims (of 29 extracted) across 23 symbols. 2 symbols have an explicit error-path proof; **19 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `CasFallback::snapshot`  (happy, invariant; EXTRACTED)
- [happy] snapshot() preserves all provided file paths and label exactly in order, retrievable via changes()  (crates/vox-vcs/src/cas_fallback.rs)
- [invariant] snapshot() IDs are monotonically increasing; undo() does not decrement next_id, leaving gaps in the sequence  (crates/vox-vcs/src/cas_fallback.rs)

### `CasFallback::undo`  (error, happy; EXTRACTED)
- [happy] Calling undo() removes the last snapshot, reducing the change count by one  (crates/vox-vcs/src/cas_fallback.rs)
- [error] Calling undo() on an empty backend returns VcsError::NothingToUndo  (crates/vox-vcs/src/cas_fallback.rs)

### `JjActor`  (error, invariant; EXTRACTED)
- [error] When an internal jj operation panics, the actor returns Err(VcsError::Unavailable) instead of crashing  (crates/vox-vcs/src/jj_actor.rs)
- [invariant] After a panicking operation, the actor remains alive and can service subsequent requests successfully  (crates/vox-vcs/src/jj_actor.rs)

### `JjBackend::changes`  (happy; EXTRACTED)
- [happy] reopened workspace exposes changes made before reopen by including them in the change log  (crates/vox-vcs/src/jj_backend.rs)
- [happy] after undo, the current head operation ID changes to the restored operation  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::snapshot`  (happy; EXTRACTED)
- [happy] snapshot() method does not panic with nested-runtime error when called inside a tokio runtime  (crates/vox-vcs/src/jj_backend.rs)
- [happy] snapshot() executed via JjBackend produces a ChangeId that appears in the result of changes()  (crates/vox-vcs/src/jj_backend.rs)

### `with_op_timeout`  (happy; EXTRACTED)
- [happy] with_op_timeout() returns VcsError::Unavailable with 'timed out' message when operation exceeds the timeout duration  (crates/vox-vcs/src/jj_actor.rs)
- [happy] with_op_timeout() allows fast operations to complete and return their result without timeout  (crates/vox-vcs/src/jj_actor.rs)

### `CasFallback::conflicts`  (happy; EXTRACTED)
- [happy] conflicts() returns an empty list in CasFallback (no content store)  (crates/vox-vcs/src/cas_fallback.rs)

### `CasFallback::diff`  (happy; EXTRACTED)
- [happy] diff() returns empty changed_paths in CasFallback (no content store)  (crates/vox-vcs/src/cas_fallback.rs)

### `CasFallback::resolve`  (happy; EXTRACTED)
- [happy] resolve() succeeds as a no-op in CasFallback when there are no actual conflicts  (crates/vox-vcs/src/cas_fallback.rs)

### `ChangeId`  (happy; EXTRACTED)
- [happy] ChangeId displays as 'chg-' prefix followed by zero-padded ID number  (crates/vox-vcs/src/types.rs)

### `Conflict`  (happy; EXTRACTED)
- [happy] conflict sides carry both divergent contents as readable data strings  (crates/vox-vcs/src/jj_backend.rs)

### `JjActor::create_branch`  (happy; EXTRACTED)
- [happy] create_branch() succeeds end-to-end when called through the actor handle  (crates/vox-vcs/src/jj_actor.rs)

### `JjActor::shutdown`  (happy; EXTRACTED)
- [happy] After shutdown() is called, subsequent operations on the actor handle return Err, never hang  (crates/vox-vcs/src/jj_actor.rs)

### `JjActor::snapshot`  (happy; EXTRACTED)
- [happy] snapshot() executed via actor handle produces a ChangeId that appears in the result of changes()  (crates/vox-vcs/src/jj_actor.rs)

### `JjActorHandle`  (invariant; EXTRACTED)
- [invariant] JjActorHandle and its futures are Send and can be moved into tokio::spawn tasks  (crates/vox-vcs/src/jj_actor.rs)

### `JjBackend`  (happy; EXTRACTED)
- [happy] JjBackend::open succeeds inside an existing tokio runtime without panicking on nested runtime initialization  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::conflicts`  (happy; EXTRACTED)
- [happy] conflicts are materialized as readable data containing both divergent edits, not blocking errors  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::create_branch`  (happy; EXTRACTED)
- [happy] create_branch sets a local bookmark at the current change, visible in the repo view  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::diff`  (happy; EXTRACTED)
- [happy] diff() lists modified files in the changed_paths field between two snapshots  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::open`  (happy; EXTRACTED)
- [happy] open() on an already-colocated repo loads the existing workspace rather than re-initializing  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::push`  (happy; EXTRACTED)
- [happy] push() successfully pushes a bookmark to a local bare git repository  (crates/vox-vcs/src/jj_backend.rs)

### `JjBackend::undo`  (happy; EXTRACTED)
- [happy] undo() restores the working tree to the previous snapshot state by checking out the restored tree to disk  (crates/vox-vcs/src/jj_backend.rs)

### `detect()`  (happy; EXTRACTED)
- [happy] detect() returns VcsBackendKind::Cas when given the current directory  (crates/vox-vcs/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CasFallback::conflicts`** — only: _conflicts() returns an empty list in CasFallback (no content store)_
- **`CasFallback::diff`** — only: _diff() returns empty changed_paths in CasFallback (no content store)_
- **`CasFallback::resolve`** — only: _resolve() succeeds as a no-op in CasFallback when there are no actual conflicts_
- **`ChangeId`** — only: _ChangeId displays as 'chg-' prefix followed by zero-padded ID number_
- **`Conflict`** — only: _conflict sides carry both divergent contents as readable data strings_
- **`JjActor::create_branch`** — only: _create_branch() succeeds end-to-end when called through the actor handle_
- **`JjActor::shutdown`** — only: _After shutdown() is called, subsequent operations on the actor handle return Err, never hang_
- **`JjActor::snapshot`** — only: _snapshot() executed via actor handle produces a ChangeId that appears in the result of changes()_
- **`JjBackend`** — only: _JjBackend::open succeeds inside an existing tokio runtime without panicking on nested runtime initialization_
- **`JjBackend::changes`** — only: _reopened workspace exposes changes made before reopen by including them in the change log_
- **`JjBackend::conflicts`** — only: _conflicts are materialized as readable data containing both divergent edits, not blocking errors_
- **`JjBackend::create_branch`** — only: _create_branch sets a local bookmark at the current change, visible in the repo view_
- **`JjBackend::diff`** — only: _diff() lists modified files in the changed_paths field between two snapshots_
- **`JjBackend::open`** — only: _open() on an already-colocated repo loads the existing workspace rather than re-initializing_
- **`JjBackend::push`** — only: _push() successfully pushes a bookmark to a local bare git repository_
- **`JjBackend::snapshot`** — only: _snapshot() method does not panic with nested-runtime error when called inside a tokio runtime_
- **`JjBackend::undo`** — only: _undo() restores the working tree to the previous snapshot state by checking out the restored tree to disk_
- **`detect()`** — only: _detect() returns VcsBackendKind::Cas when given the current directory_
- **`with_op_timeout`** — only: _with_op_timeout() returns VcsError::Unavailable with 'timed out' message when operation exceeds the timeout duration_
