---
name: deep-research
description: Use for a deep, multi-source, fact-checked research report on a topic - fan out searches across angles, fetch primary sources, adversarially verify claims, and synthesize a cited report with graded confidence.
---

# Deep Research (Vox Adaptation)

## Overview

Produce a cited, fact-checked research report. The method is fan-out → fetch → adversarially verify → synthesize. Output is a structured report with **graded confidence** (★★★ primary-confirmed · ★★ reputable single source · ★ inferred) and a Sources list.

**Announce at start:** "I'm using the deep-research skill."

## Before researching

If the question is underspecified, ask 2-3 clarifying questions to narrow scope first. Then weave the answers into the research question.

## Method

1. **Decompose** the question into 4-6 independent angles (prior art, academic feasibility, frontier/skeptical, etc.).
2. **Search in parallel clusters** — batch related queries; cover each angle.
3. **Fetch** the top primary sources (dedupe URLs); extract falsifiable claims with quotes.
4. **Adversarially verify** each load-bearing claim — try to *refute* it; keep only what survives. A claim that cannot be verified is marked *unverified*, **not** treated as true and **not** treated as refuted.
5. **Synthesize** — merge duplicates, rank by confidence, cite every claim, state caveats and open questions.

## Rate-limit discipline (IMPORTANT)

Large fan-out (one agent per query, dozens at once) can trip a transient server rate limit, especially in the *verify* phase — observed repeatedly. Mitigations:
- **Throttle:** batch ~4-8 searches at a time, sequentially across batches, rather than one huge burst.
- **Operator-paced fallback:** when the harness verify phase is rate-limited, do the searches/fetches directly and paced; grade by direct-fetch evidence, not by a rate-limited vote tally.
- **Never** record a rate-limited (abstained) claim as refuted. Distinguish "unverified this run" from "false".

## Output discipline

- Cite primary sources by URL. Prefer primary over secondary over blog.
- Grade every claim. Separate confirmed / reputable-unverified / refuted.
- End with caveats + open questions. Persist findings to `docs/src/architecture/` with YAML frontmatter (per AGENTS.md) so future sessions reuse them.
  - Output files must follow the naming pattern `*-research-YYYY-MM-DD.md` or `*-findings-YYYY-MM-DD.md`
  - Frontmatter must include: `title`, `description`, `category`, `status`, `training_eligible`
  - After creating a new research page, set YAML frontmatter (`title`, `description`, `category`, `status`, `training_eligible`). Do not edit `docs/src/architecture/research-index.md` (retired).
