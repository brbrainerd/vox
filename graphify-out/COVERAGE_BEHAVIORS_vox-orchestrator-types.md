# Semantic Behavior Map — `vox-orchestrator-types`

Deterministically synthesized from 28 distinct proven-behavior claims (of 28 extracted) across 18 symbols. 1 symbols have an explicit error-path proof; **15 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `BranchName::parse`  (error, happy; EXTRACTED)
- [happy] BranchName::parse succeeds for valid agent-style branch names like agent/refactor-cache, feature/cap-types, wip.fix.42  (crates/vox-orchestrator-types/src/vcs_capability.rs)
- [error] BranchName::parse rejects empty strings and names longer than 255 characters with BranchNameError::InvalidLength  (crates/vox-orchestrator-types/src/vcs_capability.rs)
- [error] BranchName::parse rejects names starting with '/' or '-' and names containing '..' with appropriate BranchNameError variants  (crates/vox-orchestrator-types/src/vcs_capability.rs)
- [error] BranchName::parse rejects names containing illegal characters (space, colon, caret, tilde, backslash) with BranchNameError::IllegalChar  (crates/vox-orchestrator-types/src/vcs_capability.rs)

### `PrioritySource::dominates()`  (happy, invariant; EXTRACTED)
- [happy] PrioritySource::Developer.dominates() returns true for both Orchestrator and LearningPolicy variants  (crates/vox-orchestrator-types/src/agent_types/priority_source.rs)
- [happy] PrioritySource::Orchestrator.dominates() returns true for LearningPolicy and false for Developer  (crates/vox-orchestrator-types/src/agent_types/priority_source.rs)
- [happy] PrioritySource::LearningPolicy.dominates() returns true only for itself and false for Orchestrator and Developer  (crates/vox-orchestrator-types/src/agent_types/priority_source.rs)
- [invariant] PrioritySource.dominates() returns true when comparing any variant to itself, establishing reflexivity  (crates/vox-orchestrator-types/src/agent_types/priority_source.rs)

### `AgentId`  (happy; EXTRACTED)
- [happy] AgentId parses from string with or without 'A-' prefix and displays as 'A-' prefixed 2-digit zero-padded format  (crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs)
- [happy] AgentId::Display formats values as 'A-' prefix followed by 2-digit zero-padded number  (crates/vox-orchestrator-types/src/agent_types/ids.rs)

### `MergeOutcome::LockWait`  (happy; EXTRACTED)
- [happy] MergeOutcome::LockWait serializes to JSON and deserializes back preserving path, leader, lease_ms, and leader_lamport fields  (crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs)
- [happy] MergeOutcome::LockWait variant serializes to JSON and deserializes back with all fields (path, leader, lease_ms, leader_lamport) preserved  (crates/vox-orchestrator-types/src/merge_outcome.rs)

### `TaskId`  (happy; EXTRACTED)
- [happy] TaskId parses from string with or without 'T-' prefix and displays as 'T-' prefixed 4-digit zero-padded format  (crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs)
- [happy] TaskId::Display formats values as 'T-' prefix followed by 4-digit zero-padded number  (crates/vox-orchestrator-types/src/agent_types/ids.rs)

### `route_backend_for_chat_route()`  (happy; EXTRACTED)
- [happy] route_backend_for_chat_route() normalizes OpenRouter and Gemini ChatProviderRouteKind variants to their corresponding ChatRouteBackend values  (crates/vox-orchestrator-types/tests/providers_and_routes.rs)
- [happy] route_backend_for_chat_route() resolves multiple ChatProviderRouteKind variants to their correct ChatRouteBackend values including CascadeFallback for HuggingFaceRouter  (crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs)

### `AgentIdGenerator`  (happy; EXTRACTED)
- [happy] AgentIdGenerator::new().next() produces sequential AgentId values starting at 1 and incrementing monotonically  (crates/vox-orchestrator-types/src/agent_types/ids.rs)

### `BranchCreate::mint`  (happy; EXTRACTED)
- [happy] BranchCreate::mint constructs a capability that preserves workspace and parent branch via accessor methods  (crates/vox-orchestrator-types/src/vcs_capability.rs)

### `MergeOutcome::Conflict`  (happy; EXTRACTED)
- [happy] MergeOutcome::Conflict variant serializes to JSON and deserializes back; is_merged() and is_lock_wait() return false  (crates/vox-orchestrator-types/src/merge_outcome.rs)

### `MergeOutcome::Merged`  (happy; EXTRACTED)
- [happy] MergeOutcome::Merged serializes to JSON with outcome field set to 'merged' and deserializes back to the same variant  (crates/vox-orchestrator-types/src/merge_outcome.rs)

### `PrioritySource`  (happy; EXTRACTED)
- [happy] serializes to and deserializes from JSON with snake_case variant names (Developer -> "developer", LearningPolicy -> "learning_policy")  (crates/vox-orchestrator-types/src/agent_types/priority_source.rs)

### `ProviderType::default_backend`  (happy; EXTRACTED)
- [happy] ProviderType::default_backend returns ChatRouteBackend::OpenRouter for ProviderType::OpenRouter and ChatRouteBackend::Ollama for ProviderType::Ollama  (crates/vox-orchestrator-types/tests/providers_and_routes.rs)

### `ProviderType::default_backend()`  (happy; EXTRACTED)
- [happy] ProviderType::default_backend() returns ChatRouteBackend::OpenRouter for OpenRouter and ChatRouteBackend::Ollama for Ollama  (crates/vox-orchestrator-types/tests/providers_and_routes.rs)

### `TaskIdGenerator`  (happy; EXTRACTED)
- [happy] TaskIdGenerator::new().next() produces sequential TaskId values starting at 1 and incrementing monotonically  (crates/vox-orchestrator-types/src/agent_types/ids.rs)

### `WorkingTreeWrite::mint`  (happy; EXTRACTED)
- [happy] WorkingTreeWrite::mint constructs a capability that preserves workspace and branch via accessor methods  (crates/vox-orchestrator-types/src/vcs_capability.rs)

### `WorkspaceId`  (happy; EXTRACTED)
- [happy] WorkspaceId Display trait formats value as W-XXXXXX with zero-padding to 6 digits  (crates/vox-orchestrator-types/src/vcs_capability.rs)

### `backend_telemetry_labels()`  (invariant; EXTRACTED)
- [invariant] backend_telemetry_labels() returns non-empty provider and lane strings for valid ChatRouteBackend values  (crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs)

### `mint_working_tree_write`  (happy; EXTRACTED)
- [happy] mint_working_tree_write function constructs a WorkingTreeWrite capability with correct workspace accessible via accessor  (crates/vox-orchestrator-types/src/vcs_capability.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AgentId`** — only: _AgentId parses from string with or without 'A-' prefix and displays as 'A-' prefixed 2-digit zero-padded format_
- **`AgentIdGenerator`** — only: _AgentIdGenerator::new().next() produces sequential AgentId values starting at 1 and incrementing monotonically_
- **`BranchCreate::mint`** — only: _BranchCreate::mint constructs a capability that preserves workspace and parent branch via accessor methods_
- **`MergeOutcome::Conflict`** — only: _MergeOutcome::Conflict variant serializes to JSON and deserializes back; is_merged() and is_lock_wait() return false_
- **`MergeOutcome::LockWait`** — only: _MergeOutcome::LockWait serializes to JSON and deserializes back preserving path, leader, lease_ms, and leader_lamport fields_
- **`MergeOutcome::Merged`** — only: _MergeOutcome::Merged serializes to JSON with outcome field set to 'merged' and deserializes back to the same variant_
- **`PrioritySource`** — only: _serializes to and deserializes from JSON with snake_case variant names (Developer -> "developer", LearningPolicy -> "learning_policy")_
- **`ProviderType::default_backend`** — only: _ProviderType::default_backend returns ChatRouteBackend::OpenRouter for ProviderType::OpenRouter and ChatRouteBackend::Ollama for ProviderType::Ollama_
- **`ProviderType::default_backend()`** — only: _ProviderType::default_backend() returns ChatRouteBackend::OpenRouter for OpenRouter and ChatRouteBackend::Ollama for Ollama_
- **`TaskId`** — only: _TaskId parses from string with or without 'T-' prefix and displays as 'T-' prefixed 4-digit zero-padded format_
- **`TaskIdGenerator`** — only: _TaskIdGenerator::new().next() produces sequential TaskId values starting at 1 and incrementing monotonically_
- **`WorkingTreeWrite::mint`** — only: _WorkingTreeWrite::mint constructs a capability that preserves workspace and branch via accessor methods_
- **`WorkspaceId`** — only: _WorkspaceId Display trait formats value as W-XXXXXX with zero-padding to 6 digits_
- **`mint_working_tree_write`** — only: _mint_working_tree_write function constructs a WorkingTreeWrite capability with correct workspace accessible via accessor_
- **`route_backend_for_chat_route()`** — only: _route_backend_for_chat_route() normalizes OpenRouter and Gemini ChatProviderRouteKind variants to their corresponding ChatRouteBackend values_
