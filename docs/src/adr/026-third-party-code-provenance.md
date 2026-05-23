---
title: "ADR-026: Third-Party Code Provenance Policy"
description: "Governs how Vox ingests, vendors, or studies external open-source code, with explicit AGPL guardrails."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-04-29"
training_eligible: true
---
# ADR 026: Third-Party Code Provenance Policy

## Status
Accepted (2026-04-29)

## Context

Vox is licensed **Apache-2.0**. Several high-value open-source projects (notably
[warpdotdev/warp](https://github.com/warpdotdev/warp) and
[zed-industries/zed](https://github.com/zed-industries/zed)) contain primitives —
fuzzy matchers, B-tree index structures, string-offset utilities — that would
accelerate Vox's LSP, search, and corpus infrastructure.

A systematic review (April 2026) established the following license map:

| Project | Declared license | Vendorable into Vox (Apache-2.0)? |
|---|---|---|
| `warpdotdev/warp` | AGPL-3.0-only | **No** — AGPL and Apache-2.0 are FSF-incompatible for combined works |
| `zed-industries/zed` | Apache-2.0 (most crates) | **Yes** — with attribution |
| crates.io `nucleo-matcher` | MIT | **Yes** — direct cargo dep |
| crates.io `tree-sitter` | MIT | **Yes** — direct cargo dep |

## Decision

### 1. Approved intake paths (in priority order)

1. **Direct `cargo` dependency** — for crates published to crates.io under MIT or
   Apache-2.0. Prefer this over vendoring; version is pinned in workspace
   `Cargo.toml`. Example: `nucleo-matcher`.

2. **Vendor from Apache-2.0 / MIT upstream** — clone a crate from a compatible
   upstream (e.g., Zed's `sum_tree`) into `crates/vox-<name>/`. Preserve the
   upstream `SPDX-License-Identifier` and copyright header in every `.rs` file.
   Add a `# Provenance` section to the crate's `README.md` citing the upstream
   repo, commit SHA, and date.

3. **Clean-room re-implementation** — study AGPL-licensed designs (e.g., Warp's
   `command-signatures-v2`, `input_classifier`) without copying source. Document
   the studied design in `docs/src/architecture/` under the naming pattern
   `*-design-study-2026.md`. No AGPL source text may appear in Vox commits.

### 2. Prohibited intake

- **Any file from a project declared `AGPL-3.0-only`** must not be copied,
  pasted, or vendored into this repository.
- **Git dependencies** pointing at external AGPL repos are also prohibited
  (transitive contamination risk).
- `deny.toml` (cargo-deny) SHOULD be extended to reject AGPL licenses workspace-wide.

### 3. Attribution requirements for vendored code

Every vendored crate MUST include:

```toml
# In crates/vox-<name>/Cargo.toml
# [package.metadata.provenance]
# upstream = "https://github.com/<org>/<repo>"
# upstream_path = "crates/<name>"
# upstream_commit = "<sha>"
# upstream_license = "Apache-2.0"
# vendored = "2026-04-29"
```

And the crate root `src/lib.rs` MUST open with:

```rust
// Originally from <org>/<repo> (<upstream_path>), Apache-2.0.
// Upstream commit: <sha>. Vendored: 2026-04-29.
// Local modifications: <brief description or "none">.
```

### 4. Ongoing compliance

- Run `cargo deny check licenses` in CI (`vox ci` gate) to reject new AGPL
  transitive deps automatically.
- When updating a vendored crate, update the provenance metadata and commit SHA.
- The authoritative list of all vendored crates lives in
  `docs/src/architecture/vendored-crates-registry-2026.md` (to be created when
  the first crate is vendored).

## Consequences

- **Warp** is exclusively a **design reference**. Its ideas are freely studied;
  its source is off-limits.
- **Zed** crates are the preferred Apache-2.0 source for B-tree / text primitives.
- The `fuzzy-search` feature in `vox-cli` uses `nucleo-matcher` (MIT, crates.io)
  — this is compliant under path 1 above.
- Future `vox-exec-grammar` (AST command validator) is a clean-room
  re-implementation; Warp's `command-signatures-v2` is only a design reference.

## Related

- `deny.toml` — cargo-deny license policy
- [Warp research synthesis](../architecture/warp-research-findings-2026.md)
- AGENTS.md §Cryptography Policy (analogous purity requirement for crypto deps)
