---
title: "Trim, build, and defer (feature lifecycle)"
description: "Official documentation for Trim, build, and defer (feature lifecycle) for the Vox language. Detailed technical reference, architecture gu"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Trim, build, and defer (feature lifecycle)

This policy aligns CLI/MCP/docs SSOT work:

1. **Trim** — Remove or gate command trees and tools that are not reachable from shipped entry points; document the removal in [`cli-reachability.md`](../reference/cli.md) and `ref-cli.md`.
2. **Build** — Wire stubs to real backends or replace with explicit errors and env-gated silent modes (`VOX_SILENT_STUB_*`).
3. **Defer** — Features that stay behind `Cargo` features must list the feature flag in CLI docs and architecture SSOT pages; do not imply they exist in the default minimal binary.

CI guards (`vox ci check-docs-ssot`, `vox ci check-codex-ssot`, doc-inventory verify) catch drift between this policy and the tree.


