---
title: "Doc inventory verifier (SSOT)"
description: "Official documentation for Doc inventory verifier (SSOT) for the Vox language. Detailed technical reference, architecture guides, and imp"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Doc inventory verifier (SSOT)

The committed machine-readable doc map is **`docs/agents/doc-inventory.json`** (schema v3+).

## Canonical commands

| Action | Command |
|--------|---------|
| Regenerate | `vox ci doc-inventory generate` (fallback: `cargo run -p vox-doc-inventory --bin vox-doc-inventory-generate`; legacy `--bin doc-inventory-generate`). If `doc-inventory.json` is mmap-locked on Windows, use `--output docs/agents/doc-inventory.gen.json` then copy over. |
| CI verify | `vox ci doc-inventory verify` |

**Drift tip:** the scanner walks `crates/`, `docs/`, `scripts/`, etc. A temporary `.py` / `.md` left under those trees changes the next `generate`/`verify` output; remove side files (or regenerate after cleanup) before expecting `verify` to pass.

Implementation: **`crates/vox-doc-inventory`** (Rust). There is **no** supported Python generator path in-tree; the legacy doc-inventory Python helpers were removed — use only the Rust crate and **`vox ci doc-inventory`**.

**Canonical CI entrypoint:** **`vox ci …`** (GitHub Actions often uses `cargo run -p vox-cli --quiet -- ci …` before `vox` is on `PATH`). See [Runner contract](../ci/runner-contract.md) (section *Canonical `vox ci` vs shell scripts*).


