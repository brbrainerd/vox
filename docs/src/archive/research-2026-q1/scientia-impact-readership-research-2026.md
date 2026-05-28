---
title: "SCIENTIA impact, readership, and citation-adjacent signals (research seed)"
description: "External landscape for what gets read and cited; feasibility for Vox; seeds for a projection layer orthogonal to novelty; critique of prior heuristic implementation."
category: "Architecture SSOTs"
status: "research"
sort_order: 12
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# SCIENTIA impact, readership, and citation-adjacent signals

This document is the **single research anchor** for extending SCIENTIA beyond **novelty / prior-art** toward **impact and audience success proxies** (what people read, cite, and amplify). It complements:

- [SCIENTIA publication automation SSOT](scientia-publication-automation-ssot.md) (automation boundaries),
- Novelty ledger contracts under `contracts/scientia/` (finding-candidate, novelty-evidence-bundle),
- Tunable parameter seed: [`contracts/scientia/impact-readership-projection.seed.v1.yaml`](../../../contracts/scientia/impact-readership-projection.seed.v1.yaml).
- [SCIENTIA multi-platform ranking, discovery, and anti-slop SSOT (research 2026)](scientia-multi-platform-ranking-discovery-research-2026.md) — social vs scholarly ranking surfaces, ingest vs syndicate, and operator KPI sketches complementary to impact projection.

**Non-goals:** Vox does not claim to *predict* future citations authoritatively. The feasible product is an **inspectable, contract-weighted projection** used for **prioritization, routing, and operator transparency**, never as a hard publish/deny gate without human review.

## Why this is orthogonal to novelty

| Dimension | Question | Typical signals |
| --- | --- | --- |
| **Novelty** | Is this already in the literature? | Prior-art overlap, contradiction risk, query traces |
| **Impact / success** | If published, might it travel? | Citations, citing velocity, field-relative attention, readership proxies, venue reach |

A finding can be **novel but low resonance** (narrow tooling note) or **high resonance but weakly novel** (clear survey of known ideas). Publication policy needs both lenses **without conflating them**.

## External landscape (what already does this)

Solid, citable references for implementation seeds:

1. **Bibliometric APIs (observed counts, not forecasts)**  
   - **OpenAlex**: open work metadata, citation counts, open citation graph facets—good for **post-hoc** and **comparable-work** baselines.  
   - **Crossref / DataCite**: DOI-level metadata; Crossref’s separate **Event Data** mention stream is **sunset 2026-04-23** (see [multi-platform ranking research](scientia-multi-platform-ranking-discovery-research-2026.md) §4.12 / Crossref blog). Useful for **discoverability** and **persistence** more than prediction.  
   - **Semantic Scholar**: citation counts; **highly influential citation** labeling uses ML over full-text citation contexts (useful conceptually; Vox may only see API summaries without full text).

2. **Citation *prediction* (research systems, heavy ML)**  
   - **ForeCite** ([arXiv:2505.08941](https://arxiv.org/abs/2505.08941)): causal LM–style forecasting of future citation rates on large biomedical corpora—illustrates that **title/abstract + time + field** carry signal; training such a model is **not** a near-term in-repo deliverable.  
   - **HLM-Cite** (2024): hybrid LM workflow emphasizing **core vs peripheral** citations—relevant if Vox later does structured claim–evidence graphs.  
   - **Graph vs text benchmarks** (e.g. EMNLP 2024 finding papers): edge-based (citation graph) vs node-based (text) tradeoffs depend on data scale and horizon—Vox should default to **transparent features**, not a black-box score.

3. **Readership and attention (altmetrics)**  
   - **Altmetric Attention Score** and **Dimensions** integrations (see vendor docs): weighted **mention** counts across news, policy, social, blogs, etc. **Not** the same as scientific quality; strong **early visibility** signal.  
   - Literature on **altmetrics vs early citations** (e.g. studies on Mendeley readership and Twitter features): useful for defining **feature families** if Vox ever ingests licensed altmetric feeds—not assumed available by default.

4. **Venue and genre**  
   Journal tier, open access, and subfield norms shift baseline citation rates. Any projection must carry **`field_baseline` / `venue_tier` / `topic`** metadata to avoid naive global thresholds.

## What Vox can feasibly implement (phased seeds)

Ordered for **honesty about data access** and **SSOT weighting** (`impact-readership-projection.seed.v1.yaml`):

| Phase | Capability | Data | Automation posture |
| --- | --- | --- | --- |
| **A** | **Comparable work feature pack** | From existing OpenAlex / Semantic Scholar federator responses: citation count, publication year, **simple velocity** (citations per year since publish), coarse field (from venue/container or topics) | **Assist**: attach to manifest metadata or a sibling JSON blob; show in preflight / happy-path JSON |
| **B** | **Field-normalized baselines** | Offline or cached tables keyed by subject / venue (maintained as repo data under `contracts/reports/` or small DB table)—**weights and bucket edges live in the seed YAML**, not hard-coded in Rust | **Assist**: report “above / near / below” bucket, not a single “impact score” |
| **C** | **Attention / altmetrics hook** (optional) | Clavis-backed API keys; explicit operator opt-in | **Assist** only; heavy rate limits; never block publish path by default |
| **D** | **Learned projection** | External service or training pipeline **outside** default Vox repo | **Experimental**; if adopted, model card + calibration telemetry required |

## Critique of recent in-repo novelty automation work

This section **does not replace** code review; it records architectural debt to fix while expanding toward impact projection.

1. **Heuristic constants in Rust**  
   Significance axes, confidence decomposition, and overlap-to-novelty mappings use **numeric literals** in `vox-publisher` helpers. That optimizes for a fast first slice but violates the **Dynamics** preference (parameters should move with policy). **Remediation:** load weights and bucket thresholds from [`contracts/scientia/impact-readership-projection.seed.v1.yaml`](../../../contracts/scientia/impact-readership-projection.seed.v1.yaml) (or a split `scientia-discovery-heuristics.v1.yaml` if impact vs discovery tuning diverges).

2. **Prior-art ≠ impact**  
   The federated bundle answers **overlap**; it does not, by itself, answer **who will care**. **Remediation:** extend stdout / MCP payloads with a **`ComparableWorksSummary`** (or separate `impact_projection` object) so operators see both panels.

3. **Calibration telemetry today**  
   Current calibration envelopes emphasize **latency and overlap**. **Remediation:** add optional fields (behind schema version bumps) for **projected audience tier** and **data completeness** (`missing_fields: [...]`) when phase A ships.

4. **Single source of truth**  
   Novelty contracts live under `contracts/scientia/*.schema.json`. Impact projection should follow the same pattern: **schemas for stored artifacts**, **YAML seeds for tunables**, **this doc for rationale**—avoid scattering magic numbers across `scientia_discovery.rs` and `scientia_finding_ledger.rs` long term.

## SSOT maintenance rules

- **New numeric policy** for impact/readership → update the seed YAML + one line in this doc’s changelog (below).  
- **New external signal family** → add to seed `signal_families` + document license/opt-in here.  
- **Shipped JSON shape** → add or extend a JSON Schema under `contracts/scientia/` and register in [`contracts/index.yaml`](../../../contracts/index.yaml).

## Changelog

| Date | Change |
| --- | --- |
| 2026-04-02 | Initial research seed, external survey, phased feasibility, critique of heuristic novelty work, link to projection seed YAML. |
| 2026-04-12 | Crossref Event Data sunset note (pointer to multi-platform research §4.12). |


