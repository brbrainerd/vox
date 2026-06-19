---
category: "Telemetry Contracts"
---

# Telemetry Collection Taxonomy — Schema Reference

**SSOT:** `collection-taxonomy.v1.json` (this doc is a human-readable companion; the JSON is authoritative).

**Privacy invariants (from spec §3):** All fields must be `enum | int | bool | hash`. No `string` or `free` type is allowed. Any field not on the allowlist is dropped by `redact_event()` before disk or network. The JSON is parsed at startup with fail-closed behavior: a malformed taxonomy yields an empty allowlist (nothing uploads, never panics).

---

## Categories

### `command_usage` — low privacy tier, upload_default: true

**Signal:** Which CLI commands users actually run, and how they complete.  
**Why safe:** Only the verb + bucketed duration + outcome class. No subcommand args, no flag values, no file paths.  
**OTLP event:** `vox.command`  
**Emit site:** `vox-cli::cli_dispatch::dispatch_cli_inner` (A2 proposed)

| Field | Type | Why safe |
|-------|------|----------|
| `verb` | enum | Fixed list of CLI subcommands |
| `exit_class` | enum | success/user_error/internal_error/cancelled/timeout |
| `duration_bucket` | enum | Coarse wall-time bucket, not a fingerprint |

---

### `skill_activation` — low privacy tier, upload_default: true

**Signal:** Which skills fire, from what trigger, and whether they're accepted.  
**Why safe:** Skill id is salt-hashed (install-salt, never uploaded). No skill content, no prompt text.  
**OTLP event:** `vox.skill`  
**Emit site:** `vox-orchestrator-mcp::chat_tools::build_system_prompt_with_skill:131` (A2 proposed)

| Field | Type | Why safe |
|-------|------|----------|
| `skill_id_hash` | hash | install-salt SHA-256 of skill id; salt never leaves device |
| `trigger_source` | enum | How the skill was activated (pinned/catalog/user_explicit/…) |
| `accepted` | bool | Was it included in the final prompt? |
| `surface` | enum | cli/gui/mcp/api/unknown |

---

### `edit_pattern` — medium privacy tier, upload_default: false (opt-in)

**Signal:** What kinds of edits agents perform most often (op type + file class + size).  
**Why safe:** No filenames, no paths, no content. Extension → coarse class (rust/ts/toml/…).  
**OTLP event:** `vox.edit`  
**Emit site:** `vox-orchestrator-mcp::mcp_client::write_file:108` (A2 proposed)

| Field | Type | Why safe |
|-------|------|----------|
| `op_type` | enum | insert/replace/delete/create/unknown |
| `file_kind` | enum | Extension class only |
| `size_bucket` | enum | Line-count range, not exact count |

---

### `harness_usage` — low privacy tier, upload_default: true

**Signal:** How the agentic harness is used: tool-call mix and session shape.  
**Why safe:** Counts and class buckets only. No tool arguments or outputs.  
**OTLP event:** `vox.harness`  
**Emit site:** `vox-orchestrator-mcp::dispatch::handle_tool_call:29` (A2 proposed)

| Field | Type | Why safe |
|-------|------|----------|
| `tool_call_kind` | enum | Broad class of MCP tool |
| `turns_bucket` | enum | LLM turn count bucket |
| `agents_spawned_bucket` | enum | Subagent count bucket |
| `mode` | enum | interactive/headless/plan/auto/unknown |

---

### `error_surface` — medium privacy tier, upload_default: false (opt-in)

**Signal:** Error class + subsystem cross-user aggregate. Extends existing `errors` category.  
**Why safe:** Class + subsystem enums only. No error messages, no stack traces, no context strings.  
**OTLP event:** `vox.error`  
**Emit site:** `vox-orchestrator-mcp::dispatch::handle_tool_call:240` (A2 proposed)

| Field | Type | Why safe |
|-------|------|----------|
| `error_class` | enum | rate_limited/server_error/transport_error/… |
| `subsystem` | enum | Which layer failed |
| `recoverable` | bool | Was it retried successfully? |

---

### `default_decision` — low privacy tier, upload_default: true

**Signal:** Which tunable constants are in effect and what outcomes they produce. Enables setting empirically-grounded defaults from aggregate data.  
**Why safe:** Decision id + outcome are both enum-only. The `chosen` field uses predefined buckets (never raw numbers). Enums are in the allowlist.  
**OTLP event:** `vox.default_decision`  
**Emit sites:** 12 sites across 5 crates — see `default-decision-sites.csv`

| Field | Type | Why safe |
|-------|------|----------|
| `decision_id` | enum | Which tunable (budget_max_cost_micros / llm_max_concurrent / …) |
| `chosen` | enum | Bucketed chosen value, never the raw number |
| `outcome` | enum | Observed outcome (hit_limit / comfortable / throttled / …) |
| `magnitude_bucket` | int | 0–3 ordinal for relative context |

---

### `model_prompt` — low privacy tier, upload_default: true

**Signal:** Model-Layer (Track F) injection outcomes — which profile variant fired for which model, and how the task went. Feeds the learned per-model prompt registry.  
**Why safe:** All enums/buckets. No raw prompt text, no system prompt content, no user message snippets.  
**OTLP event:** `vox.model_prompt`  
**Emit site:** `vox-orchestrator-mcp::chat_tools::build_system_prompt_with_skill` (F6)

| Field | Type | Why safe |
|-------|------|----------|
| `canonical_model_id` | enum | Canonical family (claude_sonnet_4 / gemini_flash_2 / …) — never the provider alias |
| `profile_variant_id` | enum | none / provisional_v1 / shadowed_v1 / confirmed_v1 / … |
| `task_category` | enum | coding / research / writing / debugging / planning / review / unknown |
| `quality_bucket` | enum | excellent / good / acceptable / poor / failed |

---

## Server-side k-anonymity

All aggregates surfaced from the ClickHouse store are gated at **k ≥ 20** (the `k_anonymity` field in the taxonomy root). A query result with fewer than 20 contributing install_ids is suppressed in the query layer, not stored differently. The raw `events_raw` table is never exposed directly to dashboards.

## Existing categories (migrated in Track E)

These categories exist today in local telemetry and will be routed through the same egress pipeline. Each variant gets an explicit `project_event` arm (no blanket-forward — existing events carry free-form strings):

| Category | Mapped variants |
|----------|----------------|
| `research_metrics` | ResearchMetric (session_id salted-hashed, metadata_json dropped) |
| `model_calls` | ModelCall, SelectionDecision, ModelDiscovery, ModelClassification, ConfidencePromotion, ModelIntent |
| `errors` | Error |
| `build` | BuildSummary, LintFinding |
| `agent_orchestration` | TaskRootSummary, AuditRun, RepairAttempt, RepairOutcome, SubagentDispatch, AiFixture, SearchDispatch |
