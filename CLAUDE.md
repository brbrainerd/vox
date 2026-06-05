---
title: "Claude Code Overlay"
description: "Claude Code-specific instructions and behavior narrowing."
category: "contributor"
status: "current"
training_eligible: true
training_rationale: "Defines Claude-specific rules for interacting with the Vox codebase."
---
# Claude Code Overlay

This project uses `AGENTS.md` as the cross-tool policy surface (required reading first).

## Claude-specific additions

- **Before adding code, consult [`docs/src/architecture/where-things-live.md`](docs/src/architecture/where-things-live.md)** — it's a flat lookup table for "this concept lives in this crate". Saves grep-and-guess. If your concept isn't there, add the row in the same PR. The architectural rules (layers, fan-in, LoC budgets, orphan detection) are in [`docs/src/architecture/layers.toml`](docs/src/architecture/layers.toml) and enforced by `cargo run -p vox-arch-check`.
- If you open a `.vox` file, treat it as Vox language source — not Rust, not TypeScript.
- Honor `// vox:skip` annotations in code blocks; do not validate those against the compiler.
- **Any `.md` you create under `docs/src/` must start with YAML frontmatter** (`title`/`description`/`category`) — write it as you create the file, never as an afterthought. The pre-push doc-pipeline blocks pushes without it. See [`AGENTS.md §Authored Markdown Frontmatter`](AGENTS.md) and the SSOT [`documentation-governance.md`](docs/src/contributors/documentation-governance.md). Prefer making fenced `vox` blocks compile; use `// vox:skip` + a reason only for genuine out-of-file excerpts.
- Do not store project-specific research in your IDE memory; write to `docs/src/architecture/` instead.
- **Automation scripts are `.vox` files.** Do not generate `.ps1`, `.sh`, or `.py` scripts for project automation. Use `vox run scripts/foo.vox` instead. See [`AGENTS.md §VoxScript-First Glue Code`](AGENTS.md).
- **Never run `cargo fmt --all`** (it overflows the Windows command-line limit → `os error 206`). Format the workspace with `vox run scripts/fmt.vox` (check: `VOX_FMT_CHECK=1 vox run scripts/fmt.vox`), or a single crate with `cargo fmt -p <crate>`. See [`AGENTS.md §Formatting Rust (Windows-safe)`](AGENTS.md).

See: [AGENTS.md](AGENTS.md) for full policy.

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
