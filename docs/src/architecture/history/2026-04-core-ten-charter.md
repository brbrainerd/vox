---
title: "Vox Crate Ledger (v1.0 Frozen Core)"
category: "Architecture SSOTs"
status: "current"
---

# Vox Crate Ledger (v1.0 Frozen Core)

Status: Enforced as of April 2026.

This ledger defines the 10 core crates that constitute the v1.0 release track. All other crates in the workspace are considered experimental, peripheral, or slated for deprecation, and must be feature-gated or frozen.

## Core 10 Crates (v1.0 Track)
These crates are subject to strict API compatibility, documentation, and testing requirements.

1. `vox-compiler` - The core Lexer, Parser, HIR, and Codegen infrastructure.
2. `vox-cli` - The primary developer interface and entry point.
3. `vox-db` - The SQLite/Turso persistence layer and typed client generators.
4. `vox-secrets` - Secret management, resolution, and external token handling.
5. `vox-runtime` - The execution engine for Vox semantics and OOPAV state machines.
6. `vox-orchestrator` - A2A messaging, agent orchestration, and sub-agent task routing.
7. `vox-populi` - The mesh control plane and remote inference coordination.
8. `vox-tensor` (MENS) - The native Rust/Burn machine learning pipeline and QLoRA logic.
9. `vox-search` (Scientia) - The RAG engine, BM25/FTS5, and semantic search layer.
10. `vox-toestub` - The architecture validation and conformance checking tool.

## Peripheral & Deprecated Crates
Any crate not listed in the Core 10 must be:
- Explicitly feature-gated in `vox-cli` and `vox-orchestrator`.
- Considered unstable, subject to breaking changes without notice.
- Migrated out of the default build path.

Examples of peripheral crates: `vox-dei`, `vox-ludus`, `vox-browser`, etc.
