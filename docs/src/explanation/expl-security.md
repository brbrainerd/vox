---
title: "Explanation: Security Model"
description: "Understand the security constructs, permissions, and sandbox boundaries in the Vox ecosystem."
category: "Concepts"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Explanation: Security Model

Vox brings security out of middleware and directly into the language syntax. By enforcing permissions at compile-time and strictly managing secrets from the environment, the language reduces the attack surface for both human-written and AI-authored code.

## 1. vox-secrets for Secret Management

Vox completely rejects decentralized environment variable reading throughout the codebase. You cannot use `std.env.get("STRIPE_KEY")` deep inside business logic.

Instead, all secrets must be declared and managed through **vox-secrets**, Vox's centralized secret manager.

To verify a project's secret posture, you run:
```bash
vox secrets doctor
```
This utility checks the system environment against the `SecretSpec` definition to ensure every required API key, database token, and provider credential is comprehensively mapped and secure, guaranteeing no missing configurations at deploy time.

## 2. The `@require` Precondition

Input validation is not an afterthought; it is a structural precondition. The `@require` decorator evaluates expressions before the function or type instantiation occurs.

```vox
// vox:skip
@mcp.tool "Delete user data"
@require(auth.is_admin(caller))
@endpoint(kind: mutation) fn delete_data(id: Id[User]) to Result[Unit] {
    db.User.delete(id)
    return Ok(())
}
```

If an LLM or user invokes a function that violates a `@require` check, the runtime traps the execution at the capability boundary and immediately returns an error. The unauthorized logic never executes.

## 3. Capability-Gated Execution 

Many operations in Vox execute within a **Capability-Gated System**. A function annotated with the aspirational `@task` or invoked by an LLM via the DEI orchestrator cannot just read arbitrary files or open random sockets. 

Capabilities (network, filesystem, state mutation) are granted down the call graph. If a network call uses the default `std.http.post`, it runs against the global outbound HTTP policies.

## 4. WASI/Sandbox Execution Boundaries

Vox code is sandboxed by default in its compiled representation.
- **Isolates over Threads**: Rather than exposing raw OS thread primitives, Vox utilizes an actor model compiled down to Tokio `mpsc` channels or isolated WASM/WASI modules (depending on the target). 
- **No Shared State**: Execution memory is walled off. Malicious code attempting to manipulate memory pointers is thwarted by the target compiler (Rust) rejecting the unsafe actions.

## 5. Type and Memory Safety

The core type system intrinsically blocks entire classes of errors:
- **No Nulls**: The compiler's absolute enforcement of `Option[T]` and explicit `Result[T, E]` exhaustiveness eliminates unhandled crashes.
- **SQL Injection Prevention**: All `db.*` accessors use strictly verified parameterized queries generated directly by the compiler.
- **XSS Protection**: React Islands hydrate with standard cross-site scripting encodings intact, avoiding raw HTML injection from LLM output.

---

**Related Topics**:
- [Reference: Decorators](../reference/ref-decorators.md)
- [Explanation: The Runtime](expl-runtime.md)
