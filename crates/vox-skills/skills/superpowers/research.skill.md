---
name: research
description: Use for quick-to-moderate web research - paced parallel searches and source fetches to answer a focused question, lighter than the full deep-research harness.
---

# Research (Vox Adaptation)

## Overview

Answer a focused question from current web sources without the full adversarial deep-research pipeline. Fast, paced, cited.

**Announce at start:** "I'm using the research skill."

## Method

1. **Frame** the question and 2-4 sub-questions.
2. **Search in parallel clusters** — issue ~4 related queries together; read the result titles/snippets.
3. **Fetch** the 1-3 most authoritative sources per sub-question for detail/quotes.
4. **Answer** with inline citations; flag uncertainty explicitly; note what you could not verify.

## Rate-limit discipline

Batch searches ~4 at a time, paced across batches; do not burst dozens at once (trips transient server rate limits). If a fetch is rate-limited, retry paced or fall back to the search snippet and mark the claim's confidence lower.

## When to escalate

If the question is load-bearing for a decision, needs adversarial fact-checking, or spans many angles, use the `deep-research` skill instead and persist a graded report to `docs/src/architecture/`.

## Output

- Cite sources by URL. Prefer primary sources.
- Be explicit about confidence and gaps. Do not present a rate-limited/unverified claim as confirmed.
