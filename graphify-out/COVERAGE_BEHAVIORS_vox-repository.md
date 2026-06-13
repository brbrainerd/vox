# Semantic Behavior Map — `vox-repository`

Deterministically synthesized from 39 distinct proven-behavior claims (of 39 extracted) across 19 symbols. 1 symbols have an explicit error-path proof; **17 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `read_vox_populi_toml()`  (error, happy; EXTRACTED)
- [happy] Parses legacy [mens] section from TOML and extracts control_url field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses legacy [mens] section from TOML and extracts scope_id field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses legacy [mens] section from TOML and extracts advertise_gpu field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses legacy [mens] section from TOML and extracts labels as two-element array  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses canonical [mesh] section and extracts control_url field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses canonical [mesh] section and extracts scope_id field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses canonical [mesh] section and extracts inference_base_url field  (crates/vox-repository/src/populi_toml.rs)
- [happy] When both [mesh] and [mens] sections exist, [mesh] is preferred and [mens] is ignored  (crates/vox-repository/src/populi_toml.rs)
- [error] Returns None when file does not exist  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses [mesh.transport] subsection and extracts tls_cert_path field  (crates/vox-repository/src/populi_toml.rs)
- [happy] Parses [mesh.transport] subsection and extracts tls_min_version field  (crates/vox-repository/src/populi_toml.rs)

### `resolve_repo_catalog()`  (happy; EXTRACTED)
- [happy] Resolves exactly one repository from catalog YAML  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Returns repository with display_name matching catalog entry  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Sets resolution_status to resolved_local when catalog entry is successfully resolved  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Computes repository_id from local .git directory when not provided in catalog  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Extracts provider as github from git remote origin URL  (crates/vox-repository/src/repo_catalog/tests.rs)

### `repo_query_text()`  (happy; EXTRACTED)
- [happy] Returns response with result_count matching number of search hits  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Returns response with repositories_queried count  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Groups query hits by repository with display_name field  (crates/vox-repository/src/repo_catalog/tests.rs)
- [happy] Generates trace_id starting with xrepo: prefix  (crates/vox-repository/src/repo_catalog/tests.rs)

### `discover_repository()`  (happy, invariant; EXTRACTED)
- [happy] returns canonicalized repository root path for non-git temporary directory with 16-char stable repository_id  (crates/vox-repository/src/discover.rs)
- [happy] detects Vox.toml file presence and sets capabilities.vox_project flag when found  (crates/vox-repository/src/discover.rs)
- [invariant] returns stable and identical repository_id for repeated discovery calls on same directory  (crates/vox-repository/src/lib.rs)

### `repo_workspace_status_for_cwd()`  (happy; EXTRACTED)
- [happy] Returns RepoWorkspaceStatus with non-empty repository_id  (crates/vox-repository/src/repo_workspace_status.rs)
- [happy] Returns root path canonicalized when compared to input directory  (crates/vox-repository/src/repo_workspace_status.rs)

### `TaskCapabilityHints`  (happy; EXTRACTED)
- [happy] deserializes from JSON with omitted optional fields filled with default values (false for bools, None for Options, empty vec for labels)  (crates/vox-repository/src/capabilities.rs)

### `cargo_workspace_member_dirs()`  (happy; EXTRACTED)
- [happy] expands glob pattern `crates/*` to resolve member subdirectories (alpha, beta) correctly  (crates/vox-repository/src/lib.rs)

### `find_cargo_workspace_root_from()`  (happy; EXTRACTED)
- [happy] Walks up directory tree from nested path to find parent directory containing [workspace] section in Cargo.toml  (crates/vox-repository/src/resolve.rs)

### `find_project_manifest_root()`  (happy; EXTRACTED)
- [happy] Prefers nearest Vox.toml over Cargo.toml when walking up directory tree  (crates/vox-repository/src/resolve.rs)

### `load_agent_scopes()`  (happy; EXTRACTED)
- [happy] successfully reads and parses YAML list format scopes from markdown front matter on disk  (crates/vox-repository/src/agent_scope.rs)

### `merge_agent_capabilities()`  (happy; EXTRACTED)
- [happy] merges config and probed capabilities by preferring probed CPU cores/arch while keeping config GPU settings and combining labels  (crates/vox-repository/src/capabilities.rs)

### `node_workspace_packages()`  (happy; EXTRACTED)
- [happy] expands glob pattern `packages/*` and returns package name/path tuples for discovered packages  (crates/vox-repository/src/lib.rs)

### `parse_scope_from_agent_markdown()`  (happy; EXTRACTED)
- [happy] parses bracketed scope list format `[crates/**, docs/**]` into vector of scope patterns  (crates/vox-repository/src/agent_scope.rs)

### `probe_nvidia_gpu_inventory_best_effort()`  (happy; EXTRACTED)
- [happy] returns None (stub implementation)  (crates/vox-repository/src/gpu_inventory.rs)

### `repo_query_text_with_plane()`  (happy; EXTRACTED)
- [happy] Sets source_plane field in response trace to the provided string value  (crates/vox-repository/src/repo_query_trace.rs)

### `resolve_local_path_under_repo_root()`  (happy; EXTRACTED)
- [happy] resolves existing file path under repository root after canonicalizing both root and file  (crates/vox-repository/src/path_safety.rs)

### `resolve_strict_repo_relative_path()`  (happy; EXTRACTED)
- [happy] builds relative path `src/foo.vox` under repository root without escaping via `..`  (crates/vox-repository/src/path_safety.rs)

### `skill_markdown_filename()`  (happy; EXTRACTED)
- [happy] Returns filename in format {name}.skill.md  (crates/vox-repository/src/skill_scaffold.rs)

### `skill_markdown_for_project()`  (happy; EXTRACTED)
- [happy] Generated skill markdown body contains id field with vox.{name} format  (crates/vox-repository/src/skill_scaffold.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`TaskCapabilityHints`** — only: _deserializes from JSON with omitted optional fields filled with default values (false for bools, None for Options, empty vec for labels)_
- **`cargo_workspace_member_dirs()`** — only: _expands glob pattern `crates/*` to resolve member subdirectories (alpha, beta) correctly_
- **`find_cargo_workspace_root_from()`** — only: _Walks up directory tree from nested path to find parent directory containing [workspace] section in Cargo.toml_
- **`find_project_manifest_root()`** — only: _Prefers nearest Vox.toml over Cargo.toml when walking up directory tree_
- **`load_agent_scopes()`** — only: _successfully reads and parses YAML list format scopes from markdown front matter on disk_
- **`merge_agent_capabilities()`** — only: _merges config and probed capabilities by preferring probed CPU cores/arch while keeping config GPU settings and combining labels_
- **`node_workspace_packages()`** — only: _expands glob pattern `packages/*` and returns package name/path tuples for discovered packages_
- **`parse_scope_from_agent_markdown()`** — only: _parses bracketed scope list format `[crates/**, docs/**]` into vector of scope patterns_
- **`probe_nvidia_gpu_inventory_best_effort()`** — only: _returns None (stub implementation)_
- **`repo_query_text()`** — only: _Returns response with result_count matching number of search hits_
- **`repo_query_text_with_plane()`** — only: _Sets source_plane field in response trace to the provided string value_
- **`repo_workspace_status_for_cwd()`** — only: _Returns RepoWorkspaceStatus with non-empty repository_id_
- **`resolve_local_path_under_repo_root()`** — only: _resolves existing file path under repository root after canonicalizing both root and file_
- **`resolve_repo_catalog()`** — only: _Resolves exactly one repository from catalog YAML_
- **`resolve_strict_repo_relative_path()`** — only: _builds relative path `src/foo.vox` under repository root without escaping via `..`_
- **`skill_markdown_filename()`** — only: _Returns filename in format {name}.skill.md_
- **`skill_markdown_for_project()`** — only: _Generated skill markdown body contains id field with vox.{name} format_
