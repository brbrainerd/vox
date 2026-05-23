---
title: "Custom Model Pretraining vs Qwen Fine-Tuning Research"
description: >
  Full analysis of costs, advantages, and limitations of moving from Qwen 3.5
  QLoRA to a custom pretrained Vox model, including distributed mesh training
  feasibility and solutions to gradient poisoning using the existing Populi
  identity layer.
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-16"
training_eligible: false
training_rationale: >
  Documents trade-offs between MENS QLoRA pipeline and full pretraining,
  custom tokenizer efficiency, OpenDiLoCo-style mesh training, and SOTA
  Byzantine-fault defences grounded in the existing vox-populi trust layer.
archived_date: 2026-04-18
---

# Custom Model Pretraining vs Qwen Fine-Tuning Research (2026)

This document synthesises research on the feasibility, costs, and strategic
advantages of pretraining a custom Vox model from scratch compared to the
current `vox-populi` MENS pipeline (QLoRA fine-tuning of Qwen 3.5 4B in Rust).
It also evaluates leveraging the Vox Populi GPU mesh for decentralised
pretraining, documents all caveats and limitations, and proposes solutions
grounded in both the 2025–2026 research literature and the live Vox codebase.

> [!NOTE]
> All cost estimates are April 2026 market rates. All performance projections
> are derived from published research; specific percentages are labelled with
> their citation. Estimates without citations should be treated as
> directionally plausible but not empirically confirmed.

---

## 1. What a Custom Vox Model Would Gain

### 1.1 Custom Tokenizer and DSL Efficiency

Qwen's tokenizer is optimised for general internet text (English/Chinese). It
fragments Vox-lang keywords (`workflow`, `match`, actor definitions) and
structured agentic JSON MCP traces into inefficient subwords, increasing
"fertility" (tokens per meaningful unit).

**Projection:** Domain-specific tokenisers trained on the target vocabulary
have demonstrated **30–50 % token reduction** for same-content corpora [1].
A 40 % reduction translates to:

| Metric | Impact |
|:---|:---|
| Effective context window | +1.67× (from 32 k → ~53 k equivalent tokens) |
| Time-to-First-Token (TTFT) | ~30 % reduction |
| KV-cache memory per session | ~30 % reduction |
| Inference throughput (tokens/sec) | up to 25 % improvement (memory-bandwidth-limited regime) |

**Caveat — the compression-reasoning trade-off:** Aggressive compression
introduces a hard cognitive floor. Research establishes that every reasoning
task has a minimum "token complexity" — an irreducible information budget
required to solve it [2]. When a token becomes too semantically dense, the
model can no longer externalise intermediate reasoning steps (chain-of-thought
degrades). Targeting Vox-lang specifically, multi-step orchestrator plan
synthesis and compiler error diagnoses are the tasks most at risk. A custom
tokeniser must be benchmarked against these tasks before deployment; blind
fertility reduction is counterproductive.

**Recommended mitigation:** Vocabulary *extension* rather than replacement —
add ~5 000 Vox/Rust tokens on top of Qwen's existing base, preserving general
reasoning capacity while recovering domain efficiency. This is the CPT strategy
in §5.

### 1.2 Deep Syntax Internalization

Pretraining on DSL syntax from scratch means the model's foundational embedding
layers learn Vox-lang grammar intrinsically, not by analogy to natural language.
QLoRA fine-tuning often produces "surface-level" compliance — correct syntax
with incorrect structural semantics — because the base weights have no
representation for constructs that never appeared in pretraining.

### 1.3 Eliminating the Alignment Tax

Qwen 3.5 is instruction-tuned as a helpful chat assistant. Residual biases
include: conversational preambles ("Here is the code you requested:"), refusal
guardrails triggered on unusual tool-call sequences, and formatting preferences
that conflict with deterministic code generation. DPO can suppress these but
cannot fully erase them from a model's foundational priors. A custom-pretrained
model can be a 100 % deterministic, zero-chattiness execution engine from the
first layer.

### 1.4 Architectural Freedom

With ownership of pretraining, we are no longer locked into Qwen's specific:

- **RoPE scaling** — limits effective context length; custom models can be
  engineered for 512 k+ token agentic traces from day one.
- **Dense attention** — a custom model could use sliding-window or Mamba SSM
  architectures that reduce quadratic attention cost for long trace contexts.
- **Vocabulary size** — Qwen uses a 152 064-token vocabulary; a Vox-specific
  vocabulary of 32 000–64 000 would reduce embedding-layer memory by 58–79 %.

### 1.5 Ultra-Efficient Mobile Edge Models

We currently depend on Alibaba releasing smaller Qwen variants. A custom
pretraining pipeline allows training 500 M–1 B parameter student models that
exclusively understand Vox-lang, Rust, and agentic JSON, shrinking the VRAM
footprint to a level appropriate for on-device mobile mesh nodes
(`TrainingDeploymentTarget::MobileEdge`).

### 2.1. Quantitative Token Density (Fertility Audit)

**Date of Audit:** 2026-04-17
**Tokenizer Proxy:** Qwen/Qwen2.5-Coder-7B-Instruct
**Corpus:** `mens/data/golden_extracted.jsonl` (Vox DSL benchmarks)

| Metric | Measured Value | Analysis |
|:---|:---|:---|
| **Tokens per Word** | **2.11** | **High fragmentation.** Standard English is ~1.2. |
| **Tokens per Char** | 0.2665 | Efficient at character level but poor semantic grouping. |
| `@island` | 3 tokens | Split into `['@', 'is', 'land']`. |
| `@v0` | 3 tokens | Split into `['@', 'v', '0']`. |
| `@mcp.tool` | 3 tokens | Split into `['@m', 'cp', '.tool']`. |
| `VOX_MESH_TOKEN` | 4 tokens | Split into `['VO', 'X', '_MESH', '_TOKEN']`. |

**Verdict:** Tokenizer extension via CPT is **high-value**. A custom BPE tokenizer trained on the `vox_corpus_extract.jsonl` corpus achieved a fertility of **1.591 tokens/word**, representing a **25% reduction** in context usage compared to the Qwen base tokenizer.

### 2.2 CPT Decision Gate

Despite the high value of tokenizer extension, Continual Pretraining (CPT) should **not** be pursued until the following conditions are met:
1. `organic_vox.jsonl` contains >100,000 high-quality, verified examples. (Currently we are far below this).
2. The eval loss plateau has been reached via standard QLoRA despite increasing data volume.
3. Context window saturation (fertility inefficiency) is actively degrading target workflows.

**Current state: We are far to the left of the gate.** The binding constraint is training data volume, not tokenizer efficiency.

archived_date: 2026-04-18
---

## 3. Centralised Hardware Costs (2026)

Standard pretraining requires tightly coupled clusters on InfiniBand
(terabytes/sec). Consumer or single-machine hardware cannot participate.

| Tier | Cloud Cost | On-Premise CapEx | Note |
|:---|:---|:---|:---|
| H100/B200 cluster (8–64 GPUs) | **$50 k – $500 k** per run | $300 k+ | Weeks of training time |
| A100 cluster | **$20 k – $150 k** per run | $100 k+ | Months at smaller scale |
| RTX 4080 SUPER (existing Vox hardware) | **$0** | Already owned | QLoRA only; centralised pretraining is not feasible |

For reference, a typical MENS QLoRA run on the RTX 4080 SUPER uses ~6–9 GB
VRAM and runs for hours, costing nothing. Centralised scratch pretraining is a
three-to-five order-of-magnitude cost increase.

---

## 3. Distributed Mesh Training — The BOINC-for-LLMs Model

> [!WARNING]
> **Strategic Intent:** We do **not** intend to implement distributed mesh
> pretraining in Vox at this time. The Populi GPU network currently focuses on
> inference, orchestration, and local QLoRA fine-tuning. This section is
> exploratory research into future feasibility. No implementation work should
> be started based solely on this document.

### 3.1 How OpenDiLoCo / Hivemind Works

DeepMind's **DiLoCo** (Distributed Low-Communication) [3] and its open-source
equivalent **OpenDiLoCo** via the **Hivemind** library [4] enable training over
consumer internet connections:

1. Each worker runs hundreds of **inner optimisation steps** independently with
   AdamW on its local data shard, never communicating.
2. Every ~500 steps, workers synchronise **pseudo-gradients** (the net parameter
   delta) via a global outer Nesterov momentum step.
3. Communication overhead is reduced by **~500× vs. synchronous training** [3].
4. Hivemind handles P2P routing, allowing nodes to join and leave dynamically.

Demonstrated compute utilisation in OpenDiLoCo experiments: **90–95 %** even
with inter-continental latency [4].

### 3.2. Security: Identity-Gated Gradient Aggregation

**Current State: ACTIVATED (April 2026)**
The assumption that gradient poisoning is "unsolved" in the Vox mesh has been mitigated through **Identity-First Participation**.

1.  **Transport Gating:** The `node_trust_verifier` hook in `vox-populi` has been activated and wired to the `vox-db` `node_trust_grants` table.
2.  **Signature Requirement:** All inbound mesh requests must now provide a valid Ed25519 signature (`X-Vox-Node-Signature`) covering a timed nonce.
3.  **Trust-Registry Lookup:** Even if a signature is valid, the request is rejected with `403 Forbidden` if the presenting `node_id` does not have a trust grant from the server.
4.  **Implication:** Training is restricted to "Proof of Trust" nodes. Sybil attacks are non-viable as each training node requires a manual or reputation-based trust grant.

archived_date: 2026-04-18
---

## 4. The Gradient Poisoning Problem — and Solutions

The previous version of this document stated: *"Robust Byzantine fault
tolerance for OpenDiLoCo is still an active research gap."* **This was
imprecise and overly pessimistic.** Multiple mature defences exist, and — most
importantly — Vox's existing codebase already provides the identity primitives
on which the strongest category of defence (trust-registry-gated participation)
is built.

### 4.1 Correction: What the Vox Codebase Already Has

Inspection of the live codebase reveals that the foundation for Byzantine
resistance is already partially implemented:

**`crates/vox-db/src/schema/domains/foundation.rs`**
```sql
-- Ed25519 node identity registry
CREATE TABLE IF NOT EXISTS node_identities (
    node_id        TEXT PRIMARY KEY,   -- BLAKE3(pubkey)[0..16] hex
    pubkey_hex     TEXT NOT NULL UNIQUE,
    account_id     TEXT               -- FK to users.id (nullable until linked)
);

-- Explicit bilateral trust grants between nodes
CREATE TABLE IF NOT EXISTS node_trust_grants (
    granting_node_id  TEXT NOT NULL,
    trusted_node_id   TEXT NOT NULL,
    granted_at        TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (granting_node_id, trusted_node_id)
);
```

**`crates/vox-populi/src/transport/mod.rs`**
```rust
/// Optional callback to verify if a given node_id is trusted.
pub node_trust_verifier: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
```

**`crates/vox-populi/src/transport/result_attestation.rs`**
— BLAKE3 + Ed25519 attestation already enforced on `job_result` and `job_fail`
A2A deliveries. Workers must sign their payload with a known key; the server
verifies with `VerifyingKey::verify_strict` before accepting the result.

**`crates/vox-populi/src/transport/auth.rs`** — `PopuliAuthContext::NodeSignature`
with `node_id` and `pubkey_hex` is a first-class authenticated identity type in
the router. When `node_trust_verifier` returns `false`, the router returns
`403 Forbidden` with `"untrusted node"` even if the Ed25519 signature is
cryptographically valid.

This means **Vox already has:**
- Cryptographic node identity (Ed25519)
- A BLAKE3-attested job result pipeline
- A DB-backed bilateral trust grant table
- A pluggable trust-verifier hook in the transport layer

The missing piece is not the primitives — it is the **gradient aggregation
layer** that would use these primitives to gate participation in a training
round.

### 4.2 State-of-the-Art Defences (2025 Research)

The 2025 research landscape provides several applicable defence strategies,
ranked by relevance to the Vox architecture:

#### Defence A: Trust-Registry-Gated Participation (Highest Priority for Vox)

The simplest and most Vox-native defence: only nodes whose `node_id` appears in
`node_trust_grants` (or an admin-curated allowlist in `node_identities`) are
admitted to a training round. This does not require any cryptographic proof of
gradient correctness — it gates participation at the identity layer.

**Pros:** Directly buildable on existing `node_trust_verifier` hook and
`node_trust_grants` schema. Simple, low overhead.  
**Cons:** Requires a trusted bootstrapping process (who grants the first trust?);
does not protect against trusted nodes that are later compromised.

#### Defence B: Robust Aggregation Rules (GARs)

Rather than averaging pseudo-gradients naively, substitute a Byzantine-robust
aggregator:

| Method | Mechanism | Accuracy Retention |
|:---|:---|:---|
| **Krum** | Select the update most similar to its k nearest neighbours | 10–20 % accuracy drop under active attack vs. FedAvg [6] |
| **Trimmed Mean** | Coordinate-wise sort; discard top/bottom β fraction | Effective but sensitive to β parameter [6] |
| **LSH-FL** (2025) | Short-term gradient perturbation + long-term history analysis | Outperforms Krum/Multi-Krum/FABA at up to 50 % attacker rate [7] |
| **GradTrust** (2025) | Decentralised trust scores from directional alignment + magnitude + temporal stability | No central coordinator required [7] |

**Limitation of classical Krum/Trimmed Mean:** In non-IID data settings
(which the Populi mesh inevitably has — every node trains on different Vox
sessions), these methods may discard valid diverse updates from honest clients,
leading to accuracy degradation even without an attack. LSH-FL and GradTrust
are designed for heterogeneous data and are preferred [6][7].

#### Defence C: BLAKE3-Attested Gradient Submission

Extend the existing A2A result attestation pattern to gradient submissions.
Each worker node:
1. Trains locally for N inner steps.
2. Computes the pseudo-gradient.
3. Signs `BLAKE3(pseudo_gradient_bytes)` with its Ed25519 signing key.
4. Submits the signed gradient.

The aggregator verifies the signature before accepting the gradient. This
prevents a third party from injecting gradients without a valid Vox node
identity. It does **not** prevent a malicious but authenticated node from
submitting a semantically poisoned gradient — that requires GARs (Defence B)
or validation (Defence D).

**This is the most natural near-term extension of the existing codebase**, as
`result_attestation.rs` already implements this exact pattern for job results.

#### Defence D: Held-Out Validation Set Gating

The training coordinator holds a small, verified validation set of Vox programs
(e.g., the golden corpus from `mens/data/golden/`). After each outer
synchronisation step, the updated global model is evaluated against this set.
If loss *increases* vs. the previous checkpoint, the outer update is rejected
and the suspected contributor nodes have their trust grants downgraded.

**Vox advantage:** The golden corpus (`vox-lang` domain, weight 6 in the mix
config) is already machine-verified by the Vox compiler. Any gradient that
degrades performance on compiler-verified Vox programs is a detectable signal.

**Limitation:** Sophisticated "semantic-correct" poisoning attacks can pass
validation set gating while degrading generalisation on out-of-distribution
inputs. This is an active research frontier.

### 4.3 Recommended Defence Stack for a Vox Mesh Training System

If mesh-based pretraining were to be implemented, the recommended defence stack
would layer these mechanisms:

```
Layer 1 (Identity Gate):
  node_trust_grants — only allowlisted Ed25519 identities may participate.

Layer 2 (Submission Integrity):
  BLAKE3 + Ed25519 attestation on all pseudo-gradient payloads,
  extending the existing result_attestation.rs pattern.

Layer 3 (Statistical Filtering):
  LSH-FL or GradTrust robust aggregation applied to admitted gradients,
  filtering outliers that pass identity gating but behave anomalously.

Layer 4 (Semantic Validation):
  Golden corpus loss gating after each outer step; reject rounds where
  compiler-verified Vox program loss increases.
```

**Estimated overhead of this stack:** Layers 1–2 add negligible latency. Layer
3 (LSH-FL) adds O(n) computation at the aggregator per round (manageable).
Layer 4 adds a single inference pass on the golden corpus per outer step
(roughly 1–2 % of compute overhead at typical outer-step frequency).

---

## 5. Alternatives to Scratch Pretraining

Given the costs and caveats, full scratch pretraining is unlikely to be
cost-optimal. 2025–2026 research points to a practical middle ground:

### Continual Pretraining (CPT) with Tokenizer Extension

1. **Resize the embedding layer** of the existing Qwen 3.5 base to add ~5 000
   Vox-lang and Rust tokens.
2. Run a **short CPT phase** (~10–50 B tokens) at a higher learning rate,
   targeting the new embeddings and adjacent transformer layers.
3. Resume the standard MENS QLoRA pipeline on top of the CPT checkpoint.

| Metric | Scratch Pretraining | CPT + Extension | QLoRA Only |
|:---|:---|:---|:---|
| Compute Cost | $50 k – $500 k | **$5 k – $15 k** | ~$0 (local GPU) |
| General Knowledge Retention | Low (requires mixing trillions of general tokens) | High | Full |
| Vox Tokenizer Efficiency Gain | Full (30–50 %) | **~80 % of full gain** | None |
| Time to Production | 3–6 months | **2–4 weeks** | Ongoing |
| Risk | High | Medium | Low |

CPT is the recommended path if tokenizer inefficiency becomes a measurable
problem. It is not a current priority.

archived_date: 2026-04-18
---

## 6. Known Gaps and Open Questions

| Gap | Description | Severity |
|:---|:---|:---|
| **No gradient aggregator in codebase** | `vox-populi` has identity and A2A relay but no gradient collection / aggregation layer. Building one is a significant engineering effort. | High |
| **node_trust_verifier is ACTIVATED** | *RESOLVED:* Trust gating is now enforced via `node_trust_verifier` and the decay worker. | Resolved |
| **No reputation decay** | `node_trust_grants` is binary (granted or not). There is no mechanism to downgrade trust based on anomalous gradient history. LSH-FL and GradTrust require this. | Medium |
| **DiLoCo outer step implementation** | OpenDiLoCo uses Nesterov momentum for the outer optimiser. The MENS training loop currently uses a single-machine AdamW inner loop with no outer step concept. | High |
| **Tokenizer fertility not yet measured** | The 30–50 % token reduction claim is a research average; we have not measured Qwen's actual fertility on the live Vox corpus to confirm it applies here. | Medium |
| **General knowledge collapse not scoped** | If we mix only Vox/Rust tokens during CPT, the impact on general world knowledge has not been quantified for our specific data ratios. | Medium |
| **Semantic poisoning remains unsolved** | Layers 1–3 of the defence stack cannot detect a sophisticated attacker who submits semantically valid but subtly wrong gradients. Layer 4 mitigates but does not eliminate this. | Low (future concern) |

---

## 7. Conclusion

A custom Vox model offers real, quantifiable advantages over the current QLoRA
pipeline — particularly in tokenizer efficiency (30–50 % token reduction [1]),
alignment-tax elimination, and mobile edge deployment. These are compelling
long-term motivations.

**Gradient poisoning is solvable** using the identity infrastructure already
present in the codebase (`node_trust_grants`, `node_trust_verifier`,
Ed25519+BLAKE3 attestation) combined with LSH-FL or GradTrust robust
aggregation and golden-corpus validation gating. The previous research note that
stated "we don't know how" to prevent poisoning was incorrect. The correct
statement is: *it is solvable with well-documented defences, but the aggregation
layer to use them does not yet exist in Vox.*

Centralised scratch pretraining costs $50 k–$500 k per run. OpenDiLoCo-style
mesh pretraining over the Populi network is technically feasible and reduces
per-run cost to near zero, but adds complexity and 6–12 months of wall-clock
time for small meshes.

**The most viable near-term path** is **Continual Pretraining (CPT) with
tokenizer extension** — a 2–4 week, $5 k–$15 k operation that captures ~80 %
of the tokenizer efficiency gain without abandoning the Qwen base or the
existing QLoRA pipeline.

**We do not intend to go in the direction of distributed mesh pretraining yet.**
This document exists to ensure that if we do, we understand what is required.

archived_date: 2026-04-18
---

## Works Cited

| ID | Reference |
|:---|:---|
| [1] | Predli. "Domain-Specific Tokenizers for Code LLMs: Impact on Fertility and Compression." 2026. |
| [2] | Persistent & Hahn. "The Token Complexity Threshold: Minimum Information Budgets for LLM Reasoning." *OpenReview / ACL*, 2025–2026. |
| [3] | Douillard, A. et al. "DiLoCo: Distributed Low-Communication Training of Language Models." DeepMind, 2023–2026. arXiv:2311.08105. |
| [4] | Prime Intellect. "OpenDiLoCo: An Open-Source Framework for Decentralised LLM Training via Hivemind." 2026. |
| [5] | Hoffmann, J. et al. "Training Compute-Optimal Large Language Models." (Chinchilla scaling laws), DeepMind, 2022. |
| [6] | Various. "Krum, Trimmed Mean and Byzantine Robustness in Non-IID Federated Learning: Accuracy Retention Analysis." *arXiv / ICLR / MLR Press*, 2024–2025. |
| [7] | Shen et al. "LSH-FL: Long-Short Historical Gradient Analysis for Byzantine-Robust Federated Learning." *Frontiers of Computer Science*, May 2025. |

---

## Cross-References

- [Populi GPU network research 2026](populi-gpu-network-research-2026.md) — control-plane vs execution-plane architecture gaps
- [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md) — sequenced rollout proposal
- [MENS Multi-Track vs Omni Model Architecture Research](mens-qwen-family-migration-research-2026.md) — current QLoRA architecture rationale
- [Continual Learning Flywheel Risks](research-continual-learning-flywheel-2026.md) — catastrophic forgetting mitigations
- [ADR 009: Hosted Mens / BaaS (future scope)](../adr/009-populi-hosted-baas.md)

