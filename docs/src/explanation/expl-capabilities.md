---
title: "Explanation: Capabilities"
description: "Understanding the Capability-Gated Execution limits applied to workflows and agents."
category: "Concepts"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Explanation: Capability-Gated Execution

Vox introduces a "Capability-Gated" mechanism inside its runtime. Because Vox orchestrates dynamic AI agent routines, the security model must assume that non-deterministic paths may attempt to invoke sensitive operations.

## The Execution Sandbox

When an Agent evaluates code, or when the orchestrator mounts an untrusted plugin process, it runs within a restrictive sandbox.

### Network Constraints
By default, the global HTTP policy (controlled via `vox-reqwest-defaults`) denies all outbound connections triggered dynamically inside a sandboxed evaluation context unless explicit hostnames have been whitelisted within the project manifest.

### Filesystem Constraints
`std.fs` targets are strictly bounded to the workspace's `%TEMP%` alias and sandboxed virtual roots. If an LLM-invoked execution attempts:

```vox
// vox:skip
std.fs.read("/etc/passwd")?
```

The runtime immediately terminates the WASI execution step with a Capability Violation.

### Database Constraints
All generated data abstractions via Codex are strongly typed. Agents cannot arbitrarily generate direct `db.query("DROP TABLE Users")` SQL statements because the `db.query` raw escape hatch is inherently hidden from the exposed `@mcp.tool` capability domain by default. 

## Upgrading Capabilities

If you require an Agent or task to legitimately reach the outside network or modify sensitive tables, you establish explicit boundary `@mcp.tool` functions that validate inputs using `@require` and encapsulate the permissioned operation securely.

```vox
// vox:skip
@mcp.tool "Upload telemetry data to approved vendor"
@require(auth.is_trusted(caller))
fn upload_telemetry(data: str) to Result[Unit] {
    // This runs in the Trusted context
    let res = std.http.post_json("https://trusted-vendor.com/ingest", data)?
    return Ok(())
}
```

---

**Related Content**:
- [Explanation: Security Model](expl-security.md)
- [How-To: System IO](../how-to/how-to-system-io.md)
