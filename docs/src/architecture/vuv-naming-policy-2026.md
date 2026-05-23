---
title: "VUV Naming Policy (2026)"
description: "Deprecation cycle for primitive names, kwarg names, and decorator names. Every rename is announced, aliased for one major version, then removed. A rename registry tracks every alias; the `vox migrate` codemod rewrites old names to new ones."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Canonical reference for how Vox renames evolve. Cited from VUV phase plans."
---

# VUV Naming Policy (2026)

**The rule:** Every public Vox identifier — primitive name, kwarg name, decorator
name, type name, decorator-argument enum value — follows a three-step lifecycle:

1. **Announce** in a release note: "X has been renamed to Y."
2. **Alias** X to Y in the rename registry. Both names parse. Using X emits a
   one-line deprecation warning at compile time.
3. **Remove** X in the next major version. The registry retains the entry with
   `removed_in: "1.X"` for tooling and historical reference.

**The codemod:** `vox migrate` reads the rename registry and rewrites every
occurrence of an old name in a `.vox` corpus to its new name. The codemod is
**byte-equivalent**: re-running on a migrated corpus produces no diff.

**The registry:** `contracts/naming/renames.v1.json`. Single source of truth.
Every rename has `from`, `to`, `kind` (one of `primitive`, `kwarg`, `decorator`,
`enum_value`, `type`), `since` (version where the alias was introduced), and
optional `removed_in` (version where the alias becomes a hard error).

**No silent renames.** A change to a public name without a registry entry is
a CI failure. Enforcement: see `crates/vox-arch-check`.

**Why:** the dominant LLM-author failure mode in Gradio and Streamlit is that
old training corpora contain dead names. A model trained on Gradio 4.x cheerfully
emits `concurrency_count`, `Interface.load()`, `.style()` — all dead. Vox is
itself a corpus-aware language: MENS retrains on `examples/golden/` and the
dashboard. We cannot afford uncontrolled churn.

**See also:** [Gradio & Streamlit Research (2026)](gradio-streamlit-research-2026.md)
for the historical evidence; [GUI Authoring Syntax (2026)](gui-authoring-syntax-2026.md)
for the current VUV phase plan.
