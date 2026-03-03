# Explanation: Security Model

Understand how Vox provides a secure-by-default environment for running AI-generated code and business-critical logic.

## 1. Actor Sandboxing

Every actor in Vox runs in its own isolated memory space.
- **No Shared State**: Actors cannot access the memory of other actors directly.
- **Resource Limits**: The scheduler enforces CPU and memory limits per-actor to prevent Denial-of-Service (DoS) attacks from buggy or malicious code.

## 2. Permission-Based Decorators

Access to sensitive system resources is controlled by decorators.
- **`@server`**: Explicitly markers functions as entry points from the web.
- **`@query`/`@mutation`**: Granular control over database read/write access.
- **`@mcp.tool`**: Controls which internal logic is exposed to external AI agents.

## 3. Data Safety & Type Unification

Vox's type system prevents many common security vulnerabilities:
- **No Null Pointers**: The use of `Option[T]` and `Result[T, E]` eliminates null-pointer dereferences (the "billion-dollar mistake").
- **Injection Prevention**: Database queries generated from `@table` are automatically parameterized, preventing SQL injection.
- **XSS Protection**: The `@component` compiler automatically escapes dynamic content in JSX.

## 4. Secure Communication

- **Encrypted RPC**: All communication between the frontend and backend is encrypted (HTTPS/WSS) by default in production.
- **Signed Journaling**: In high-security environments, the durable execution journal can be signed to prevent tampering.

## 5. Summary

The Vox security model focuses on:
- **Defense in Depth**: Multiple layers of protection (Type System -> Actor Sandbox -> Decorator Permissions).
- **Secure by Default**: Safe patterns are the easiest to write.
- **AI-Safety**: Built-in gates to prevent AI agents from performing unauthorized destructive operations.

---

**Related Reference**:
- [Runtime Explanation](expl-runtime.md) — How the scheduler handles actor isolation.
- [How-To: Handle Errors](how-to-error-handling.md) — Robust error management for security.
