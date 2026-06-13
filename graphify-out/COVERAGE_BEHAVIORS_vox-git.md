# Semantic Behavior Map — `vox-git`

Deterministically synthesized from 23 distinct proven-behavior claims (of 23 extracted) across 17 symbols. 4 symbols have an explicit error-path proof; **11 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `read_only`  (error, happy, invariant; EXTRACTED)
- [happy] read_only() accepts 'log' subcommand and returns output  (crates/vox-git/src/read_cmd.rs)
- [invariant] read_only() preserves git stdout content unchanged  (crates/vox-git/src/read_cmd.rs)
- [happy] read_only() accepts 'show' subcommand and returns diff output  (crates/vox-git/src/read_cmd.rs)
- [happy] read_only() with 'show' returns filenames and added line content in diff  (crates/vox-git/src/read_cmd.rs)
- [error] read_only() rejects write subcommands (commit, push, init, add) with GitReadError::Disallowed  (crates/vox-git/src/read_cmd.rs)

### `GitBridge::open`  (error, happy; EXTRACTED)
- [happy] GitBridge::open succeeds on a valid repository with .git directory  (crates/vox-git/src/bridge.rs)
- [error] GitBridge::open fails when given a path without a .git directory  (crates/vox-git/src/bridge.rs)

### `ObjectId::parse`  (error, happy; EXTRACTED)
- [happy] parse() accepts a 40-character hexadecimal string  (crates/vox-git/src/object.rs)
- [error] parse() rejects strings shorter than 40 characters  (crates/vox-git/src/object.rs)

### `GitBridge::head_commit_id`  (happy; EXTRACTED)
- [happy] head_commit_id() returns Some(ObjectId) when HEAD ref is present  (crates/vox-git/src/bridge.rs)

### `GitBridge::local_branches`  (happy; EXTRACTED)
- [happy] local_branches() enumerates all branch files in .git/refs/heads  (crates/vox-git/src/bridge.rs)

### `GitBridge::remote_url`  (happy; EXTRACTED)
- [happy] remote_url() parses the configured remote URL from .git/config  (crates/vox-git/src/bridge.rs)

### `GitBridge::repo_path`  (happy; EXTRACTED)
- [happy] repo_path() returns the path provided to open()  (crates/vox-git/src/bridge.rs)

### `GitBridge::sync_status`  (happy; EXTRACTED)
- [happy] sync_status() succeeds even without a complete object database  (crates/vox-git/src/bridge.rs)

### `GitReadError::Disallowed`  (error; EXTRACTED)
- [error] read_only() returns GitReadError::Disallowed when invoked with disallowed write subcommands (commit, push, init, add)  (crates/vox-git/src/read_cmd.rs)

### `ObjectId::short`  (happy; EXTRACTED)
- [happy] short() returns the first 7 hexadecimal characters of the object ID  (crates/vox-git/src/bridge.rs)

### `RefName::as_branch_name`  (happy; EXTRACTED)
- [happy] as_branch_name() extracts the branch name from a RefName  (crates/vox-git/src/bridge.rs)

### `RefName::branch()`  (happy; EXTRACTED)
- [happy] RefName::branch(name) produces refs/heads/{name} format  (crates/vox-git/src/refs.rs)

### `RefName::remote_tracking()`  (happy; EXTRACTED)
- [happy] RefName::remote_tracking(remote, branch) produces refs/remotes/{remote}/{branch} format  (crates/vox-git/src/refs.rs)

### `RefName::tag()`  (happy; EXTRACTED)
- [happy] RefName::tag(name) produces refs/tags/{name} format  (crates/vox-git/src/refs.rs)

### `SyncStatus::ref_diffs`  (happy; EXTRACTED)
- [happy] sync_status returns one SyncStatusRef per local branch  (crates/vox-git/src/bridge.rs)

### `SyncStatusRef::ahead`  (edge; EXTRACTED)
- [edge] ahead defaults to 0 when object database is incomplete  (crates/vox-git/src/bridge.rs)

### `SyncStatusRef::behind`  (edge; EXTRACTED)
- [edge] behind defaults to 0 when object database is incomplete  (crates/vox-git/src/bridge.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`GitBridge::head_commit_id`** — only: _head_commit_id() returns Some(ObjectId) when HEAD ref is present_
- **`GitBridge::local_branches`** — only: _local_branches() enumerates all branch files in .git/refs/heads_
- **`GitBridge::remote_url`** — only: _remote_url() parses the configured remote URL from .git/config_
- **`GitBridge::repo_path`** — only: _repo_path() returns the path provided to open()_
- **`GitBridge::sync_status`** — only: _sync_status() succeeds even without a complete object database_
- **`ObjectId::short`** — only: _short() returns the first 7 hexadecimal characters of the object ID_
- **`RefName::as_branch_name`** — only: _as_branch_name() extracts the branch name from a RefName_
- **`RefName::branch()`** — only: _RefName::branch(name) produces refs/heads/{name} format_
- **`RefName::remote_tracking()`** — only: _RefName::remote_tracking(remote, branch) produces refs/remotes/{remote}/{branch} format_
- **`RefName::tag()`** — only: _RefName::tag(name) produces refs/tags/{name} format_
- **`SyncStatus::ref_diffs`** — only: _sync_status returns one SyncStatusRef per local branch_
