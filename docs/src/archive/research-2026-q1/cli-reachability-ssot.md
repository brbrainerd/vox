---
title: "CLI reachability SSOT"
description: "Stable anchor to reference/cli.md for the top-level command reachability matrix; extend the table when the CLI registry marks reachability_required and run command-compliance."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# CLI reachability SSOT

The **top-level reachability matrix** (`| \`build\` |` …) is authored in **[`reference/cli.md`](../reference/cli.md)** under **CLI command reachability** (content merged from the former `cli-reachability.md`).

When you add a `vox-cli` registry entry with `reachability_required: true`, extend that table in `reference/cli.md` and run **`vox ci command-compliance`**.

This architecture page exists so doc-inventory / SSOT file lists keep a stable anchor; it is not a second copy of the table.

