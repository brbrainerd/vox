---
title: "The Optimization Landscape of Positive-Only Training Loops"
description: "Research on positive-only code RL loops, negative reinforcement, and exploration failure modes."
category: "Architecture SSOTs"
status: "research"
research_source: "gemini_deep_research"
research_date: "2026-04-08"
training_eligible: false
last_updated: "2026-04-09"
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# The Optimization Landscape of Positive-Only Training Loops

The Vox MENS architecture proposes a "positive-only" training loop design, wherein only valid parses are permitted to generate a gradient signal within the RL environment, while invalid parses are sequestered, stripped of their RL context, and ingested as negative supervised examples in a separate SFT phase. The empirical evidence across 2025 and 2026 literature definitively establishes that this decoupled approach introduces severe optimization bottlenecks, degrades model calibration, and is demonstrably inferior to unified, on-policy RL objectives that natively process negative feedback.

### The "Pull-Up" Effect and Model Collapse

When a reinforcement learning algorithm is configured to only reinforce positive or successful trajectories, it induces a well-documented statistical phenomenon known as the "pull-up" effect.54 By exclusively updating the policy gradient based on successful code generation, the algorithm concentrates the model's probability mass entirely on the narrow subset of logical paths that the base model already knows how to navigate.55

This approach effectively ignores the vast, highly diagnostic data inherent in *why* a reasoning path failed.57 While positive-only feedback loops may temporarily boost raw accuracy on familiar benchmarks, they impose a severe epistemic calibration cost.55 The outcome of exclusively reinforcing correct paths is a manifestation of Model Collapse. The model's predictive behavior converges toward low-variance point estimates, intensely reinforcing its own biased, pre-existing beliefs while simultaneously discarding the distributional tails and alternative reasoning pathways that are absolutely necessary for reliable uncertainty estimation and complex logical deduction.55

Furthermore, separating invalid parses into a disconnected SFT phase fundamentally severs the temporal and contextual link between the policy's active state and the errors it generated. Because SFT operates via cross-entropy loss to force imitation—rather than optimizing a relative advantage—the SFT phase acts as a destabilizing force. It frequently induces catastrophic forgetting, actively overwriting the nuanced behaviors the model painstakingly acquired during the RL phase.54

### The Efficacy of Negative Sample Reinforcement (NSR)

The empirical consensus strongly favors unified, on-policy RL objectives that natively ingest both positive and negative feedback over decoupled SFT/RL approaches. A seminal 2025 study evaluating Qwen2.5 models demonstrated that incorporating *incorrect* reasoning trajectories (negative samples) directly into the gradient updates substantially improves Out-of-Domain (OOD) generalization.43

The research revealed 22 distinct recurring patterns in incorrect reasoning chains. When these negative trajectories are retained in the RL loop and penalized through Negative Sample Reinforcement (NSR), they effectively act as mathematical guardrails, mapping the boundaries of the solution space.43 By systematically suppressing incorrect generations through negative advantages, the model is forced to redistribute its probability mass toward alternative, plausible candidates, refining its existing knowledge base rather than simply repeating safe actions. Crucially, training exclusively on positive samples resulted in a 15.81% *worse* OOD performance compared to methods that natively integrated negative trajectories via Gain-based Loss Weighting (GLOW).43

### Balancing the Distribution: Anna Karenina Sampling and TOPR

Further research on Truncated Optimistic Policy Gradients (TOPR) proves that standard importance sampling fails precipitously when positive examples are sparse—a common occurrence in complex code generation tasks.59 When the effective proportion of positive examples is extremely low, the model tends to lower the probability of most trajectories in its training set, inadvertently suppressing the probability of the rare correct trajectories as well.59

To combat this, frameworks utilize "Anna Karenina sampling" to artificially construct training batches deliberately filled with negative examples (failed solutions) drawn from the model's own rollouts.59 By continuously forcing the model to evaluate and penalize its own specific failure modes, the RL loop maintains a higher policy entropy (increasing by up to 35%). This elevated entropy prevents catastrophic overfitting on trivial syntax and sustains the rigorous exploration necessary to discover novel, functionally correct algorithms.59

In code generation specifically, treating compilation and parse failures as hard negatives directly inside the PPO or GRPO objective creates a robust "contrastive" learning environment. The model learns exactly which tokens and structural choices cause a syntax error, rather than blindly learning that a specific, highly-formatted sequence is "good".61

*Evidence Quality Rating:* **Strong**. Extensive algorithmic literature from 2025 and 2026 (including GLOW, SPoT, NSR, and TOPR) precisely isolates the detrimental effects of positive-only training and provides mathematical proofs supporting unified negative reinforcement in reasoning LLMs.


