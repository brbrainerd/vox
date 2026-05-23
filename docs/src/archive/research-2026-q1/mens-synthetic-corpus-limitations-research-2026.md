---
title: "MENS Synthetic Corpus: Limitations and Mitigation Strategies (Research 2026)"
description: "Synthesizes the known limitations of Vox's gigantic synthetic corpus generation approach for MENS training, maps them to the existing codebase, and proposes concrete mitigation strategies to bypass the data paradox."
category: "Architecture SSOTs"
status: "research"
research_date: "2026-04-12"
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Directly shapes the corpus generation and quality-gating strategy for all MENS domain adapters."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# MENS Synthetic Corpus: Limitations and Mitigation Strategies (Research 2026)

## The Paradox

Training a specialist model on a novel DSL like Vox-lang requires large-scale, high-quality text — but Vox-lang does not yet have large-scale, high-quality text because the language is new and its real-world usage is thin. The natural impulse is to generate it synthetically. The paradox is that synthetic generation itself requires a capable model to generate plausible Vox code — but that capable model only exists *after* training.

This document synthesizes what Vox is currently doing to escape this paradox, maps the known limitations of each approach (grounded in existing research in this docs tree), and proposes concrete mitigation vectors for each failure class.

---

## 1. What Vox Is Currently Doing

### 1.1 Template-Expansion Generator (`vox generate-data`)

The native Rust generator in `crates/vox-cli/src/training/datagen.rs` expands a fixed set of **Base Examples** via deterministic shuffling and instruction-variant permutation. Each base example contains:

- Multiple instruction phrasings (to improve prompt robustness)
- A canonical code segment (syntactically verified)
- A difficulty score (1–10) for curriculum learning
- A category tag (`actor`, `workflow`, `type`, `component`, etc.)

This allows a small number of hand-authored seeds to produce a formally large JSONL output. The generator is fast (orders of magnitude faster than Python equivalents), integrated into CI, and inherently compiler-verifiable.

**Current outputs referenced in config:**

| Mix file | Lanes | Primary weight |
|---|---|---|
| `mix-vox-lang.yaml` | `golden`, `organic`, `docs`, `synthetic`, `distillation` | `golden` (6) |
| `mix-rust.yaml` | `rust_pairs`, `rust_doc` | `rust_pairs` (4) |
| `mix-agents.yaml` | `tool_traces`, `autofeedback`, `multi_turn` | `tool_traces` (5) |
| `mix-research.yaml` | (emerging) research lane | — |
| `mix-populi-meta.yaml` | (emerging) self-knowledge lane | — |

### 1.2 The Healing Loop (`HealingLoop` in `healing.rs`)

When the model generates Vox code that fails compilation, the healing loop iteratively calls the LLM with the compiler diagnostics until the code heals or `max_attempts` is exhausted. Every successful `(failed → repaired)` pair is logged to `~/.vox/corpus/heal_pairs.jsonl` for offline fine-tuning. This is a live, compiler-in-the-loop corpus-enrichment mechanism that derives new training signal from production failures.

### 1.3 The Dogfood Flywheel

Real orchestrator sessions produce `tool_traces.example.jsonl`, `multi_turn.jsonl`, and `autofeedback.jsonl` under `target/dogfood/`. The `vox populi corpus extract` command promotes quality-rated traces into the training mix. This creates a closed loop: better model → better sessions → richer dogfood → better model.

### 1.4 Frontier Distillation (`distillation` lane, weight 2)

Frontier model outputs (Gemini, Claude performing real Vox-related tasks) are recorded and promoted into the `vox-lang` distillation lane. This injects an exogenous distribution anchor that is not structurally limited by the DSL's current real-world usage.

### 1.5 Corpus Lab Tier System

The [corpus lab research](vox-corpus-lab-research-2026.md) formalizes a Tier A / B / C policy:

- **Tier A** — checked-in `examples/golden/**/*.vox`, CI-gated
- **Tier B** — ephemeral operator-local mass corpus (seeded, mutated, LLM-generated) — must be compiler-validated before promotion
- **Tier C** — negative fixtures (`examples/parser-inventory/`) — never mixed into training goldens

archived_date: 2026-04-18
---

## 2. Limitations of the Synthetic Corpus Approach

### 2.1 Template Exhaustion and Low Semantic Diversity

The template-expansion generator is fundamentally bounded by its seed set. Permuting instruction phrasings and shuffling code segments does not produce *novel semantic programs* — it produces variants of the same ~N base examples. The AST structures generated are a tiny fraction of the actual program space expressible in Vox. As documented in [MAD and mode collapse](research-cl-mad-mode-collapse-2026.md), recursive training on a low-variance distribution collapses the model toward the mean of that distribution, erasing rare and boundary behaviors.

**Concrete consequence:** A model trained predominantly on template-expanded data will learn to write `actor` blocks and `workflow` blocks in the specific structural patterns of the ~30 base examples. It will not generalize to novel compositions, deeply nested constructs, or unusual (but valid) syntactic paths.

### 2.2 Syntactic Validity ≠ Semantic Correctness (The Oracle Problem)

As documented in [The Compile-Pass Oracle and Semantic Degradation](research-cl-oracle-semantic-drift-2026.md), a compile-pass binary oracle is an insufficient gating mechanism. Vox code that compiles can be semantically void — empty actors with no handlers, workflows that always return the trivial case, functions that produce a constant regardless of input. These "hollow programs" satisfy the compiler but teach the model nothing about meaningful intent-to-code mapping.

> Semantic errors — programs that compile successfully but execute incorrect logic — constitute the vast majority of observed faults in code generation models (>60% across DeepSeek-Coder / QwenCoder evaluations, 2025).

The healing loop in `healing.rs` is also constrained by this: `heal_pairs.jsonl` contains `(failed → compiled)` pairs, not `(failed → correct)` pairs.

### 2.3 Model Autophagy Disorder (MAD)

As documented in [Quality and Mode Collapse](research-cl-mad-mode-collapse-2026.md), if synthetic data *replaces* rather than *accumulates alongside* real data in each fine-tuning batch, mode collapse is mathematically guaranteed:

1. **Early MAD**: statistical tails (rare constructs, unusual but valid patterns) are pruned from the distribution
2. **Late MAD**: variance collapses to near zero; the model "confuses disparate concepts" and outputs homogeneous code

The Vox lane weighting system (`golden: 6`, `synthetic: 1`) is a first-order mitigation — but it is not sufficient alone if the absolute volume of synthetic data grows to 10×+ the golden corpus, because the effective sample count still skews toward synthetic.

### 2.4 Corpus Volume Thresholds Are Not Met by Templates Alone

From [Minimum Viable Corpus Size for QLoRA Domain Adaptation](research-cl-qlora-minimum-corpus-2026.md):

| Threshold | Required examples | Status |
|---|---|---|
| Avoid catastrophic overfitting | ≥ 1,000–5,000 diverse pairs | 🟡 Achievable via templates but with low diversity |
| Robust novel-syntax generation | ≥ 10,000–50,000 pairs | 🔴 Not met for most domains |
| Deep domain expertise capture | ≥ 50,000–500,000 pairs | 🔴 Not met for any domain |

Template expansion from ~30 seeds with instruction permutations realistically produces 3,000–15,000 structurally similar pairs. This technically crosses the minimum overfitting threshold but provides a narrow distribution that doesn't support production-quality code generation.

### 2.5 The "AI Slop" Contamination Risk

As documented in [The Risks of Agent-Generated Prose](research-cl-slop-typicality-bias-2026.md), any prose included in the training corpus (documentation, Schola explanations, Scientia summaries) is structurally vulnerable to **typicality bias**: models prefer stereotypical phrasings, creating feedback loops that amplify mediocre patterns. Without an independent curator LLM, training on self-generated documentation causes:

- **Semantic hallucination**: fabricated Vox APIs embedded in "correct" explanations
- **Stylistic homogenization**: all documentation sounds identical because of structural tropes

This is especially dangerous for the emerging `mix-research.yaml` and `mix-populi-meta.yaml` lanes, which are primarily prose-based.

### 2.6 Catastrophic Forgetting in Repeated QLoRA Cycles

As documented in [Catastrophic Forgetting in QLoRA Fine-Tuning](research-cl-qlora-catastrophic-forgetting-2026.md), repeated sequential QLoRA runs erode the base model's generalized capabilities even though only 3–5% of weights are modified. Three active mechanisms:

1. Gradient interference in attention weights (15–23% of attention heads disrupted)
2. Representational drift in intermediate layers
3. Loss landscape flattening destroying prior task minima

Standard LoRA does not mitigate this. The existing MENS architecture (separate adapters, no cross-domain contamination) is the right *structural* defense — but within each domain's sequential runs, forgetting accumulates.

### 2.7 Reward Hacking in GRPO Fine-Tuning

As documented in [GRPO Reward Shaping](research-grpo-reward-shaping-2026.md) and [The Compile-Pass Oracle](research-cl-oracle-semantic-drift-2026.md), a binary compile-pass reward trains models to discover the shortest path to a passing compile — often empty structural scaffolding (empty actors, trivial returns, unused variable declarations). The current `0.6 × r_syntax + 0.3 × r_test + 0.1 × r_coverage` reward split assigns 60% weight to raw syntactic correctness, which actively incentivizes this pathology.

### 2.8 Negative Examples Are Discarded

The dogfood flywheel and template generator currently discard all non-compiling outputs. This is a waste. As documented in [Utilizing Parse Failures as Negative Examples](research-cl-nat-dpo-2026.md), negative-aware training (NAT) and DPO-style preference optimization over `(failed, repaired)` pairs provide dense, localized learning signals that are often more informative than additional positive examples. The `heal_pairs.jsonl` mechanism *does* capture `(failed → repaired)` pairs, but they are not yet wired into a DPO training loop.

---

## 3. Mitigation Strategies

### 3.1 Compiler-Coupled AST-Aware Mutation

**Addresses:** Template exhaustion (§2.1), volume threshold (§2.4)

Instead of expanding fixed instruction variants, the generator should **mutate the AST** of passing programs:

- **Subtree substitution**: replace a leaf expression with a semantically comparable variant (a different literal, a named constant, a different binary operator)
- **Block insertion/wrapping**: wrap an actor's handler in a `retry` block, add `error` branches to a `workflow`
- **Cross-pollination**: graft valid subtrees from one example into another that type-checks

Because mutations start from *compiler-verified programs*, every valid mutation is trivially verifiable by running the Vox compiler on the mutated output. This produces high-diversity, high-volume programs at low marginal cost. The existing `canonicalize_vox` utility provides stable diffs for mutation tracking. This is analogous to AlphaCode 2's high-temperature sampling → execution filter → clustering pipeline.

**Target:** 10× the diversity of template expansion at similar volume, with 100% compiler validity by construction.

### 3.2 Fictional Knowledge Graph Synthesis (for Prose/Research Lanes)

**Addresses:** Slop contamination (§2.5), Oracle problem for prose (§2.2)

For the `research-expert` lane and `populi-meta` lane — which are inherently prose-based and cannot be verified by a compiler — the [MENS Research Track Blueprint](mens-research-track-blueprint-2026.md) proposes generating **fictional knowledge graphs** and forcing the model to reason over them. The model must learn the *logic* of synthesis (A + B → C) without memorizing facts about real-world entities.

This eliminates the hallucination risk at training time: facts are fictional by construction, so "hallucinating" them is impossible. The reward signal shifts from "is this true?" to "is this compositionally valid given the premises?"

**Existing hook:** `vox-corpus research-gen` (referenced in the blueprint but not yet fully implemented).

### 3.3 Structured Incoherence Gating

**Addresses:** Oracle problem / Semantic drift (§2.2), Reward hacking (§2.7)

Every generated program that passes compilation must pass a secondary **incoherence check** before entering the training corpus. The 2026 AAAI "incoherence" metric evaluates internal consistency of program logic without requiring a test runner:

- Does the function body contradict the instruction's semantic intent?
- Are variables declared but never used?
- Does the return type mismatch the described behavior?

The `vox-eval` crate is the appropriate implementation surface. Until a native incoherence metric is implemented, a **frontier LLM curator call** can serve as a proxy — the same pattern used by Cosmopedia. Each synthetic program is checked by an API-accessible frontier model before promotion from Tier B to training input.

**VRAM cost:** Zero — frontier curator runs API-side, not locally.

### 3.4 Anchor Accumulation Policy (10–20% Golden Fixed Ratio)

**Addresses:** MAD / Mode collapse (§2.3)

As established in [MAD and Mode Collapse](research-cl-mad-mode-collapse-2026.md), recursive stability requires that golden human-authored examples constitute 10–20% of every fine-tuning batch. The existing `golden: 6` weight is intended to enforce this but is expressed as a *relative* weight, not an absolute floor.

**Concrete enforcement:** Add a pre-training validation gate that rejects any batch configuration where the golden lane contributes less than 10% of total samples (across all lanes by absolute count). This must be checked at batch construction time, not at YAML config time, since absolute counts depend on corpus file sizes.

**Implementation surface:** `mens/config/review-weight-policy.yaml` (already exists at 187 bytes; currently minimal) → extend with an `anchor_floor: 0.10` field that is enforced by the MENS training orchestrator.

### 3.5 `heal_pairs.jsonl` → DPO Training Loop

**Addresses:** Negative examples discarded (§2.8), Semantic drift (§2.2)

The healing loop in `healing.rs` already produces `HealPair` records with `(failed_source, diagnostics, repaired_source)` triples. These are the correct input format for **Direct Preference Optimization (DPO)**:

```
chosen:  repaired_source  (compiles, addresses diagnostics)
rejected: failed_source   (does not compile)
prompt:  description + compiler diagnostics
```

Wiring `heal_pairs.jsonl` into a DPO lane requires:

1. A new mix entry in `mix-vox-lang.yaml` with a `dpo` format flag
2. A DPO-aware training path in the MENS orchestrator (or an external DPO library call)
3. A balance policy: rejected samples must not exceed positive samples by more than 2:1

This immediately doubles the training signal extracted from every healing interaction without requiring new data collection.

### 3.6 Advanced PEFT: CURLoRA or FAPM for Sequential Runs

**Addresses:** Catastrophic forgetting (§2.6)

Replace standard LoRA within each domain's sequential training runs with one of:

- **CURLoRA** — initializes U-matrix as zero, uses inverted CUR probabilities as implicit regularization; maintains base model perplexity while adapting
- **FAPM** — prunes LoRA updates that heavily overlap pre-trained weight magnitudes; limits forgetting to 0.25% while preserving 99.67% downstream accuracy

Both are drop-in replacements at the adapter level and do not require changes to the YAML-driven domain profile system. Either could be selected via a new `peft_variant` field in `domain-profiles.yaml`.

> **Note:** O-LoRA (the cross-domain orthogonality enforcer from [Catastrophic Forgetting research](research-cl-qlora-catastrophic-forgetting-2026.md)) solves a different problem — preventing cross-domain interference in a *single* adapter. CURLoRA/FAPM solve *within-domain* sequential forgetting.

### 3.7 Automated Dogfood Flywheel Gate

**Addresses:** Volume threshold (§2.4), Loop automation (from MENS KI section 8)

The dogfood flywheel is currently manual: someone must run `vox populi corpus extract` and trigger a training run. Automating it requires:

1. A `vox-eval` quality threshold (e.g., `min_rating: 3`) as a gate on what enters the corpus
2. A background scheduler (or CI cron) that auto-runs corpus extract when new session logs accumulate above a configurable sample floor (e.g., 500 new traces)
3. A semantic entropy check on freshly extracted data to detect loop collapse before the training run begins

The `autofeedback.jsonl` lane (weight 3 in `mix-agents.yaml`) is the correct hook for this but requires the quality gate to prevent raw, unvetted session noise from entering the mix.

### 3.8 Cross-Pollination from Rust Corpus into Vox-Lang

**Addresses:** Volume threshold (§2.4)

The `rust-expert` domain has a richer real-world corpus (Rust source code, documentation, and pairs from the entire open-source Rust ecosystem). Vox-lang compiles *to* WebAssembly via a Rust-backed IR. Pairs of the form:

```
instruction: "Translate this Rust function to an equivalent Vox actor"
response:    <valid Vox actor>
```

...can be generated by the Vox compiler from real Rust source. The `vox-compiler` pipeline can already lower Rust FFI boundaries to Vox interface declarations. Every valid such translation is a high-quality cross-domain pair that increases `vox-lang` corpus volume without synthetic generation.

**This approach is uniquely powerful for Vox** because the semantic intent is grounded in real, author-verified Rust programs — not from an LLM's imagination.

archived_date: 2026-04-18
---

## 4. Risk Matrix: Mitigations vs. Failure Modes

| Failure Mode | Severity | Existing Defense | Proposed Mitigation |
|---|---|---|---|
| Template exhaustion / low diversity | High | Mix-lane weighting | AST-aware mutation (§3.1) |
| Syntactic-only oracle (hollow programs) | Critical | `vox-eval` ratings | Incoherence gating + curator LLM (§3.3) |
| MAD / mode collapse | Critical | Golden lane weight | 10–20% anchor floor policy (§3.4) |
| Volume below production threshold | High | `vox generate-data` | AST mutation + Rust cross-pollination (§3.1, §3.8) |
| AI slop in prose lanes | Medium | None currently | Fictional knowledge graphs + curator (§3.2, §3.3) |
| Catastrophic forgetting | High | Separate adapters | CURLoRA / FAPM in sequential runs (§3.6) |
| Reward hacking in GRPO | Critical | None currently | Incoherence gate + DPO lane (§3.3, §3.5) |
| Negative examples discarded | Moderate | `heal_pairs.jsonl` (inactive) | DPO wiring (§3.5) |
| Manual flywheel bottleneck | Medium | None currently | Automated eval-gated extraction (§3.7) |

---

## 5. Implementation Priority Ordering

> [!IMPORTANT]
> These are ordered by risk-reduction per implementation cost. Each requires an ADR or formal planning cycle before execution.

1. **Anchor floor policy** (§3.4) — pure YAML config change in `review-weight-policy.yaml` + orchestrator validation. Zero risk, immediate MAD protection.
2. **`heal_pairs.jsonl` → DPO lane** (§3.5) — the data already exists. Requires a DPO format adapter in the training path. Doubles signal extraction from existing production data.
3. **Incoherence gating via frontier curator** (§3.3) — API-only, no local infra required. Blocks the most critical failure mode (hollow-program reward hacking) before it poisons the corpus.
4. **AST-aware mutation** (§3.1) — extends the existing `datagen.rs` generator with a mutation pass. Significantly increases structural diversity without new infrastructure.
5. **Automated flywheel gate** (§3.7) — requires scheduler + `vox-eval` integration. Eliminates the manual corpus extract bottleneck.
6. **Rust → Vox cross-pollination pairs** (§3.8) — requires a translation pipeline but produces uniquely high-quality, semantically grounded pairs.
7. **CURLoRA / FAPM PEFT variant** (§3.6) — library-level change to the training backend. Highest engineering cost, but provides structural protection against the slow-boil catastrophic forgetting risk.

archived_date: 2026-04-18
---

## 6. Relationship to Existing Research Cluster

This document synthesizes and extends findings from the Continual Learning Flywheel cluster (Wave 2):

- [MAD and Mode Collapse](research-cl-mad-mode-collapse-2026.md)
- [The Compile-Pass Oracle and Semantic Degradation](research-cl-oracle-semantic-drift-2026.md)
- [Catastrophic Forgetting in QLoRA Fine-Tuning](research-cl-qlora-catastrophic-forgetting-2026.md)
- [The Risks of Agent-Generated Prose](research-cl-slop-typicality-bias-2026.md)
- [Minimum Viable Corpus Size for QLoRA Domain Adaptation](research-cl-qlora-minimum-corpus-2026.md)
- [Utilizing Parse Failures as Negative Examples](research-cl-nat-dpo-2026.md)

And extends findings from the GRPO cluster (Wave 3):

- [GRPO Reward Shaping for Code LLMs](research-grpo-reward-shaping-2026.md)

And the MENS multi-track KI:

- [MENS Architecture: Multi-Track vs. Omni Model Research](../../../../../../.gemini/antigravity/knowledge/mens_multitrack_research/artifacts/findings.md) (accessible via `vox_agent`)

---

*Document date: 2026-04-12. Update when: (a) a new corpus strategy is implemented, (b) a new domain profile is added, or (c) a production flywheel cycle reveals novel failure modes not covered here.*


