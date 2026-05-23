---
title: "CLI vs compilerd params (split-brain backlog)"
description: "Documents the duplicate clap vs serde request structs today and the convergence target."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# CLI vs `compilerd` params

**Today:** [`crates/vox-cli/src/cli_args.rs`](../../../crates/vox-cli/src/cli_args.rs) (clap) and [`crates/vox-cli/src/compilerd.rs`](../../../crates/vox-cli/src/compilerd.rs) (`Deserialize` bodies) mirror overlapping flags/options.

**Target:** one contract-backed shape (or a single Rust struct carrying both `clap::Args` + serde) so CLI argv and daemon JSON cannot drift.

**Guardrails until merge:** `vox ci command-compliance` keeps registry / reachability / MCP wiring aligned; changing either surface requires updating both files deliberately.
