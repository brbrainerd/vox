---
description: How to safely run Cargo tasks inside agent sessions to avoid target locking and deadlocks.
---

# Cargo Lock and Deadlock Prevention
When the AI is asked to run extensive checks, tests, or compiling on Rust workspaces, it should always follow these rules to avoid deadlocking the `cargo` build lock or overwhelming I/O storage:

1. **Do not run overlapping background checks.** Ensure only one `cargo test` or `cargo build` command runs at a time. If the user requests to verify multiple crates interactively, either run them synchronously, queue them, or use `cargo check --workspace` as a single job.
2. **Handle I/O bloat.** If the `target/` directory bloats, it will slow down file indexing drastically and lock `cargo` for minutes while checking `.rmeta` metadata. When running in an agent session, always use `cargo check` instead of `cargo build` when validating syntax and structures, as it generates far fewer artifacts.
3. **Isolate Agent Outputs (Optional).** If testing a newly written sub-project or integration test script within the codebase, explicitly define `$env:CARGO_TARGET_DIR = 'target_agent'` (Windows) before invoking cargo to skip colliding with the user's primary IDE rust-analyzer or their existing build target. Wait until the background command completes and then verify.
4. **Isolate Integration Test Workspaces.** If writing Rust Integration tests that execute `Command::new("cargo")` to compile dynamically generated code, append `\n[workspace]\n` to the generated `Cargo.toml` of the child invocation, AND output to a unique folder. Otherwise, the child `cargo build` will attach to the parent workspace and deadlock waiting for a lock on the `target/` directory which is currently held by the running `cargo test` itself.
