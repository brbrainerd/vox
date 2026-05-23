---
title: "LLM Output Mediation and Programmatic Validator Generation"
last_updated: "2026-04-11"
research_source: "mixed (web research + codebase cross-reference)"
category: "Architecture SSOTs"
description: >
  Comprehensive research on the problem of having one consistent, extensible system
  to mediate between LLM outputs (non-zero error probability) and validated, deterministic
  downstream system responses. Covers programmatic validator generation, dynamic schema
  derivation, how existing Vox crates already participate in this pattern, and a roadmap
  for a unified 'LLM Mediation Layer' that can extend or reduce MCP necessity across the
  codebase.
research_date: "2026-04-11"
status: "research"
training_eligible: false
training_rationale: >
  Synthesises the architecture constraints and trade-offs of bridging probabilistic LLM
  outputs to deterministic validated responses, directly impacting how Vox agents, skills,
  and MCP tools are built.
cross_references:
  - research-grammar-constrained-decoding-2026.md
  - trust-reliability-layer.md
  - hitl-doubt-loop-ssot.md
  - capability-registry-ssot.md
  - mcp-vox-language-exposure.md
  - rag-and-research-architecture-2026.md
  - research-grpo-reward-shaping-2026.md
  - vox_agentic_loop_and_mens_plan.md
  - research-diagnostic-questioning-2026.md

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# LLM Output Mediation and Programmatic Validator Generation

## 1. The Core Problem

Large language models are probabilistic functions. Every invocation of an LLM — regardless of
provider, model size, or temperature setting — carries a **non-zero probability of producing
output that is syntactically malformed, semantically incorrect, or structurally inconsistent**
with the expected contract of the calling system. This is not an edge case: it is an
architectural invariant that must be handled as first-class business logic.

The specific failure the user identifies is this:

> We start with an LLM to choose a method of operation, but it has the possibility of error
> (non-zero), so we have to handle that in ways we would not otherwise need to. How can we
> apply this broadly to the entire codebase and mediate, in a more extensible way, the common
> problem of going between an AI and handling the layer where we need a definite set of
> responses and a validator?

This document synthesises web research with a cross-reference of the current Vox codebase to
answer that question, document existing solutions, identify gaps, and propose a unified
**LLM Mediation Layer (LML)** architecture.

---

## 2. The Universal Pattern: The Mediation Sandwich

Industry-wide convergence in 2025–2026 has settled on a pattern referred to informally as
the "Validation Sandwich" or, more architecturally, the **Mediation Layer** pattern. Its three
mandatory tiers are:

| Tier | Kind | Mechanism | What it catches |
|---|---|---|---|
| **1 – Syntactic (generation-time)** | Hard constraint | Constrained decoding (FSM / Earley / PDA), native provider structured output mode | Completely malformed output: wrong types, missing required fields, non-enum values |
| **2 – Semantic (application-time)** | Rule-based deterministic | Typed parsing + programmatic validation rules | Logically inconsistent values that pass schema: negative prices, impossible date ranges, cross-field contradictions |
| **3 – Reflective (feedback loop)** | Probabilistic (secondary LLM or symbolic) | LLM-as-judge, RLVR verifier, constraint-feedback repair loop | Complex subjective/nuanced failures the type system cannot express |

The key insight is: **you cannot rely on any single tier alone**. Each tier has a different cost
profile, failure mode, and applicability. Structuring the codebase to compose these tiers is
the goal.

### 2.1  Why MCP Alone Is Insufficient

MCP (Model Context Protocol) defines tool surfaces as JSON Schema-described contracts. It solves 
**discovery and invocation** of tools, but it does not guarantee that the LLM correctly populates 
the required arguments, nor does it validate that the result returned by the tool is semantically 
coherent when fed back to the LLM. MCP is the *declaration* of an interface; the mediation layer 
is the *enforcement* of it.

The problem with MCP as currently practiced in Vox:

1. **Each MCP tool is its own validation island.** Tools contain ad-hoc argument guards, but 
   there is no shared infrastructure to express, compose, or test validators.
2. **Repair loops are absent or implicit.** When an LLM provides a malformed tool call, MCP 
   returns an error, but there is no systematic mechanism to feed that error back to the LLM 
   with structured repair context.
3. **Validators are never generated programmatically.** For each new capability, a developer 
   must write both the tool definition and the validation logic manually. This is expensive and 
   inconsistently applied.

archived_date: 2026-04-18
---

## 3. State of the Art in Programmatic Validator Generation (2025–2026)

### 3.1  Generation-Time Constrained Decoding

The dominant 2026 state of the art for Tier 1 validation uses **token-level logit masking** 
driven by a parser that maintains a live parse state. The three leading approaches:

| System | Architecture | Latency | Ideal for |
|---|---|---|---|
| **XGrammar-2** | JIT Earley + PDA with repetition compression | <40µs/token | Dynamic per-request schema changes |
| **llguidance** | Earley + regex-derivative lexer (Rust) | ~50µs/token | Static grammars, low startup cost |
| **Outlines** | FSM / regex lexer | High first-token latency | Simpler schemas, rare grammar change |

Vox already has `vox-constrained-gen` implementing an **Earley parser** and **Pushdown Automaton** 
backend, as well as a `DeadlockWatchdog` and `RevisionSampler`. This is architecturally correct 
and matches the recommended approach. The existing `GrammarMode` enum already distinguishes 
`Json`, `Vox`, and `VoxPda` modes.

**Gap:** `GrammarMode::Json` still delegates to the legacy `JsonGrammarAutomaton` in `vox-populi` 
rather than using the same Earley/PDA pipeline with a dynamically compiled JSON schema grammar. 
This creates an asymmetry: custom Vox grammar uses the modern stack, while JSON validation 
(which is more common in LLM output) still uses a separate, potentially outdated path.

### 3.2  Typed Schema Derivation

In Rust the canonical path is `#[derive(JsonSchema, Deserialize)]` via `schemars`, converting 
Rust types to JSON Schema at zero runtime cost. `vox-jsonschema-util` already centralises 
`compile_validator` and `validate` around the `jsonschema` crate. However:

- **`schemars` is not yet used to drive `vox-constrained-gen` at inference time.** The 
  generation-time constraint grammar is compiled from EBNF, not from a live Rust type 
  derivation. For non-Vox-language tasks (e.g., "classify this task into one of these 
  categories"), a `schemars`-derived grammar would be ideal.
- **No unified `ValidatedOutput<T>` wrapper exists.** Each consumer of LLM output re-implements 
  parsing and validation ad hoc.

The industry solution (Python: Instructor/Pydantic; TypeScript: Zod; Rust: rstructor) is a 
**schema-first extraction pipeline**: define your output type, derive the schema, pass the 
schema to the LLM, parse and validate the response, retry on failure. Vox needs a native Rust 
equivalent.

### 3.3  Repair Loops

The standard production repair loop:

```
attempt 0:
  prompt → LLM → parse() → validate() → return Ok(result)

attempt n (on failure):
  [original prompt] + [malformed output n-1] + [validation error n-1] → LLM
  → parse() → validate() → return Ok(result) | escalate if n > max_retries
```

Key properties:
- **Max retry budget** (typically 2–3). Never infinite.
- **Error is injected into the next prompt**, not merely suppressed.
- **Fail-fast on structural failure, escalate on semantic failure.** Different error classes 
  warrant different remediation policies.

Vox's HITL doubt loop (`vox_doubt_task` → `TaskStatus::Doubted`) handles escalation to human 
review, which is the correct terminal state. The path from *validation failure → repair attempt 
→ HITL escalation* needs to be explicit infrastructure rather than per-agent convention.

---

## 4. How Vox Already Participates in This Pattern

The Vox codebase has sophisticated partial implementations across several layers. Rather than 
building from scratch, the opportunity is to **connect existing subsystems into a coherent 
architectural seam**.

### 4.1  `vox-constrained-gen` — Tier 1 (Generation-Time)

**What it does:** Provides `ConstrainedSampler` trait with Earley and PDA backends. Plugs into 
the populi inference server to mask invalid tokens in real-time. Includes `DeadlockWatchdog` 
(timeout-based deadlock prevention) and `RevisionSampler` (mid-generation backtrack via a 
special revision token). Directly implements the "Stream of Revision" pattern from the 
grammar-constrained decoding research.

**What it lacks:**
- Dynamic schema-driven grammar compilation: `GrammarMode` is a closed enum, not a 
  registerable factory. Adding a new constrained output type requires modifying the enum.
- Integration with `vox-jsonschema-util`: the `Json` mode in `GrammarMode` is a stub that 
  defers to `vox-populi`'s legacy automaton, not to the Earley/PDA stack.
- Per-request grammar injection: the grammar is compiled once at startup, not derived 
  dynamically from the schema of the expected output type.

### 4.2  `vox-socrates-policy` — Tier 2 (Semantic, Risk-Based)

**What it does:** Provides `ConfidencePolicy`, `RiskBand`, `RiskDecision` (Answer / Ask / 
Abstain), information-theoretic clarification selection via `QuestioningPolicy`, and Shannon 
entropy math. Also provides `SocratesComplexityJudge` and `ConfidencePolicyOverride` for 
task-specific policy adjustment.

This is a **metacognitive layer** — it evaluates the *quality* of the evidence backing an LLM 
decision, not just the structural correctness of the output itself.

**What it lacks:**
- Connection to Tier 1 failure signals. If `vox-constrained-gen` produces a deadlock or 
  `RevisionDepthExceeded`, neither feeds into Socrates confidence scoring.
- Domain-specific policy profiles. There is a single `ConfidencePolicy::workspace_default()`. 
  Different task classes (code generation vs. classification vs. research) warrant different 
  thresholds.

### 4.3  `vox-orchestrator/src/validation.rs` — Post-Task Gate

**What it does:** Uses TOESTUB, LSP diagnostics, and `cargo check` as post-task validators, 
blocked behind the `toestub-gate` feature flag. Returns `ValidationResult { passed, error_count, 
warning_count, report }`.

**What it lacks:**
- This validator only runs *after* a task is "complete" — it is not part of the 
  per-inference output validation loop. An agent can complete dozens of LLM calls without 
  any intermediate validation.
- No connection to the repair loop. When `post_task_validate` fails, the caller must 
  decide what to do; there is no standardised retry protocol.

### 4.4  `vox-jsonschema-util` — Schema Compilation

**What it does:** `compile_validator` and `validate` thin wrappers around the `jsonschema` 
crate, with `anyhow` context chains.

**What it lacks:**
- Cannot directly drive generation-time constraints; only does post-hoc validation.
- Not integrated with `schemars::schema_for!()` to produce the schema from Rust types 
  automatically.

### 4.5  `vox-orchestrator/src/socrates.rs` — Evidence Envelope

**What it does:** `evaluate_socrates_gate` + `SocratesTaskContext` + `SocratesGateOutcome`. 
Synthesises retrieval evidence quality, contradiction ratio, and fatigue signals into a 
normalised confidence score and `RiskDecision`. Used to decide whether an agent's response 
quality meets the bar for completion.

**What it lacks:**
- This runs at task-completion time, not at individual inference-step time. An agent that 
  calls an LLM 10 times before completing only gets gated once.
- No connection to the structured output validation results of individual calls.

### 4.6  Trust Layer — Longitudinal Signal

**What it does:** `trust_observations` + `trust_rollups` (EWMA) track per-entity reliability 
over time. Feeds routing decisions.

**What it lacks:**
- No per-validator-kind tracking. We know an agent failed overall, but not whether it failed 
  due to schema non-conformance, semantic policy violation, or hallucination. Knowing the 
  failure class enables targeted improvement.

archived_date: 2026-04-18
---

## 5. The Gap: No Unified `LlmMediator<T>` Abstraction

The most significant architectural gap is the absence of a **single composable abstraction** 
that any call site can use to:

1. Express "I expect the LLM to return type `T`."
2. Produce a constrained grammar/schema for `T` automatically.
3. Invoke the LLM under that constraint.
4. Parse and validate `T` at the application boundary.
5. On failure, run a bounded repair loop with error context injected.
6. On repair exhaustion, escalate to Socrates → HITL doubt.
7. Record the outcome into the trust layer.

Without this abstraction, every call site (MCP tool handler, skill, planner, Scientia research 
loop) must re-implement some subset of these steps. The result is inconsistent validation 
coverage, inconsistent retry semantics, and trust data that doesn't capture per-call failure 
modes.

---

## 6. Proposed Architecture: The Vox LLM Mediation Layer (LML)

### 6.1  Design Principles

1. **Schema-first.** The output contract (`T`) is the canonical artefact. Everything else 
   (grammar, prompt addendum, validator, repair template) is derived from `T`.
2. **Composable tiers.** Each of the three validation tiers is independently pluggable. 
   A caller can use only Tier 1 (generation-time constraint) or all three.
3. **Fail-forward with structured error context.** Validation failures are not exceptions; 
   they are typed values that flow into the repair loop.
4. **Type-safe state transitions.** The TypeState pattern in Rust ensures that unconstrained 
   raw output can never accidentally be used as validated output.
5. **Reduces MCP boilerplate.** If the mediation layer can automatically derive a validator 
   from the declared output type, MCP tool handlers become thin shims that declare intent 
   and delegate all validation logic to the LML.

### 6.2  Core Types

```rust
/// Erased schema handle — can be compiled from schemars or EBNF.
pub trait OutputSchema: Send + Sync {
    fn json_schema(&self) -> serde_json::Value;
    fn grammar_mode(&self) -> Option<GrammarMode>;
}

/// A validated, type-safe result from one LLM mediation round.
pub struct Mediated<T> {
    pub value: T,
    pub attempts: u8,
    pub final_confidence: f64,
}

/// Tier-3 repair policy: controls the feedback-loop budget.
pub struct RepairPolicy {
    pub max_attempts: u8,
    pub inject_error_context: bool,
    pub escalate_to_hitl: bool,
}

/// The central mediator.
pub struct LlmMediator<T> {
    schema: Arc<dyn OutputSchema>,
    semantic_validators: Vec<Box<dyn SemanticValidator<T>>>,
    repair_policy: RepairPolicy,
    socrates_policy: ConfidencePolicy,
    trust_sink: Option<Arc<dyn TrustSink>>,
    _marker: PhantomData<T>,
}

impl<T: DeserializeOwned + JsonSchema> LlmMediator<T> {
    /// Derive schema, grammar mode, and validator from Rust type T.
    pub fn from_type() -> Self { ... }
    
    /// Execute a single mediated LLM call.
    pub async fn call(
        &self,
        prompt: &str,
        client: &dyn LlmClient,
    ) -> Result<Mediated<T>, MediationError> { ... }
}
```

The TypeState guarantee:
```rust
// Only a Mediated<T> (not a raw &str) can be passed downstream.
fn consume_classification(result: Mediated<TaskClassification>) { ... }
```

### 6.3  Tier Integration Map

```
           ┌─────────────────────────────────────────────────────┐
           │              LlmMediator<T>                         │
           │                                                     │
           │  schema = schemars::schema_for!(T)                  │
           │  grammar = vox_constrained_gen::build_sampler(mode) │
           │                                                     │
  prompt ──►  [Tier 1] constrained generation                    │
           │         ↓ raw structured text                       │
           │  [Tier 2] serde_json::from_str + jsonschema        │
           │         ↓ typed T                                   │
           │  [Tier 2b] SemanticValidator trait impls           │
           │         ↓ validated T                              │
           │  [Tier 3 on failure] repair_loop(error_context)    │
           │         ↓ repair prompt → back to Tier 1           │
           │  [Socrates] evaluate_socrates_gate()               │
           │         ↓ RiskDecision                             │
           │  [Trust] trust_observations.insert()               │
           └─────────────────────────────────────────────────────┘
```

### 6.4  Programmatic Validator Derivation

The `SemanticValidator<T>` trait is the extensibility surface:

```rust
pub trait SemanticValidator<T>: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, value: &T) -> Result<(), ValidationFailure>;
}
```

Validators can be:

- **Derived from the type**: for `enum` types, the JSON schema already enforces the finite 
  response set; no additional validator is needed.
- **Derived from the task**: for a code-generation task, a compile check (already in 
  `vox-orchestrator/src/validation.rs`) is a `SemanticValidator` for `VoxSourceFile`.
- **Derived from the trust layer**: past reliability data on specific agents or models 
  can adjust `ConfidencePolicy` thresholds.
- **Programmatically generated at call time**: for dynamic tasks (e.g., "return one of 
  the following five options based on this list"), build a `JsonEnumValidator` from the 
  option list at runtime instead of defining a static Rust enum.

The last case is the key to **automating MCP reduction**: instead of writing a separate 
MCP tool for each task that needs a bounded response, you instantiate a typed 
`LlmMediator<DynamicEnum>` where `DynamicEnum` is constructed from the live option set.

### 6.5  MCP Position in This Model

MCP's role becomes narrower and cleaner:

| Before LML | After LML |
|---|---|
| Each MCP tool handler validates its own arguments | Tool handlers declare output type; LML validates |
| Validation logic duplicated across dozens of tools | Single `LlmMediator<T>` per output type |
| Repair to human is manual and per-tool | Repair loop is systematic and configurable |
| Trust tracking per-task but not per-tool-call | Trust tracking per mediation round |
| MCP needed for every new LLM-facing interface | LML can generate a transient tool spec on the fly |

MCP continues to be necessary for **external tool exposure** (IDE clients, external agents, 
CLI bridges). It is not necessary for internal-to-orchestrator LLM calls, which can use 
the LML directly.

archived_date: 2026-04-18
---

## 7. Dynamic Validator Generation: The Finite Response Set Problem

### 7.1  The Problem in Concrete Terms

Consider the orchestrator routing step: the LLM must choose one agent from a set of N 
available agents. Today, the routing code passes a prompt that lists agents, and then 
parses the LLM's response to extract a choice. If the LLM hallucinates an agent name that 
is not in the set, the routing fails silently or with an opaque error.

The correct design:

1. At routing time, build a `DynamicEnumSchema` from `{agent_id_1, ..., agent_id_n}`.
2. Compile this into a grammar that allows only these string values.
3. Run the LLM constrained to this grammar.
4. Parse the response as a validated `AgentId`—guaranteed to be a member of the set.

This eliminates the hallucinated-agent-name failure class entirely, without requiring a new 
MCP tool or a new Rust type.

### 7.2  The `DynamicEnumSchema` Builder

```rust
/// A finite set constraint that can be compiled to JSON Schema and grammar.
pub struct DynamicEnumSchema {
    values: Vec<String>,
}

impl DynamicEnumSchema {
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self { ... }
}

impl OutputSchema for DynamicEnumSchema {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "string", "enum": self.values })
    }
    
    fn grammar_mode(&self) -> Option<GrammarMode> {
        // Compile a custom EBNF where start = "value_1" | "value_2" | ...
        Some(GrammarMode::DynamicEnum(self.clone()))
    }
}
```

This pattern generalises: any bounded response set (status codes, action verbs, plan steps) 
becomes a `DynamicEnumSchema`, removing the need to model it as a statically defined MCP 
tool contract.

### 7.3  Composite and Nested Schemas

For complex responses, compose schemas:

```rust
pub struct CompositeSchema {
    fields: Vec<(String, Arc<dyn OutputSchema>)>,
    required: Vec<String>,
}
```

This effectively mirrors `schemars::schema_for!()` but for runtime-constructed types, 
enabling entirely dynamic output specification without static Rust structs.

---

## 8. Cross-Cutting Improvements Required

### 8.1  Grammar Mode Registry (not a closed enum)

The current `GrammarMode` in `vox-constrained-gen/src/lib.rs` is a closed enum. Adding 
`DynamicEnum` requires modifying the library. A better design:

```rust
pub enum GrammarMode {
    None,
    Vox,
    VoxPda,
    Json,
    Custom(Arc<dyn ConstrainedSampler>),  // ← extensibility point
}
```

Or move to a factory registry pattern where modes are registered by name.

### 8.2  JSON Mode Should Use the Modern Stack

`GrammarMode::Json` currently delegates to `vox-populi`'s legacy `JsonGrammarAutomaton`. 
It should instead compile a JSON Schema into the Earley/PDA parser, achieving:
- Parity with the Vox-language constraint path
- Support for arbitrary JSON Schema constraints, not just flat JSON
- Elimination of the legacy automaton maintenance burden

### 8.3  Socrates Per-Inference, Not Just Per-Task

`evaluate_socrates_gate` should be callable per inference invocation, not just at 
task-completion time. The confidence signal from each `LlmMediator::call()` should 
accumulate into the task-level Socrates context.

Implementation sketch:

```rust
impl LlmMediator<T> {
    async fn call(...) -> Result<Mediated<T>, MediationError> {
        // ...run tiers...
        
        // Update task-level Socrates context with evidence from this call
        if let Some(ctx) = &self.task_socrates_ctx {
            ctx.evidence_count = ctx.evidence_count.saturating_add(1);
            if failed { ctx.contradiction_hints = ctx.contradiction_hints.saturating_add(1); }
        }
    }
}
```

### 8.4  Trust Recording Per Validation Failure Class

Extend `trust_observations` with a `validation_class` dimension:

| dimension | meaning |
|---|---|
| `schema_conformance` | Tier 1/2 structural failures: is output machine-parseable? |
| `semantic_policy` | Tier 2 business-rule failures |
| `repair_exhaustion` | Cases where the repair loop hit max_attempts |
| `factuality` | Existing |
| `latency_reliability` | Existing |

This gives operators visibility into *why* an agent/model is losing trust.

### 8.5  Capability Registry Integration

`vox-capability-registry` defines `CuratedCapability` with a `parameters` schema. Each 
capability should also carry an `output_schema` field that becomes the input to 
`LlmMediator::from_schema()`. This creates a closed loop:

```
CuratedCapability.output_schema 
  → LlmMediator<serde_json::Value>
  → validated output at invocation time
```

No additional MCP tool definition is needed; the capability registry *is* the schema source 
of truth.

archived_date: 2026-04-18
---

## 9. Reducing vs. Extending MCP Necessity

This question is nuanced. MCP is **necessary** for the external interface boundary: 
any agent (Cursor, Claude, other IDEs) that wants to invoke Vox tools must do so via MCP 
because that is the protocol they understand. MCP is **unnecessary** for internal 
orchestrator-to-agent communication, where the LML can operate without the overhead of 
JSON-RPC transport.

### Reducing MCP Necessity

The key insight is that **most MCP tools were created to give the LLM a bounded interface 
for a task that could be expressed as a typed schema**. Given: `LlmMediator<DynamicEnum>`, 
the following MCP tools become optional:

- `vox_task_classify` — replace with `LlmMediator<TaskCategory>`
- `vox_routing_select_agent` — replace with `LlmMediator<AgentId>`
- `vox_plan_step_kind` — replace with `LlmMediator<PlanStepKind>`
- Any tool whose sole purpose is to extract a categorical value from LLM text

MCP tools that **remain necessary**:
- Tools that invoke external side effects (file writes, git operations, web requests)
- Tools that surface Vox system state to external IDE clients
- Tools that need to be discoverable by external agents via MCP's tool-listing protocol

### Extending MCP Automatically

For tools that remain necessary, the capability registry + LML combination allows **auto-generation** 
of MCP tool definitions:

```rust
impl CuratedCapability {
    pub fn as_mcp_tool(&self) -> McpToolDefinition {
        McpToolDefinition {
            name: self.id.clone(),
            description: self.description.clone(),
            input_schema: self.parameters.clone(),
            output_schema: self.output_schema.clone(),  // ← new field
        }
    }
}
```

The `output_schema` field drives both the internal `LlmMediator` and the external MCP 
tool definition simultaneously, ensuring they remain in sync.

---

## 10. RLVR/GRPO Training Alignment

The mediation layer connects forward to the training pipeline. Each Tier 2 semantic 
validation failure is a **verifiable reward signal** suitable for RLVR:

- Structural pass (Tier 1) → reward 0.3 (necessary but not sufficient)
- Semantic validation pass (Tier 2) → reward 0.6
- Task success confirmed by downstream artifact check → reward 1.0

This mirrors the existing GRPO reward shaping research 
(`research-grpo-reward-shaping-2026.md`), which already uses compile-pass as a binary 
reward. The LML makes this reward signal *automatic* for every mediated call: validation 
pass/fail is already recorded, and it can be replayed as an RLVR training signal.

The MENS training pipeline should tag RLVR-eligible traces from mediated calls with a 
`lml_validated: true` annotation to distinguish them from raw unvalidated generations.

archived_date: 2026-04-18
---

## 11. Implementation Roadmap (Proposed Waves)

### Wave 0 — Foundation (Low Effort, High Impact)

- [ ] Extend `GrammarMode` with a `Custom(Arc<dyn ConstrainedSampler>)` variant.
- [ ] Migrate `GrammarMode::Json` to use Earley/PDA with compiled JSON schema grammar.
- [ ] Add `DynamicEnumSchema` builder in `vox-constrained-gen`.
- [ ] Add `SemanticValidator<T>` trait in a new `vox-mediation` crate (or `vox-orchestrator` module).

### Wave 1 — LlmMediator Core

- [ ] Implement `LlmMediator<T>` with three-tier pipeline.
- [ ] Implement repair loop with error-context injection.
- [ ] Wire Socrates per-inference confidence accumulation.
- [ ] Record validation failure class into trust layer.

### Wave 2 — Schema-First MCP Reduction

- [ ] Add `output_schema: Option<serde_json::Value>` to `CuratedCapability`.
- [ ] Generate `McpToolDefinition` from `CuratedCapability` automatically.
- [ ] Replace internal categorical MCP tools with typed `LlmMediator` calls.

### Wave 3 — Training Integration

- [ ] Tag RLVR-eligible traces from mediated calls.
- [ ] Expose `lml_validation_result` as a reward dimension in GRPO training runs.
- [ ] Build corpus-level analytics: schema_conformance rate, repair loop depth distribution.

---

## 12. Open Questions

1. **Latency budget for three-tier validation.** Tier 1 (constrained generation) reduces 
   generation failures but adds per-token overhead. For latency-sensitive paths (e.g., 
   interactive clarification), should the default be Tier 1-only with Tier 2 applied async?

2. **Dynamic grammar compilation cost.** Compiling a new grammar per request (e.g., 
   `DynamicEnumSchema` with 20 agent IDs) must be cheap. The current Earley backend builds 
   the chart incrementally, but the grammar object itself must be compiled from EBNF. Should 
   dynamic enum schemas bypass EBNF and construct the grammar IR directly?

3. **Semantic validator registry.** Should `SemanticValidator` impls be registered 
   per-type via a factory (like `ConstrainedSampler`), or instantiated inline at each call 
   site? The former is more discoverable; the latter is more ergonomic.

4. **MCP output schema standardisation.** MCP currently has no standard `outputSchema` 
   field on tool definitions (it is an extension). This means external agents cannot 
   introspect what a tool returns. Should Vox propose a MCP extension or use an 
   out-of-band mechanism?

5. **HITL escalation trigger definition.** Currently the HITL doubt loop is triggered 
   explicitly via `vox_doubt_task`. Should the LML auto-escalate to HITL when `repair_policy.
   max_attempts` is exhausted, or should that be a configurable decision per call site?

archived_date: 2026-04-18
---

## Works Cited and Evidence Quality

- "The Validation Sandwich" pattern: synthesised from Guardrails AI docs, Pydantic AI docs, 
  Instructor Python library docs, and 2025–2026 blog posts. **High confidence** — consistent 
  across multiple independent practitioners.
- XGrammar-2 / llguidance metrics: from `research-grammar-constrained-decoding-2026.md` 
  (compiled April 2026 from XGrammar-2 arXiv and MLSys 2026). **High confidence**.
- RLVR and GRPO: from `research-grpo-reward-shaping-2026.md` and supporting cluster. 
  **High confidence**.
- `rstructor` Rust crate (LLM typed extraction): crates.io listing, April 2026. 
  **Moderate confidence** — new crate, API stability unclear.
- Arazzo specification for workflow-level determinism: nordicapis.com, 2025. 
  **Low confidence** — adoption still early.
- TypeState pattern in Rust: well-established Rust community pattern, multiple blog posts 
  2023–2025. **High confidence**.
- MCP `outputSchema` extension: not yet in official spec as of April 2026. 
  **Low confidence** — speculative proposal.

---

_This research document should be cross-referenced when implementing
[`vox-mediation` crate design](../architecture/architecture-index.md) and when revising
[`capability-registry-ssot.md`](capability-registry-ssot.md)._


