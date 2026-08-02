---
title: "Claude Code Overlay"
description: "Claude Code-specific instructions and behavior narrowing."
category: "contributor"
status: "current"
training_eligible: true
training_rationale: "Defines Claude-specific rules for interacting with the Vox codebase."
---
@AGENTS.md

## Claude Code

These are behaviors specific to Claude Code, in addition to the imported `AGENTS.md` above.

- If you open a `.vox` file, treat it as Vox language source — not Rust, not TypeScript.
- Honor `// vox:skip` annotations in code blocks; do not validate those against the compiler. (Prefer making fenced `vox` blocks compile; use `// vox:skip` + a reason only for genuine out-of-file excerpts.)
- Do not store project-specific research in your IDE/agent memory; write it to `docs/src/architecture/` instead (with frontmatter).

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
