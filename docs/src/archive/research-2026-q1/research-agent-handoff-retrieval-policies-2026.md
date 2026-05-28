---
title: "Evidence Base for Context Retrieval Policies"
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
**4\. Evidence Base for Context Retrieval Policies**

*Evidence Quality Rating: High (Derived from peer-reviewed NLP conferences such as ICLR 2024/2025, EMNLP, and large-scale benchmarks like HotpotQA and 2WikiMultiHopQA).*  
The platform's vulnerability regarding "policy duplication" arises from a lack of systematic guidance on when an agent should rely on internal working memory versus when it must execute an external retrieval. The naive "always retrieve" paradigm (Standard RAG) severely degrades performance on simple or multi-hop tasks by flooding the context window with "hard distractors," diluting attention, and increasing latency and token costs unnecessarily.9

### **4.1 Retrieve-on-Demand (Self-RAG)**

Self-RAG (Self-Reflective Retrieval-Augmented Generation, 2023\) pioneered the "retrieve-on-demand" strategy. It trains a language model to adaptively retrieve passages only when necessary by generating explicit reflection tokens (e.g., , , \`\`). The model actively assesses its own uncertainty and critiques both the retrieved passages and its own generations.52

* **Empirical Evidence:** Self-RAG achieved a massive reduction in hallucinations (down to 5.8% in localized tests) and significantly outperformed naive RAG and state-of-the-art LLMs on open-domain QA and fact verification tasks.52  
* **Failure Modes:** Relying on the primary generation model for continuous self-reflection introduces extreme computational overhead. Passing entire sequences through heavy models simply to decide *whether* to retrieve wastes FLOPs and increases latency substantially, sometimes adding up to 220ms per reflection loop.53 Furthermore, it requires specialized fine-tuning on reflection data.

### **4.2 Corrective and Evaluative Retrieval (CRAG)**

Corrective Retrieval-Augmented Generation (CRAG, 2024\) decouples the retrieval assessment from the main generation model. It utilizes a lightweight, independent retrieval evaluator to score retrieved chunks into three confidence tiers: Correct, Incorrect, or Ambiguous.

* **Mechanisms:** If the context is scored 'Correct', a refiner extracts the pertinent information. If 'Incorrect', the system bypasses the vector results and autonomously triggers web-search fallbacks to find accurate data. If 'Ambiguous', both vector results and web searches are utilized.55  
* **Empirical Evidence:** CRAG's plug-and-play architecture robustly mitigates issues of retrieval noise and irrelevant context. Tiny-Critic RAG (an optimized evolution of CRAG) demonstrated a 94.6% reduction in routing overhead latency (from 785ms down to 42ms) compared to heavy-model reflection, making the evaluation step nearly imperceptible while maintaining high accuracy.54

### **4.3 Advanced Frameworks and Policy Selection Guidance**

Recent advancements like SEAL-RAG ("replace, don't expand") fight context dilution by actively swapping out distractors for gap-closing evidence under a fixed retrieval depth, improving answer correctness by up to 13 percentage points over Self-RAG on complex benchmarks like HotpotQA.57 Similarly, SCIM (Quality-Driven Convergence) integrates multi-dimensional quality assessment (relevance, faithfulness, completeness) into the iterative loop, adaptively terminating retrieval based on multi-dimensional assessment rather than single-dimensional confidence scores.58  
Empirical data from the RAGRouter-Bench and related studies provides clear guidance on policy selection based on query intent and task properties 56:

| Policy Strategy | Ideal Task Properties | Empirical Justification |
| :---- | :---- | :---- |
| **Trust Memory (LLM-Only)** | Highly abstract summarization, creative formatting, or tasks where the required working context is already fully loaded into an isolated sub-agent's state. | Avoids attention dilution and latency penalties. Cost is 1.0x baseline.59 |
| **Retrieve-on-Demand (Self-RAG / Adaptive)** | Complex, multi-hop reasoning where the agent must evaluate step one before knowing what to query for step two. Vague or exploratory queries. | Allows dynamic adjustment of reasoning depth and prevents over-retrieval on simple queries. Requires robust reflection mechanisms.52 |
| **Corrective Retrieval (CRAG)** | High-stakes factual queries (e.g., financial data, compliance) where the cost of hallucination outweighs the latency of evaluation. | Explicit filtering of low-confidence documents and automated fallback to external search guarantees higher factual integrity.55 |

## ---

*(Original Source: AI Agent Context and Handoff Research)*

