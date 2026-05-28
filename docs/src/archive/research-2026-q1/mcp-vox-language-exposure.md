---
title: "MCP exposure from the Vox language (SSOT)"
description: "How @mcp.tool maps to shipped MCP surfaces, how this differs from vox-mcp, and how MCP relates to WebSocket, VoxDb, and future zero-wiring goals."
category: "Architecture SSOTs"
status: "current"
sort_order: 45
last_updated: "2026-03-29"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# MCP exposure from the Vox language (SSOT)

This page is the **contributor SSOT** for what “put `@mcp.tool` on Vox code and it is exposed via MCP” means **in this repository today**, how that intersects **WebSocket** and **VoxDb**, and what **roadmap** options exist to reduce manual wiring.

## Claim policy (read this first)

| Statement | True today? | Notes |
| --- | --- | --- |
| `@mcp.tool` on `.vox` source causes the compiler to emit an MCP-capable **stdio JSON-RPC** server for **that generated crate** | **Yes** | See [Generated app path](#generated-app-path-vox--compiler). |
| The same decorator **automatically** registers tools into the shipped **`vox-mcp`** binary every editor uses | **No** | `vox-mcp` uses a **separate** YAML registry and hand-wired Rust; see [First-party vox-mcp path](#first-party-vox-mcp-path). |
| `@mcp.resource` is implemented in the core lexer/parser/codegen | **Yes** | [`@mcp.resource`](../reference/ref-decorators.md): nullary fn, exact URI match; `resources/list` + `resources/read` in generated `mcp_server.rs`. |

If marketing or tutorials imply a single global “drop a decorator and Cursor sees it,” that is **not** accurate until the [Roadmap: delivering the zero-wiring promise](#roadmap-delivering-the-zero-wiring-promise) items land.

## Two MCP surfaces (do not conflate them)

### Generated app path (Vox → compiler)

**Flow:** `.vox` module with `@mcp.tool` → HIR `mcp_tools` → [`emit_mcp_server`](../../../crates/vox-compiler/src/codegen_rust/emit/client.rs) writes `src/mcp_server.rs` when the module is non-empty ([`emit/mod.rs`](../../../crates/vox-compiler/src/codegen_rust/emit/mod.rs)).

**Wire:** JSON-RPC 2.0 over **stdio** (`initialize`, `tools/list`, `tools/call`). Tool **name** is the **Vox function name**; the decorator string is the **description**.

**Scaling:** O(n) in the number of decorated functions inside **one** emitted crate; dispatch is a generated `match`. No central repo-wide registry file is updated.

**Limits today:**

- `inputSchema` is derived from a **small** type map (strings, integers, floats, bools); other types fall back to string-ish behavior in the generator.
- Return values are serialized with `serde_json::to_value` with coarse error surfaces.
- This path is **orthogonal** to Turso/VoxDb unless the generated `lib` already implements DB-backed fns and the MCP entrypoint calls into that same Rust API.

### First-party `vox-mcp` path

**Flow:** Unified operation rows in [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) project to MCP registry output [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml) via `vox ci operations-sync --target mcp --write`; Rust then consumes this through [`vox-mcp-registry`](../../../crates/vox-mcp-registry) → `TOOL_REGISTRY`. The same catalog projects transport-independent **capability ids** / planner metadata to [`contracts/capability/capability-registry.yaml`](../../../contracts/capability/capability-registry.yaml) via `--target capability --write` (see [Capability registry SSOT](capability-registry-ssot.md)); agents can call MCP tool **`vox_capability_model_manifest`** for the merged JSON view. Per-tool behavior lives in [`crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs), JSON Schema in [`input_schemas.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/input_schemas.rs), params in [`params.rs`](../../../crates/vox-orchestrator/src/mcp_tools/params.rs).

**Wire:** **RMCP** stdio server; optional **HTTP + WebSocket** gateway ([`docs/src/reference/cli.md)).

**Scaling:** First-party **registry identity** is one catalog row per operation (MCP + CLI + capability YAML are generated); implementation cost is still **dispatch + schema + handler code** per tool in Rust.

**VoxDb:** Many `vox-mcp` tools receive [`ServerState`](../../../crates/vox-orchestrator/src/mcp_tools/lib.rs) and talk to **Turso / Codex** through orchestrator and DB facades. That is **not** produced by `@mcp.tool` on user `.vox` files; it is **Rust-native** integration.

## How MCP fits next to WebSocket and HTTP

Use the right **framing** for the **latency and session model**:

| Transport (Vox ecosystem) | Typical use | Relationship to MCP |
| --- | --- | --- |
| **MCP stdio** (generated `mcp_server.rs` or `vox-mcp`) | Host process spawns server; request/response tool calls | **Canonical** for “model calls a tool” across editors. |
| **MCP-over-HTTP/WS** (`vox-mcp` gateway) | Remote/mobile clients, same tool catalog as RMCP | Same **tool names/schemas** as stdio; different **transport**. See [MCP HTTP gateway contract](../reference/mcp-http-gateway-contract.md). |
| **OpenClaw WebSocket** (`vox-skills`) | Gateway events, subscriptions, upstream skill catalog | **Interop**, not a replacement for MCP tool naming; bridged via [`openclaw_tools.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/openclaw_tools.rs). |
| **SSE / long-lived app streams** | Incremental UX, executor output | Prefer **stream-native** protocols; do not force MCP tool calls per chunk. |

**Creative SSOT pattern:** Treat **tool name + JSON Schema** as the stable contract. HTTP and WebSocket gateways should **reuse** that contract (they already converge on `tools/list` shapes) instead of inventing parallel per-endpoint JSON.

## How VoxDb fits

**Today:**

- **User Vox apps:** `@table` / `@query` / `@mutation` codegen lives in the same crate as `@mcp.tool` fns; MCP exposure is “call Rust that may call DB,” not “MCP reads the schema catalog directly.”
- **`vox-mcp`:** DB is attached to **process** state (orchestrator + optional Codex); tools like `vox_db_*` are explicit Rust implementations.

**Creative directions (roadmap-friendly):**

1. **Manifest table or JSON artifact:** Emit a versioned **`mcp_surface.json`** (or reuse [`app_contract.json`](../../../crates/vox-compiler/src/codegen_rust/emit/mod.rs) with an `mcp_tools` section) from the compiler so **CI** can diff “what MCP this package exports” without running the binary.
2. **Read models via resources:** When `@mcp.resource` exists, resources could expose **schema snapshots** or **Codex digest** for RAG-style hosts—still **read-optimized**, not a substitute for transactional `@mutation`.
3. **Optional registration:** A future `vox-mcp` **plugin** mode could **merge** manifests from discovered workspace packages into a **dynamic** `tools/list` for power users; policy and auth would need to be stricter than static YAML.

## Agent-to-agent (A2A) and orchestration

- **Mesh/DB/local bus** carry A2A payloads; they are **not** MCP-framed on the wire.
- **MCP** exposes **operator/LLM** controls such as `a2a_send` / `a2a_inbox` ([`crates/vox-orchestrator/src/mcp_tools/a2a.rs`](../../../crates/vox-orchestrator/src/mcp_tools/a2a.rs)); see [`docs/src/reference/cli.md).
- **Creative:** For selected `A2AMessageType`s, define **JSON sub-schemas** shared with MCP tool `inputSchema` so the same **validation** runs at message ingress and at tool boundaries—**SSOT = schema**, transport stays native.

## When **not** to use MCP (even if it is trendy)

- **High-frequency internal queues** (orchestrator dispatch, Populi relay): keep **domain binary/HTTP** semantics and idempotency keys.
- **Large streaming pipelines:** WebSocket/SSE/DeI-style lines beat per-chunk tool calls.
- **Security-sensitive execution:** MCP host allowlists are coarse; mesh workers need **leases, authz, and attestation** (see Populi remote execution ADRs).

## Roadmap: delivering the “no custom wiring” promise

These are **design options**, not all committed work. Pick based on product boundary (user apps vs monorepo `vox-mcp`).

1. **App contract SSOT (shipped):** `app_contract.json` **schema_version 2** includes `mcp_tools` and `mcp_resources` (names, descriptions, signatures) for workspace tooling and docs generation ([`app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs)).
2. **Richer schemas from HIR (partial):** Generated `inputSchema` now maps `list[T]`, tuples, and core scalars; extend for structs, enums, and optional fields.
3. **Merge manifests across packages:** Workspace build produces a **union** of MCP surfaces from multiple packages for discovery.
4. **Reduce triple-write in `vox-mcp`:** CI guard: `yaml_registry_tools_have_dispatch_match_arms` ([`dispatch.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs)); optional codegen for stubs/schemas from `tool-registry.canonical.yaml`.
5. **Optional host integration:** Subprocess or dynamic load so `vox-mcp` can attach **user** MCP servers with namespaced tool IDs without hand-editing YAML.
6. **WebSocket parity tests:** Contract tests that `tools/list` over stdio and over the HTTP gateway **match** for the same server build.

## Related docs and contracts

- [Crate API: vox-mcp](../reference/cli.md) — operational SSOT for the first-party server.
- [@mcp.tool decorator](../reference/ref-decorators.md) — syntax entry (link here for architecture depth).
- [Communication protocols taxonomy](../reference/communication-protocols.md) — MCP vs WS vs SSE.
- [MCP tool registry contract](../reference/mcp-tool-registry-contract.md) — YAML SSOT pointer.
- [VoxDB connection policy (SSOT)](voxdb-connect-policy.md) — where DB belongs in the stack.


