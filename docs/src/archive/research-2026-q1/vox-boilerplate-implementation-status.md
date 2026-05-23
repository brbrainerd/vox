---
title: "Vox boilerplate implementation status"
description: "Live status board for roadmap execution across Wave 1, Wave 2, and Wave 3."
category: "Architecture SSOTs"
last_updated: "2026-03-25"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox boilerplate implementation status

## Progress summary
- Wave 1 foundation: started
- Wave 2 leverage: started
- Wave 3 scale: started

## Completed in this execution batch
- Baseline research persisted in architecture docs:
  - `docs/src/architecture/vox-boilerplate-reduction-master-roadmap.md`
  - `docs/src/architecture/vox-boilerplate-research-findings-2026.md`
  - `docs/src/architecture/vox-fullstack-ergonomics-deep-dive.md`
- Navigation/index updates:
  - `docs/src/SUMMARY.md`
  - `docs/agents/doc-inventory.json` regenerated through `vox ci doc-inventory generate`
- Wave 1 foundational code scaffolding:
  - `crates/vox-compiler/src/typeck/autofix.rs` upgraded from single stub behavior to rule-based architecture (`RuleBasedAutoFixer`) with backward-compatible `StubAutoFixer`
  - Focused tests passed: `cargo test -p vox-compiler autofix -- --nocapture`
- Wave 1 docs/code drift reduction:
  - `docs/src/explanation/expl-architecture.md` updated with consolidated `vox-compiler` implementation note and current file-path checklist
  - `docs/src/explanation/expl-compiler-lowering.md` updated with implementation note

## In-flight roadmap mapping

### Wave 1 foundation (partial)
- B001 parser coverage audit: partially completed (repo-grounded gap map in deep-dive docs).
- E001 doc/code parity for `?`: partially completed (parity called out and prioritized; compiler pass implementation pending).
- H001 metadata duplication map: completed in deep-dive mapping.
- I001 autofix scaffolding: completed with rule-based autofixer architecture.
- J001/J002 KPI baseline framing: partially completed in research + roadmap docs.

### Wave 2 leverage (partial)
- A001 syntax principles: draft-level coverage in master roadmap and research doc.
- D001 inference boundaries: draft-level guidance in roadmap.
- F001 shared route IR design target: defined in roadmap + deep dive.
- G001 data-layer friction audit: initial inventory in deep dive.

### Wave 3 scale (partial)
- Governance and migration framework: initialized via completion criteria, risk controls, and CI parity direction in roadmap docs.

## Explicit remaining work
- Implement all remaining stream tasks A002-J020 in code and tests.
- Add machine-readable task dependency graph with per-task risk/deps for execution automation.
- Land route IR unification and typed HIR debt elimination.
- Expand autofix rules beyond suggested-text baseline.
- Add KPI instrumentation and CI policy gates for boilerplate regression.


