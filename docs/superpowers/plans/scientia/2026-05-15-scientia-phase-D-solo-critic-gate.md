# SCIENTIA Phase D — Solo-Author Audited-Critic Gate

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** outline.

**Goal:** Design and implement a venue-gated path by which a single developer can clear the dual-distinct-approver requirement using an **audited LLM critic** as the second approver, while preserving the project's hard rule against GPT-4-grades-GPT-4 self-validation.

**Architecture:** Extend the existing `publication_approvers` table and approval flow with an `ApproverRole` enum (`Human`, `AuditedLLMCritic`). LLM-critic approvals require: (1) a registered critic identity with a distinct ORCID; (2) a model fingerprint that does not match any model used in the artifact's pipeline (enforced by recording model fingerprints in both manifest and critic approval, then refusing on match); (3) explicit `AiDisclosureBlock` entry with CRediT role `Validation` only; (4) venue-catalog opt-in (`allows_llm_critic: true`) — IMC/MLSys/TMLR default to `false`, Zenodo-deposit-only and F1000-track default to `true`; (5) the critic must run against the artifact's RO-Crate, not the abstract, and emit a signed structured review report stored alongside the approval row.

**Tech Stack:** Rust 2024; existing `publication_approvers` schema; existing `vox-crypto` ed25519; existing ORCID OAuth machinery (Finalization Phase 8); existing `vox-claim-extractor` (re-used as critic harness).

**Strategic context:** [Gap-map §2 Gap D](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-d--solo-author-critic-gate-path); [Finalization Plan R3 risk (GPT-4 grades GPT-4)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#10-risks).

**Out of scope:**
- Replacing human approvers in venues that prohibit LLM critics (the catalog flag enforces this).
- Critic model training (this phase uses an existing critic — likely Inspect-Evals task framework + MiniCheck for grounding).
- Adversarial critic detection beyond model-fingerprint exclusion.

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/vox-db/src/schema/domains/publish_cloud.rs` | Add `approver_role`, `model_fingerprint`, `critic_report_uri` columns to `publication_approvers` |
| Modify | `crates/vox-db/src/store/ops_publication.rs` | Approver insert path with role; gate logic enforces ≥1 human + ≥1 distinct-fingerprint LLM-critic OR ≥2 distinct humans |
| Create | `crates/vox-publisher/src/critic/mod.rs` | LLM-critic runner: takes manifest+RO-Crate, emits structured review JSON |
| Create | `crates/vox-publisher/src/critic/fingerprint.rs` | Model-fingerprint computation + exclusion check |
| Create | `crates/vox-publisher/src/critic/report.rs` | Structured critic-report shape (issues found, severity, evidence-bound) |
| Modify | `contracts/scientia/venue-catalog.v1.yaml` | Add `allows_llm_critic: bool` per venue row |
| Modify | `contracts/scientia/publication-worthiness.schema.json` | Add `solo_via_critic: bool` decision-trace field |
| Modify | `crates/vox-ro-crate/src/ai_disclosure.rs` | LLM-critic auto-disclosure entry with CRediT `Validation` role |
| Modify | `crates/vox-cli/src/commands/db/publication.rs` | Add `publication-critic-approve` subcommand |
| Modify | `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs` | Add MCP tool for critic invocation |
| Modify | `docs/src/reference/scientia-publication-playbook.md` | Add critic-failure remediation entries |
| Modify | `docs/src/how-to/how-to-scientia-publication.md` | Solo-publication walkthrough |
| Modify | `docs/src/reference/scientia-ssot-handbook.md` | Document `ApproverRole` |

LoC budget: ~800 LoC + ~400 tests. Mostly policy + DB plumbing.

---

## Tasks (headings only)

### Task D1: Schema migration
Add columns to `publication_approvers`. Per [SSOT §5.5](../../../src/architecture/mesh-and-language-distribution-ssot-2026.md), bump `BASELINE_VERSION` in `manifest.rs` only — no date-stamped SQL files.

### Task D2: Gate logic update
`approval_gate_cleared(digest)` returns true iff:
- ≥2 distinct human approvers, OR
- ≥1 human approver AND ≥1 LLM-critic approver with distinct model fingerprint AND venue allows critic.

### Task D3: Model fingerprint
A model fingerprint is the hash of `(provider_name, model_id, parameter_count_hint, training_cutoff_date)`. Stored on every artifact-producing event and on every critic approval. Exclusion check refuses if any artifact-side fingerprint appears in the critic-side fingerprint set.

### Task D4: LLM-critic runner
Critic process:
1. Receives RO-Crate path + manifest digest.
2. Loads the deposited artifact, claim envelopes, prereg, worthiness signals.
3. Runs structured review prompts (constrained-grammar emission) checking for:
   - Claim-evidence mismatches (cross-check vs `vox-claim-extractor` output).
   - Methods omissions vs reported metrics.
   - Reproducibility-pack completeness vs RO-Crate mainEntity.
   - Citation-source matches vs SPECTER2 retrieval.
5. Emits signed JSON report; stores under `critic_reports/{manifest-digest}/{critic-id}.json`.
6. Inserts approval row only on `report.recommendation = approve`.

### Task D5: Venue-catalog flag
Add `allows_llm_critic: bool` (default `false`) to every venue row in `contracts/scientia/venue-catalog.v1.yaml`. Set `true` only on:
- `zenodo_deposit_only`
- `f1000_publish_then_review`
- Vox-native living-review canonical-URL track

### Task D6: AI-disclosure auto-fill
When a critic approval contributes to the gate, `AiDisclosureBlock::build` auto-appends an `AiToolUsage` entry with:
- `crediT_role: "Validation"` (only).
- `model_identification: <critic fingerprint>`.
- `audit_log_uri: <critic report URI>`.

### Task D7: CLI surface
`vox scientia publication-critic-approve --publication-id <id> --critic-id <id>` runs the critic and writes the approval if recommendation is approve.

### Task D8: Integration tests
- Critic-fingerprint matches artifact-fingerprint → gate refuses approval.
- Venue does not allow critic → gate refuses.
- Two distinct humans → gate clears (no change to existing behavior).
- One human + one fingerprint-distinct critic on a critic-allowed venue → gate clears.

### Task D9: Failure-mode playbook entries
Stable codes: `critic_fingerprint_match`, `critic_venue_not_allowed`, `critic_recommendation_revise`, `critic_report_signature_invalid`.

### Task D10: Documentation
- Solo publication how-to.
- Playbook entries.
- SSOT handbook role table update.

---

## Acceptance criteria

1. Schema migration green; existing dual-human flow unaffected.
2. Critic-fingerprint exclusion test refuses approval when fingerprints match.
3. Venue-catalog gate test refuses approval when venue doesn't allow critic.
4. AI-disclosure auto-fill correctly records critic contribution as CRediT `Validation` only.
5. CLI surface walks a solo developer through Zenodo-deposit-only path end-to-end.
6. Playbook + how-to docs updated with the four stable failure codes.

---

## Open questions

- **OQ-D1.** Critic-of-critic. If two distinct LLM critics with distinct fingerprints are used, can they substitute for a human? Recommendation: **no**. Always require ≥1 human approver. The critic is a *supplement*, not a substitute.
- **OQ-D2.** ORCID-distinct critic identity ethical acceptability. Per gap-map open question. Recommendation: venue-catalog enforces this per venue; we don't make a blanket judgment.
- **OQ-D3.** Critic recommendation taxonomy. `approve` / `revise` / `reject` are minimal. Add `approve_with_notes`? Recommendation: yes, with notes surfaced in `next_actions` but still gating to `approve`.
- **OQ-D4.** Critic-report retention. RO-Crate-embedded, status-event-only, or separate ledger table? Recommendation: separate `critic_reports` table, hashed pointer from approval row, included in RO-Crate deposit at publication time.

---

## Dependencies

- **Upstream:** Finalization Phase 7 (`AiDisclosureBlock`) ✅; Phase 8 (venue catalog, ORCID OAuth) ✅; `vox-claim-extractor` ✅.
- **Downstream:** None hard. Phase E's micro-track config can preselect the critic-allowed venues.

---

## Cross-references

- Gap: [gap-map §2 Gap D](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-d--solo-author-critic-gate-path)
- Risk R3: [Finalization Plan §10](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#10-risks)
- AI-disclosure: `crates/vox-ro-crate/src/ai_disclosure.rs`
- Venue catalog: [`contracts/scientia/venue-catalog.v1.yaml`](../../../../contracts/scientia/venue-catalog.v1.yaml)
