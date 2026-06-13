# Semantic Behavior Map — `vox-gui`

Deterministically synthesized from 60 distinct proven-behavior claims (of 60 extracted) across 23 symbols. 3 symbols have an explicit error-path proof; **11 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `locator_for`  (edge, error, happy; EXTRACTED)
- [happy] locator_for() returns a locator with kind='web' when source is 'web'  (crates/vox-gui/src/commands/search.rs)
- [happy] locator_for() returns a locator with value field matching the path argument passed  (crates/vox-gui/src/commands/search.rs)
- [happy] locator_for() returns a locator with kind='memory' when source is 'memory'  (crates/vox-gui/src/commands/search.rs)
- [happy] locator_for() with source='knowledge' returns locator with kind='memory', mapping knowledge source to memory kind  (crates/vox-gui/src/commands/search.rs)
- [happy] locator_for() with source='chunk' returns locator with kind='file'  (crates/vox-gui/src/commands/search.rs)
- [happy] locator_for() with source='repo' returns locator with kind='file'  (crates/vox-gui/src/commands/search.rs)
- [error] locator_for() returns locator with kind='none' when source is unrecognized  (crates/vox-gui/src/commands/search.rs)
- [error] locator_for() returns locator with empty value for unrecognized source  (crates/vox-gui/src/commands/search.rs)
- [edge] locator_for() returns locator with kind='none' when path argument is None  (crates/vox-gui/src/commands/search.rs)

### `glob_match`  (happy, invariant; EXTRACTED)
- [happy] glob_match() returns true when single-segment wildcard pattern matches files in same directory  (crates/vox-gui/src/commands/search.rs)
- [invariant] glob_match() returns false when wildcard would require crossing path separator  (crates/vox-gui/src/commands/search.rs)
- [happy] glob_match() with bare star pattern matches any single segment  (crates/vox-gui/src/commands/search.rs)
- [happy] glob_match() returns true for exact identical path strings  (crates/vox-gui/src/commands/search.rs)
- [happy] glob_match() returns false when paths differ in any component  (crates/vox-gui/src/commands/search.rs)
- [happy] glob_match() with question mark pattern matches exactly one character that is not a path separator  (crates/vox-gui/src/commands/search.rs)
- [invariant] glob_match() question mark does not match path separator character  (crates/vox-gui/src/commands/search.rs)

### `locator_for()`  (edge, happy; EXTRACTED)
- [happy] Returns a locator with kind='web' when passed 'web' source type  (crates/vox-gui/src/commands/search.rs)
- [happy] Returns a locator with kind='memory' when passed 'memory' source type  (crates/vox-gui/src/commands/search.rs)
- [happy] Maps 'knowledge' source type to kind='memory'  (crates/vox-gui/src/commands/search.rs)
- [happy] Maps 'chunk' source type to kind='file'  (crates/vox-gui/src/commands/search.rs)
- [happy] Maps 'repo' source type to kind='file'  (crates/vox-gui/src/commands/search.rs)
- [edge] Unknown source type returns locator with kind='none' and empty value  (crates/vox-gui/src/commands/search.rs)
- [edge] Missing path argument returns locator with kind='none'  (crates/vox-gui/src/commands/search.rs)

### `write_wav_16k_mono()`  (happy, invariant; EXTRACTED, INFERRED)
- [happy] write_wav_16k_mono produces a valid 16 kHz mono 16-bit WAV file from a stereo CaptureBuffer  (crates/vox-gui/src/commands/mic.rs)
- [happy] write_wav_16k_mono proportionally resamples audio from 48 kHz to 16 kHz (48k→16k yields approximately 16000 samples for 1s input)  (crates/vox-gui/src/commands/mic.rs)
- [happy] write_wav_16k_mono accepts real audio captured from cpal device stream and produces usable output  (crates/vox-gui/src/commands/mic.rs)
- [happy] write_wav_16k_mono creates valid 16 kHz mono 16-bit WAV file with TARGET_SAMPLE_RATE  (crates/vox-gui/src/commands/mic.rs)
- [invariant] write_wav_16k_mono creates WAV with exactly 1 channel (mono)  (crates/vox-gui/src/commands/mic.rs)
- [happy] write_wav_16k_mono resamples 48kHz input to 16kHz with proportional sample count reduction (15000-17000 samples for 48000 input)  (crates/vox-gui/src/commands/mic.rs)
- [happy] write_wav_16k_mono creates WAV with TARGET_SAMPLE_RATE and 1 channel from real microphone capture  (crates/vox-gui/src/commands/mic.rs)

### `transcribe_audio_file()`  (happy, invariant; EXTRACTED)
- [happy] transcribe_audio_file returns refined text output that preserves keywords from input (e.g., 'hello' in 'hello world')  (crates/vox-gui/src/commands/mic.rs)
- [happy] transcribe_audio_file processes text passthrough and returns refined text containing input words  (crates/vox-gui/src/commands/mic.rs)
- [invariant] transcribe_audio_file does not panic on valid WAV input  (crates/vox-gui/src/commands/mic.rs)
- [happy] transcribe_audio_file handles real microphone WAV input without crashing  (crates/vox-gui/src/commands/mic.rs)

### `glob_match()`  (happy, invariant; EXTRACTED)
- [happy] Single-segment glob patterns with * match files in the same directory  (crates/vox-gui/src/commands/search.rs)
- [invariant] Single-segment glob patterns with * do not match across directory separators  (crates/vox-gui/src/commands/search.rs)
- [happy] Bare * pattern matches any flat name without directory separators  (crates/vox-gui/src/commands/search.rs)

### `notification_level()`  (happy; EXTRACTED)
- [happy] notification_level maps NotificationType::LevelUp to 'ok' severity  (crates/vox-gui/src/commands/gamify.rs)
- [happy] notification_level maps NotificationType::StreakLost to 'warn' severity  (crates/vox-gui/src/commands/gamify.rs)
- [happy] notification_level maps NotificationType::CompanionStatus to 'info' severity  (crates/vox-gui/src/commands/gamify.rs)

### `LudusProfileDto`  (invariant; EXTRACTED)
- [invariant] LudusProfileDto.xp_progress is bounded between 0.0 and 1.0  (crates/vox-gui/src/commands/gamify.rs)
- [invariant] LudusProfileDto.title is non-empty  (crates/vox-gui/src/commands/gamify.rs)

### `nudge_axis()`  (edge, happy; EXTRACTED)
- [happy] nudge_axis with Promote direction increases axis value and Doubt direction decreases it, respecting floor and ceiling bounds  (crates/vox-gui/src/commands/models.rs)
- [edge] nudge_axis on an unknown axis name returns unchanged priority fields  (crates/vox-gui/src/commands/models.rs)

### `trust_mesh_node()`  (error; EXTRACTED)
- [error] trust_mesh_node returns an error when given a whitespace-only node_id  (crates/vox-gui/src/commands/mesh.rs)
- [error] trust_mesh_node rejects empty/whitespace node_id and returns Err  (crates/vox-gui/src/commands/mesh.rs)

### `untrust_mesh_node()`  (error; EXTRACTED)
- [error] untrust_mesh_node returns an error when given an empty node_id  (crates/vox-gui/src/commands/mesh.rs)
- [error] untrust_mesh_node rejects empty node_id and returns Err  (crates/vox-gui/src/commands/mesh.rs)

### `Action`  (invariant; EXTRACTED)
- [invariant] CLI actions have platform.mobile set to false (not advertised on mobile)  (crates/vox-gui/src/commands/action_manifest.rs)

### `GuiState`  (happy; EXTRACTED)
- [happy] GuiState.initial_view mutex can be locked and holds 'dashboard' string value  (crates/vox-gui/src/commands/app_state.rs)

### `LudusProfileDto::from_profile()`  (happy; EXTRACTED)
- [happy] LudusProfileDto maps user_id, level, and crystals from profile correctly  (crates/vox-gui/src/commands/gamify.rs)

### `PolicyDetailDto`  (happy; EXTRACTED)
- [happy] PolicyDetailDto.from() maps PolicyEntry domain to string format (e.g., CodeAuditRule → 'code-audit-rule') and preserves source detail and protected fields  (crates/vox-gui/src/commands/policy.rs)

### `agent_ids()`  (happy; EXTRACTED)
- [happy] agent_ids RPC succeeds and returns zero agents on freshly-relaunched daemon  (crates/vox-gui/tests/gui_relaunch_smoke.rs)

### `build_action_manifest()`  (happy; EXTRACTED)
- [happy] build_action_manifest returns manifest with at least one CLI action  (crates/vox-gui/src/commands/action_manifest.rs)

### `build_catalog()`  (happy; EXTRACTED)
- [happy] build_catalog() returns non-empty command catalog entries  (crates/vox-gui/tests/gui_relaunch_smoke.rs)

### `build_status_for_branch()`  (edge; EXTRACTED)
- [edge] build_status_for_branch returns status 'not_run' for all requested policy IDs when .vox/policy-status directory is absent  (crates/vox-gui/src/commands/policy.rs)

### `find_repo_root()`  (happy; EXTRACTED)
- [happy] find_repo_root walks up directory hierarchy until it finds contracts/policy/policy-registry.v1.yaml  (crates/vox-gui/src/commands/policy.rs)

### `handle_tool_call()`  (happy; EXTRACTED)
- [happy] handle_tool_call for read-only vox_git_status returns non-error JSON envelope  (crates/vox-gui/tests/mcp_bridge_tests.rs)

### `orchestrator_status()`  (happy; EXTRACTED)
- [happy] orchestrator_status RPC succeeds and returns without error  (crates/vox-gui/tests/gui_relaunch_smoke.rs)

### `parse_worktree_porcelain()`  (happy; EXTRACTED)
- [happy] parse_worktree_porcelain extracts branch names and paths from git porcelain format, marking the first entry as is_current  (crates/vox-gui/src/commands/policy.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`GuiState`** — only: _GuiState.initial_view mutex can be locked and holds 'dashboard' string value_
- **`LudusProfileDto::from_profile()`** — only: _LudusProfileDto maps user_id, level, and crystals from profile correctly_
- **`PolicyDetailDto`** — only: _PolicyDetailDto.from() maps PolicyEntry domain to string format (e.g., CodeAuditRule → 'code-audit-rule') and preserves source detail and protected fields_
- **`agent_ids()`** — only: _agent_ids RPC succeeds and returns zero agents on freshly-relaunched daemon_
- **`build_action_manifest()`** — only: _build_action_manifest returns manifest with at least one CLI action_
- **`build_catalog()`** — only: _build_catalog() returns non-empty command catalog entries_
- **`find_repo_root()`** — only: _find_repo_root walks up directory hierarchy until it finds contracts/policy/policy-registry.v1.yaml_
- **`handle_tool_call()`** — only: _handle_tool_call for read-only vox_git_status returns non-error JSON envelope_
- **`notification_level()`** — only: _notification_level maps NotificationType::LevelUp to 'ok' severity_
- **`orchestrator_status()`** — only: _orchestrator_status RPC succeeds and returns without error_
- **`parse_worktree_porcelain()`** — only: _parse_worktree_porcelain extracts branch names and paths from git porcelain format, marking the first entry as is_current_
