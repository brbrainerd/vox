---
title: "agent handoff contract 2026"
description: "Automatically added frontmatter for agent handoff contract 2026"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---
# Cross-Agent & Cross-Repo Handoff Contract (2026)

This document defines the canonical Single Source of Truth (SSOT) schema for cross-agent and cross-repository handoffs within the Vox orchestrator architecture.

To prevent context rot, prompt injection, and excessive token usage during agent transitions, raw conversation transcription is strictly forbidden. All handoffs must be serialized explicitly via the structured `.vox/handoffs/` mechanism.

## Storage Location
All active handoffs must be stored in `.vox/handoffs/<session-id>.json`.
Completed or acknowledged handoffs can be archived but should not pollute the active Git worktree. The `.vox/handoffs/` directory is specifically configured in `.voxignore` to be excluded from general RAG ingestion, preventing hallucination loops.

## JSON Schema (v1.0)
The standard context envelope schema must be adhered to explicitly.

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["version", "session_id", "source_agent", "target_agent", "goal", "completed_steps", "pending_blockers"],
  "properties": {
    "version": {
      "type": "string",
      "const": "1.0",
      "description": "Schema version. Must be 1.0."
    },
    "session_id": {
      "type": "string",
      "description": "Unique UUID mapping to the orchestrator plan session."
    },
    "source_agent": {
      "type": "string",
      "description": "The unique AgentId or identifier of the originating agent."
    },
    "target_agent": {
      "type": "string",
      "description": "The target AgentId, role, or repository identifier (if cross-repo)."
    },
    "goal": {
      "type": "string",
      "description": "The exact objective the receiving agent needs to accomplish."
    },
    "completed_steps": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Succinct list of steps already executed and verified by the source agent."
    },
    "pending_blockers": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Specific error messages, missing resources, or logical dependencies blocking progress."
    },
    "relevant_files": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Relative paths to critical files. Maximum 5 files."
    },
    "cryptographic_obo_token": {
      "type": "string",
      "description": "Optional explicitly scoped OBO (On-Behalf-Of) token for authorized execution."
    }
  }
}
```

## Protocol Execution Policy
1. **Serialization**: Before an agent transitions work to another agent or repository, it must synthesize its accomplishments and next steps into the JSON schema defined above.
2. **Transmission**: The handoff artifact is written to `.vox/handoffs/<session-id>.json`.
3. **Resumption**: The target agent (upon spin-up in the target repository or environment) detects the specified `.vox/handoffs/` payload, ingests *only* the contents of the handoff JSON (ignoring the previous conversation), and executes the `goal`.
4. **Ephemerality**: Upon successful resumption, the orchestrator issues a deletion for the handoff artifact to maintain directory hygiene.

## Cross-Repo Handoff Note
When an agent shifts context boundaries (e.g. from `vox` repository to `client_repo`), the handoff payload is used explicitly as the initial context initialization block, minimizing the tokens loaded into the new model context window. Raw conversation logs stay securely housed in the originating repository.

