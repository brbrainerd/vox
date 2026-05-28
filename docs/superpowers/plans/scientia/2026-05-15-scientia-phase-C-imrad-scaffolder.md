# SCIENTIA Phase C — Long-Form IMRaD Manuscript Scaffolder

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** outline.

**Goal:** Generate a long-form IMRaD (Introduction / Methods / Results / Discussion) markdown skeleton from a `FindingCandidate` + its verified atomic claims + its RO-Crate, filling **only provenance-bound safe slots** and leaving explicit `<!-- TODO(narrative): -->` blocks for human-written sections that the worthiness rubric forbids automating.

**Architecture:** New L2 crate `vox-manuscript-scaffold` providing `scaffold_imrad(publication_id) -> ScaffoldOutput`. The scaffolder emits constrained JSON (via existing `vox-constrained-gen` XGrammar pipeline) representing the section tree, then renders it through a templating layer to markdown. Hard rule (enforced by unit tests): the renderer never emits free-form prose in sections the rubric protects — Introduction, Discussion, Significance. Those sections are emitted as empty TODO blocks with cited facts pre-listed for the human to compose around.

**Tech Stack:** Rust 2024; existing `vox-constrained-gen`; existing claim envelope types from `vox-claim-extractor`; existing RO-Crate from `vox-ro-crate`; existing `AiDisclosureBlock` from Finalization Phase 7; `tera` or `handlebars` for templating (already in tree — verify in Task C1).

**Strategic context:** [Gap-map §2 Gap C](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-c--long-form-manuscript-scaffolder); [Finalization Plan §4 (Artifact format)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#4-artifact-format--the-unit-of-trust-is-not-the-paper) and [§7 (format adapt)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#phase-7--format-adaptation-constrained-grammar-all-the-way-2-wk--complete-2026-05-09).

**Out of scope:**
- Short-form adaptation (already complete — Finalization Phase 7).
- LaTeX rendering (markdown only in Phase C; LaTeX export via pandoc is a follow-up).
- Narrative-section auto-generation (forbidden by rubric — this phase enforces that boundary).
- Per-venue style transforms (Phase E handles per-class profiles).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Create | `crates/vox-manuscript-scaffold/Cargo.toml` | L2 crate manifest |
| Create | `crates/vox-manuscript-scaffold/src/lib.rs` | Public API: `scaffold_imrad`, `ScaffoldOutput` |
| Create | `crates/vox-manuscript-scaffold/src/section_tree.rs` | Constrained section-tree JSON shape |
| Create | `crates/vox-manuscript-scaffold/src/safe_slots.rs` | What can be auto-filled vs what's forbidden |
| Create | `crates/vox-manuscript-scaffold/src/render.rs` | Section-tree → markdown renderer |
| Create | `crates/vox-manuscript-scaffold/src/citations.rs` | Inline citation rendering from verified prior-art set |
| Create | `crates/vox-manuscript-scaffold/templates/imrad.md.tera` | Base template (Tera if that's already in tree) |
| Create | `crates/vox-manuscript-scaffold/tests/integration.rs` | Round-trip: synthetic candidate + RO-Crate → markdown |
| Create | `crates/vox-manuscript-scaffold/tests/rubric_compliance.rs` | Tests that scaffolder never auto-emits forbidden sections |
| Modify | `crates/vox-cli/src/commands/db/publication.rs` | Add `publication-manuscript-draft` subcommand |
| Modify | `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs` | Add `vox_scientia_publication_manuscript_draft` MCP tool |
| Modify | `contracts/cli/command-registry.yaml` | Register new CLI command |
| Modify | `contracts/mcp/tool-registry.canonical.yaml` | Register new MCP tool |
| Modify | `docs/src/architecture/where-things-live.md` | Add row: "IMRaD scaffolder" |
| Modify | `docs/src/architecture/layers.toml` | Register at L2 |

LoC budget: ~1200 LoC + ~500 tests.

---

## Tasks (headings only)

### Task C1: Identify in-tree templating crate
Verify whether `tera`, `handlebars`, `askama`, or other is already in `Cargo.lock`. Match the existing one rather than adding a new dep.

### Task C2: Define `ScaffoldOutput` and section tree
JSON shape:
```jsonc
{
  "title": "<filled from candidate.title_hint, marked machine_suggested>",
  "abstract": null,  // TODO block
  "introduction": null,  // TODO block + cited-facts list
  "methods": {
    "design": "<from prereg.eval_substrate>",
    "data": "<from RO-Crate datasets>",
    "procedure": "<from RO-Crate mainEntity>",
    "metrics": "<from prereg.metric>"
  },
  "results": {
    "table": [/* one row per verified claim with Trusty URI */],
    "narrative": null  // TODO block
  },
  "discussion": null,  // TODO block
  "significance": null,  // TODO block
  "limitations": "<from worthiness signals with manual_required hints>",
  "references": [/* SPECTER2-verified prior-art set */],
  "acknowledgments": "<from CRediT block>",
  "ai_disclosure": "<from AiDisclosureBlock::build>",
  "competing_interests": "<from COI declaration>"
}
```

### Task C3: Implement `safe_slots.rs`
Hard rule with unit-test coverage: any slot in the `forbidden` list MUST emit `null` in the JSON output. Forbidden slots are derived from the worthiness rubric §"What should not be generated".

### Task C4: Claim → results-table renderer
One row per atomic claim with: claim text, Trusty URI link, evidence source, verifier verdict, confidence interval.

### Task C5: Methods section renderer
From prereg + RO-Crate + benchmark config. Each statement traces to a declarative source — no narrative interpretation.

### Task C6: Citation rendering
From the SPECTER2-verified prior-art set used in novelty checking. Standard CSL-style references.

### Task C7: TODO-block renderer
For each forbidden section, emit:
```markdown
## Introduction

<!-- TODO(narrative): write the introduction yourself.

The worthiness rubric forbids auto-generating novelty/significance
assertions. Use the cited facts below to compose your introduction.

Cited facts (Trusty URIs preserved):
- [fact 1 from verified claim set]
- [fact 2 ...]
-->
```

### Task C8: Rubric-compliance test suite
Property tests: for any synthetic input, the rendered markdown has zero non-empty prose in forbidden sections. Fuzz this.

### Task C9: CLI wiring
`vox scientia publication-manuscript-draft --publication-id <id> --output <path>` emits markdown.

### Task C10: MCP wiring
`vox_scientia_publication_manuscript_draft` for agent invocation.

### Task C11: Integration test
End-to-end: synthetic candidate + RO-Crate + verified claims → scaffolder → markdown → assert structure.

### Task C12: Documentation
- README: how to use the scaffolder, what it will and won't fill.
- Worthiness rules doc cross-reference.

---

## Acceptance criteria

1. `cargo test -p vox-manuscript-scaffold` green; rubric-compliance suite green on fuzzed inputs.
2. `vox scientia publication-manuscript-draft` produces a syntactically valid markdown file with the structure in Task C2.
3. Zero auto-emitted prose in forbidden sections across 1000 fuzzed inputs.
4. CLI + MCP coverage per registry parity checks.
5. The resulting markdown is a valid input to existing manuscript pipelines (pandoc clean-render check).

---

## Open questions

- **OQ-C1.** Reference format. CSL JSON, BibTeX, or both? Recommendation: emit CSL JSON sidecar + inline `[Author Year]` placeholders; let the human pick rendering.
- **OQ-C2.** Author block. Auto-filled from ORCID metadata or always TODO? The Finalization Plan strongly couples author identity to ORCID. Recommendation: auto-fill if all ORCID present; TODO with hint if any are missing.
- **OQ-C3.** Figure handling. Phase 7 disabled LLM-figure generation; schematic-only with mandatory legend disclosure. Should the scaffolder include figure-placeholder blocks? Recommendation: yes, with `FigurePolicy` enforcement and `<!-- TODO(figure): -->` blocks for measured-outcome figures.
- **OQ-C4.** Multilingual scaffolds. Out of scope for Phase C; English only.
- **OQ-C5.** Per-class structure. `algorithmic_improvement` papers and `policy_governance` papers have different section weights. Should this be in Phase C or deferred to Phase E? Recommendation: provide a single IMRaD template in Phase C; per-class templates in Phase E.

---

## Dependencies

- **Upstream:** Phase 1 (claim extractor) ✅; Phase 2 (prereg) ✅; Phase 4 (RO-Crate) ✅; Phase 7 (AI-disclosure, FigurePolicy) ✅.
- **Downstream:** Phase E (per-class manuscript profiles); Phase G (reading surface renders these manuscripts).

---

## Cross-references

- Gap: [gap-map §2 Gap C](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-c--long-form-manuscript-scaffolder)
- Worthiness rules: [`scientia-publication-worthiness-rules.md`](../../../src/reference/scientia-publication-worthiness-rules.md)
- AI-disclosure builder: `crates/vox-ro-crate/src/ai_disclosure.rs`
- Format adaptation: `crates/vox-research-events/src/publication_format.rs`
