---
title: "Version Tracking SSOT Research 2026"
description: "Research and best practices on establishing a single source of truth for versioning across Cargo crates, standard documentation, and compiler IR for the Vox 0.4 language."
category: "Architecture SSOTs"
status: "research"
sort_order: 10
last_updated: "2026-04-16"
training_eligible: false
training_rationale: "Defines the centralized version policy for Vox deliverables, explicitly required to minimize technical debt and conform to AI-pipeline expectations."
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Version Tracking SSOT Research (April 2026)

## Executive Summary
Vox is currently evolving its syntax (v0.4), but technical debt has accumulated via decentralized crate versions and disjointed IR/Documentation versions. Establishing a **Single Source of Truth (SSOT)** is necessary to maintain stability for AI agents and the MENS pipeline. This document synthesizes industry best practices for versioning Cargo workspaces, AST/IR boundaries, and language documentation into an actionable architecture.

## 1. Cargo Crates: Workspace Inheritance
Relying on scattered version numbers across `Cargo.toml` files limits maintainability and increases the risk of SemVer drift. 

**Best Practices for Rust Monorepos:**
- **Workspace-Level SSOT:** The root `[workspace.package]` table must define the base project version (e.g. `version = "0.4.0"` reflecting the syntax), alongside authorship, edition, and repository metadata.
- **Explicit Inheritance:** Every inner crate inside `crates/` must use `version = { workspace = true }` rather than literal strings, and must do the same for `edition`, `license`, and `repository`.
- **CI/CD Enforcement:** Incorporate tools such as `cargo-semver-checks` into the CI pipeline (via `vox ci ssot-drift` or similar tasks) to fail builds when an API-breaking change violates Semantic Versioning. Tools like `cargo-release` automate cross-crate synchronization based on the root state.

**Link Verification:** See the [Vox root `Cargo.toml`](../../Cargo.toml) for where this SSOT lives and must be enforced.

## 2. Abstract Syntax Tree (AST) & Intermediate Representation (IR)
Because Vox is a language designed for AI (specifically training the MENS multi-track model natively), IR and AST outputs must be strictly governed API boundaries. Desyncs here result in hallucinated parsing rules.

**Best Practices for AI-Native Compilers:**
- **Inherent Payload Versions:** Every serialized IR layout (JSON or Binary) should embed its version. Examples include `[Magic Number][Version Header]` for binary formats, or `"$schema": "vox-ir.v0.4.schema.json"` for JSON payload boundaries. Tools parsing IR should strictly refuse mismatched versions rather than corrupting ingestion.
- **Extensible Headers:** Add backwards-compatible extensions via JSON sections or Protocol Buffer optional fields (e.g., adding dynamic diagnostic tokens like `ast_node_kind`).
- **Semantic Hash Keys:** Utilize SHA-256 / Content-Addressable Hashes of the IR payloads embedded within the artifacts. This allows agent-based CI checks to rely on `hash` verification when semantic versioning falls short, preventing subtle structural changes from invalidating downstream models in `crates/vox-eval/`.

## 3. Documentation Version Binding
Documentation must not stray from the codebase implementation, particularly regarding the complex Vox AI-compiler architecture constraints.

**Best Practices:**
- **Code is Truth (API):** For `rustdoc`, use inline `///` documentation comments. By depending on `docs.rs` or standard `cargo doc` compilation, documentation auto-versions exactly identically to the Rust crates.
- **Conceptual Guides (`mdBook`):** Store non-API documentation directly inside the repository (`docs/src/`).
- **Git Tags Integration:** The CI pipeline should align documentation generation strictly against Git tagged releases (e.g., Tag `v0.4.0`). It freezes a formal documentation archive specifically for version 0.4 without disrupting the `main` branch evolution. 
- **Governance:** Ensure code changes are committed within the same PR as their respective `.md` documentation changes to prevent decoupling.

**Link Verification:** Refer to [Documentation Governance](../contributors/documentation-governance.md) and the [Architecture Index](architecture-index.md) to contextualize these documents inside our unified docs pipeline.


