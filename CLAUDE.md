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

These are behaviors specific to Claude Code. **All cross-tool rules live in [`AGENTS.md`](AGENTS.md) — read it first.** In particular AGENTS.md is normative for: the "where does this code go" lookup (`where-things-live.md`), required Markdown frontmatter under `docs/src/`, VoxScript-only automation (no new `.ps1`/`.sh`/`.py`), and the `cargo fmt --all` ban (use `vox run scripts/fmt.vox` / `cargo fmt -p <crate>`).

- If you open a `.vox` file, treat it as Vox language source — not Rust, not TypeScript.
- Honor `// vox:skip` annotations in code blocks; do not validate those against the compiler. (Prefer making fenced `vox` blocks compile; use `// vox:skip` + a reason only for genuine out-of-file excerpts.)
- Do not store project-specific research in your IDE/agent memory; write it to `docs/src/architecture/` instead (with frontmatter).

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
