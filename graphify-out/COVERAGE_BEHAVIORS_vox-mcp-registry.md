## Semantic Behavior Map — `vox-mcp-registry`

Synthesized from 6 extracted Behavior claims (5 distinct after dedup) spanning 2 smoke tests in `crates/vox-mcp-registry/tests/tool_registry_smoke.rs`. All proofs are static assertions over the compiled-in registry tables. The crate is exercised for shape and cross-table consistency (presence, uniqueness, subset containment, field well-formedness) but never for any failure, empty-input, or conflict mode. Coverage profile: happy-path + invariant only; error-path proofs = 0.

### `TOOL_REGISTRY`
- **Proven (happy):** Registry is non-empty and contains MCP tool entries.
- Error-path: none. Edge/invariant: see `TOOL_REGISTRY names`.

### `TOOL_REGISTRY names`
- **Proven (invariant):** All tool names are unique — no duplicates in the live registry.
- Error-path: none. The uniqueness check runs against current data only; no planted-duplicate fixture proves the dedup/detection logic actually rejects a collision.

### `SKILL_TOOLS`
- **Proven (invariant):** Every `SKILL_TOOLS` entry resolves to a `TOOL_REGISTRY` entry by name (subset containment).
- Error-path: none. No test for a `SKILL_TOOLS` entry naming a tool absent from the registry (dangling reference).

### `ORCHESTRATOR_TOOLS`
- **Proven (invariant):** Every `ORCHESTRATOR_TOOLS` entry resolves to a `TOOL_REGISTRY` entry by name (subset containment).
- Error-path: none. Same dangling-reference gap as `SKILL_TOOLS`.

### `http_read_role_eligible entries`
- **Proven (happy):** At least one `http_read_role_eligible` tool exists in `TOOL_REGISTRY`.
- **Proven (invariant):** All such tools have non-empty names and non-empty descriptions.
- Error-path: none. No rejection test for an eligible tool with an empty name/description.

## Semantic gaps

Every symbol is proven only on the happy path / current-data invariant; none has a proof that a contract violation is actually caught. The most actionable, ranked by blast radius:

1. **`http_read_role_eligible entries` — role/security surface with no rejection test.** This is the network-exposed allowlist (read-eligible HTTP tools). The well-formedness invariant is asserted against existing data, but nothing proves a malformed eligible tool (empty name or empty description) is rejected. A blank-name eligible tool could silently pass — highest-value gap.
2. **`SKILL_TOOLS` / `ORCHESTRATOR_TOOLS` — dangling-reference failure mode unguarded.** Subset containment is the whole point of these tables, yet there's no negative test proving an entry pointing at a non-existent tool fails. A typo'd or removed tool name would only be caught incidentally.
3. **`TOOL_REGISTRY names` — uniqueness validator with no collision test.** Uniqueness holds on today's data, but the detection path is never exercised against a planted duplicate, so a future collision-detection regression would go unnoticed.

Recommended additions: a fixture (or compile-time table copy) seeding a duplicate name, an unknown-tool reference in each subset list, and an eligible tool with blank name/description — each asserting the registry surfaces an error rather than passing.