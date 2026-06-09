# Claude Code Feature Adoption — Implementation Roadmap

> **For agentic workers:** This is a *roadmap of plans* (multi-subsystem). Each Tier-1 item should be taken into its own brainstorm→spec→bite-sized plan via superpowers:writing-plans before implementation. The keystone (Item 1) is fully bite-sized below as the worked example and the recommended first build. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Adopt the load-bearing, *beloved* Claude Code patterns that Vox lacks — prioritizing self-validation, context discipline, and execution-based evaluation — across the harness, the MENS pipeline, Candle, and the Vox language.

**Architecture:** The research ([claude-code-beloved-features-research-2026-06-07.md](../../src/architecture/claude-code-beloved-features-research-2026-06-07.md)) found Vox already implements the *expensive* harness machinery (dispatch, routing, compaction, trust-scored permissions, checkpoint engine, MCP server). The gaps are a handful of *sharp primitives and surfaces*. We adopt the principles, not the UI: "verification the agent can run itself," progressive context disclosure, and real behavioral eval.

**Tech Stack:** Rust workspace (vox-orchestrator, vox-actor-runtime, vox-orchestrator-mcp, vox-eval, vox-inference, vox-plugin-mens-candle-cuda), Candle, `.vox` scripts for automation, MCP via `rmcp`.

---

## Prioritization summary (value ÷ effort)

| # | Item | Subsystem | Tier | Effort | Headline gain |
|---|---|---|---|---|---|
| 1 | **Verification-driven agent loop** | harness | 1 | M | The #1 beloved feature; closes the self-correct loop with real `vox check`/test output |
| 2 | **MCP tool search / progressive disclosure** | harness | 1 | M | Context-budget win as the 100+ tool surface grows |
| 3 | **Execution-based MENS eval harness** | MENS | 1 | M | "Verification is real" for our own model; keystone from golden-corpus initiative |
| 4 | **Project-scoped `VOX.md` memory** | harness | 1 | S | Always-injected project DNA (the highest-ROI CC lever per Anthropic's own teams) |
| 5 | **Gradient checkpointing (Candle trainer)** | Candle | 2 | L | Biggest single VRAM lever — bigger models / longer seq on 16 GB |
| 6 | **User-defined hooks (PreToolUse/PostToolUse)** | harness | 2 | M | Determinism wrapping probabilism; format/guard rules the model can't skip |
| 7 | **Prompt-cache structuring in llm egress** | harness | 2 | S–M | 90%-off cache reads make the loop economical |
| 8 | **Grammar-constrained decoding** | Candle/Vox | 2 | L | Structured Outputs for serving + a Vox `generate(schema:)` builtin |
| 9 | **Extended/interleaved thinking surface** | harness | 2 | M | Reasoning between tool calls; quality on multi-step debugging |
| 10 | **Worktree-isolated sub-agents + user rewind** | harness | 3 | L | Conflict-free parallelism + ambitious-edit safety net |
| 11 | **Federated LoRA-delta averaging** | MENS | 3 | L | The realistic LAN-training path (MB-scale sync) |
| 12 | **Flash-attention** | Candle | 3 | L | Activation/throughput win; pairs with #5 |

Recommended sequence: **1 → 4 → 2 → 3** (Tier 1), then **7 → 6 → 5 → 9 → 8** (Tier 2), then Tier 3 as capacity allows. Items 1, 4, 7 are mutually independent and could run in parallel worktrees.

---

## TIER 1

### Item 1 (KEYSTONE — fully bite-sized): Verification-driven agent loop

**Why:** The single most-loved Claude Code property is the loop that *runs its own validation and self-corrects* ([Building agents](https://claude.com/blog/building-agents-with-the-claude-agent-sdk); [Willison](https://simonwillison.net/2026/Feb/23/agentic-engineering-patterns/)). Vox has the loop (`vox-orchestrator` agent execution + `mutation_classifier` + `checkpoint_engine`) and a real validator (`vox-langtool` / `vox check` / `vox run`), but does not currently feed validator output back as a structured tool result that drives a bounded self-correct retry. This wires the highest-value behavior using primitives we already own.

**Architecture:** Add a `verification` module to `vox-orchestrator` that, after any turn whose mutations touched `.vox`/Rust files, runs a configured verifier, captures structured `VerificationOutcome`, and — on failure — re-injects the diagnostics as a tool result and grants the agent a bounded retry budget before escalating. Verifier choice is policy-driven (reuse `route_capability_policy` env conventions).

**Files:**
- Create: `crates/vox-orchestrator/src/verification/mod.rs`
- Create: `crates/vox-orchestrator/src/verification/outcome.rs`
- Create: `crates/vox-orchestrator/src/verification/runner.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs` (register `pub mod verification;`)
- Modify: agent turn-completion site in `crates/vox-orchestrator/src/orchestrator/agent/mod.rs` (call the verifier after mutating turns)
- Modify: `crates/vox-telemetry/src/types.rs` (add `METRIC_TYPE_VERIFICATION_RESULT`)
- Test: `crates/vox-orchestrator/src/verification/outcome.rs` (unit, inline `#[cfg(test)]`)
- Test: `crates/vox-orchestrator/tests/verification_loop.rs` (integration)

- [ ] **Step 1: Write the failing test for `VerificationOutcome` parsing**

```rust
// crates/vox-orchestrator/src/verification/outcome.rs  (#[cfg(test)] mod tests)
#[test]
fn parses_vox_check_failure_into_structured_diagnostics() {
    let raw = "error[E0425]: cannot find value `x`\n --> probe.vox:3:5\n";
    let outcome = VerificationOutcome::from_check_output(2, raw, "");
    assert!(!outcome.passed);
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(outcome.diagnostics[0].file.as_deref(), Some("probe.vox"));
    assert_eq!(outcome.diagnostics[0].line, Some(3));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-orchestrator verification::outcome -- --nocapture`
Expected: FAIL — `VerificationOutcome` not defined.

- [ ] **Step 3: Implement `VerificationOutcome`**

```rust
// crates/vox-orchestrator/src/verification/outcome.rs
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity { Error, Warning }

#[derive(Debug, Clone, PartialEq)]
pub struct VerificationOutcome {
    pub passed: bool,
    pub exit_code: i32,
    pub diagnostics: Vec<Diagnostic>,
    /// Compact, model-facing summary re-injected as a tool result.
    pub summary: String,
}

impl VerificationOutcome {
    pub fn from_check_output(exit_code: i32, stdout: &str, stderr: &str) -> Self {
        let mut diagnostics = Vec::new();
        let mut pending: Option<Diagnostic> = None;
        for line in stdout.lines().chain(stderr.lines()) {
            let l = line.trim_start();
            if l.starts_with("error") {
                if let Some(d) = pending.take() { diagnostics.push(d); }
                pending = Some(Diagnostic {
                    severity: Severity::Error,
                    message: l.to_string(),
                    file: None, line: None,
                });
            } else if l.starts_with("--> ") {
                if let Some(d) = pending.as_mut() {
                    let loc = l.trim_start_matches("--> ");
                    let mut parts = loc.split(':');
                    d.file = parts.next().map(|s| s.trim().to_string());
                    d.line = parts.next().and_then(|s| s.trim().parse().ok());
                }
            }
        }
        if let Some(d) = pending.take() { diagnostics.push(d); }
        let passed = exit_code == 0;
        let summary = if passed {
            "verification passed".to_string()
        } else {
            format!("verification failed ({} error(s)); first: {}",
                diagnostics.len(),
                diagnostics.first().map(|d| d.message.as_str()).unwrap_or("unknown"))
        };
        Self { passed, exit_code, diagnostics, summary }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-orchestrator verification::outcome`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/verification/outcome.rs
git commit -m "feat(orchestrator): VerificationOutcome structured diagnostics parser"
```

- [ ] **Step 6: Write the failing test for the runner (verifier abstraction)**

```rust
// crates/vox-orchestrator/tests/verification_loop.rs
use vox_orchestrator::verification::{VerificationRunner, VerifierKind};

#[test]
fn runner_reports_failure_for_nonzero_exit() {
    // A fake verifier that always returns exit 2 with one error line.
    let runner = VerificationRunner::fake(2, "error[E0425]: boom\n --> a.vox:1:1\n");
    let outcome = runner.run_blocking(&["a.vox"]);
    assert!(!outcome.passed);
    assert_eq!(outcome.diagnostics.len(), 1);
}
```

- [ ] **Step 7: Run it to verify it fails**

Run: `cargo test -p vox-orchestrator --test verification_loop`
Expected: FAIL — `VerificationRunner` not defined.

- [ ] **Step 8: Implement `VerificationRunner`**

```rust
// crates/vox-orchestrator/src/verification/runner.rs
use super::outcome::VerificationOutcome;

#[derive(Debug, Clone, Copy)]
pub enum VerifierKind { VoxCheck, VoxRun, None }

pub struct VerificationRunner {
    kind: VerifierKind,
    #[cfg(any(test, feature = "test-util"))]
    fake: Option<(i32, String)>,
}

impl VerificationRunner {
    pub fn new(kind: VerifierKind) -> Self {
        Self { kind, #[cfg(any(test, feature = "test-util"))] fake: None }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn fake(exit: i32, stdout: &str) -> Self {
        Self { kind: VerifierKind::VoxCheck, fake: Some((exit, stdout.to_string())) }
    }

    /// Runs the configured verifier against the given files. On Windows,
    /// the spawned child MUST set CREATE_NO_WINDOW (see feedback_no_console_windows_on_spawn).
    pub fn run_blocking(&self, files: &[&str]) -> VerificationOutcome {
        #[cfg(any(test, feature = "test-util"))]
        if let Some((code, out)) = &self.fake {
            return VerificationOutcome::from_check_output(*code, out, "");
        }
        match self.kind {
            VerifierKind::None => VerificationOutcome::from_check_output(0, "", ""),
            VerifierKind::VoxCheck | VerifierKind::VoxRun => {
                let sub = match self.kind { VerifierKind::VoxRun => "run", _ => "check" };
                let mut cmd = quiet_command("vox");
                cmd.arg(sub).args(files);
                let out = cmd.output().expect("spawn vox verifier");
                VerificationOutcome::from_check_output(
                    out.status.code().unwrap_or(-1),
                    &String::from_utf8_lossy(&out.stdout),
                    &String::from_utf8_lossy(&out.stderr),
                )
            }
        }
    }
}
```

(Reuse the existing `quiet_command` helper from the codebase — see `docs/src/ci/runner-contract.md` §Windows subprocess console suppression. If it is not yet shared, import it from where the autoscaler defines it; do not introduce a second copy.)

- [ ] **Step 9: Wire `mod.rs` and register the module**

```rust
// crates/vox-orchestrator/src/verification/mod.rs
mod outcome;
mod runner;
pub use outcome::{Diagnostic, Severity, VerificationOutcome};
pub use runner::{VerificationRunner, VerifierKind};

/// Bounded self-correct policy. After a failed verification, the agent gets
/// `max_retries` additional turns with diagnostics injected before escalation.
#[derive(Debug, Clone, Copy)]
pub struct SelfCorrectPolicy { pub max_retries: u8 }
impl Default for SelfCorrectPolicy { fn default() -> Self { Self { max_retries: 2 } } }
```

```rust
// crates/vox-orchestrator/src/lib.rs  (add near other `pub mod` lines)
pub mod verification;
```

- [ ] **Step 10: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator --test verification_loop && cargo test -p vox-orchestrator verification`
Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add crates/vox-orchestrator/src/verification/ crates/vox-orchestrator/src/lib.rs crates/vox-orchestrator/tests/verification_loop.rs
git commit -m "feat(orchestrator): VerificationRunner with bounded self-correct policy"
```

- [ ] **Step 12: Add the telemetry metric type**

```rust
// crates/vox-telemetry/src/types.rs  (add alongside METRIC_TYPE_* constants)
/// Emitted after a post-mutation verification pass (D-series, verification lane).
pub const METRIC_TYPE_VERIFICATION_RESULT: &str = "verification_result";
```

- [ ] **Step 13: Integrate at the turn-completion site (write the integration test first)**

```rust
// crates/vox-orchestrator/tests/verification_loop.rs  (append)
#[test]
fn mutating_turn_triggers_verification_and_injects_failure_summary() {
    // Use the orchestrator's existing test harness to run one mutating turn
    // against a fixture that fails `vox check`, then assert the next turn's
    // context contains the verification summary and retry_count == 1.
    let h = vox_orchestrator::test_support::TurnHarness::new()
        .with_verifier(VerificationRunner::fake(2, "error[E0001]: bad\n --> f.vox:2:1\n"));
    let next = h.run_mutating_turn_touching(&["f.vox"]);
    assert!(next.injected_context.contains("verification failed"));
    assert_eq!(next.retry_count, 1);
}
```

> If `test_support::TurnHarness` does not exist, that is itself a task: add a minimal harness exposing `with_verifier`, `run_mutating_turn_touching`, returning `{ injected_context: String, retry_count: u8 }`. Keep it behind `#[cfg(any(test, feature = "test-util"))]`.

- [ ] **Step 14: Run it to verify it fails, implement the integration, re-run to green, commit**

Run: `cargo test -p vox-orchestrator --test verification_loop`
Expected: FAIL → implement the call in `orchestrator/agent/mod.rs` turn completion (only when `mutation_classifier` reports a `local_mutation`/`external_side_effect` touching verifiable files; emit `METRIC_TYPE_VERIFICATION_RESULT`; on failure inject `outcome.summary` and increment `retry_count` up to `SelfCorrectPolicy::max_retries`) → PASS.

```bash
git add -A && git commit -m "feat(orchestrator): wire verification self-correct loop into agent turns"
```

- [ ] **Step 15: Format + workspace checks**

Run: `cargo fmt -p vox-orchestrator -p vox-telemetry` then `cargo run -p vox-arch-check` then `VOX_FMT_CHECK=1 vox run scripts/fmt.vox`
Expected: clean. (Never `cargo fmt --all` on Windows — see `AGENTS.md` §Formatting Rust (Windows-safe).)

**Verification of the whole item:** an agent turn that writes a syntactically-broken `.vox` file is followed automatically by a `vox check`, the diagnostics appear in the next turn's context, and the agent gets up to 2 self-correct retries before escalation; `METRIC_TYPE_VERIFICATION_RESULT` is emitted each pass.

---

### Item 2: MCP tool search / progressive tool disclosure

**Why:** Claude Code's tool search "keeps MCP context usage low by deferring tool definitions until needed — only tool names and server instructions load at session start" ([mcp docs](https://code.claude.com/docs/en/mcp)). Vox's `vox-orchestrator-mcp` exposes all 100+ tools eagerly; as the surface grows this taxes every prompt. (This is the exact pattern this session uses via `ToolSearch`.)

**What's involved:**
- In `vox-orchestrator-mcp`, split the tool registry into (a) a lightweight always-present `{name, one-line description}` index plus a single `tool_search` meta-tool, and (b) full JSON schemas fetched on demand.
- A resolver that, on `tool_search(query)`, returns matching full schemas and marks them "active" for the session (`mcp_context.rs` session state).
- Keep eager mode behind a flag for back-compat / tests.
- Files: `crates/vox-orchestrator-mcp/src/tool_index.rs` (new), modify the server registration in `vox-orchestrator-mcp/src/*` and `mcp_context.rs`; reuse the existing `vox-mcp-registry` YAML SSOT to generate the index so names/descriptions stay DRY.

**Effort:** M. **Risk:** medium — must not regress existing tool-call paths; gate behind a `VOX_MCP_TOOL_SEARCH` profile and default-off until parity is proven against the current tool-call goldens.

**Verification:** with tool search on, session-start tool payload drops to names+descriptions; a `tool_search("git commit")` call returns the commit tool's full schema and a subsequent call to it succeeds; existing MCP integration tests pass in both modes.

---

### Item 3: Execution-based MENS eval harness

**Why:** "The loop only works if verification is real." Our MENS eval (`vox-eval`) is heuristic (format/safety/length) and `vox audit humaneval` is **static typecheck only** — neither *runs* the generated program. This is also the keystone identified in the golden-corpus & compiler-reality initiative (no behavioral-output harness exists; 48/62 goldens parse-only). Behavioral eval is what lets us tell a *good* fine-tune from a plausible-looking one.

**What's involved:**
- A new execution scorer in `vox-eval`: given a model completion for a HumanEval-Vox problem, write it to a temp `.vox`, run it via `vox run` against the problem's test asserts, score pass/fail (not just typecheck).
- Extend `vox audit humaneval` with an `--execute` mode that uses the scorer; keep the static gate as a fast pre-filter.
- A small results schema (`problem_id`, `compiled`, `ran`, `passed`, `stderr_excerpt`) and an aggregate pass@1.
- Reuse the MENS serving path (`vox mens serve` / `vox-inference`) to generate completions; sandbox each run with the existing activity-timeout mechanism (`METRIC_TYPE_SANDBOX_TIMEOUT_KILL`).
- Files: `crates/vox-eval/src/execution.rs` (new), modify `crates/vox-audit/src/subcommands/humaneval.rs`, add a `mens/config/eval-gates-exec.yaml`.

**Effort:** M. **Risk:** medium — needs a real Vox stdout/return harness (the missing piece flagged in the golden-corpus plan); coordinate so we build it once. **Dependency:** a behavioral output harness for `.vox` (may be a prerequisite sub-task).

**Verification:** `vox audit humaneval --execute` reports an executed pass@1 distinct from the static pass rate on a known model; a deliberately wrong completion scores `passed=false` with a captured stderr excerpt.

---

### Item 4: Project-scoped `VOX.md` memory

**Why:** Anthropic's own teams report CLAUDE.md is the single biggest lever on output quality ([Anthropic teams](https://claude.com/blog/how-anthropic-teams-use-claude-code)). Vox has global/daily memory (`MemoryManager`, MEMORY.md) but **no per-workspace always-injected project file**. Cheapest high-ROI item.

**What's involved:**
- On orchestrator session start in a workspace, discover `VOX.md` (and `.vox/VOX.md`), parse `@path` imports recursively to a max depth (cap at 5, with a first-import approval prompt, mirroring CC's security gate), and inject as a high-precedence context block.
- Precedence tier slotted into the existing `memory/` hierarchy (above user memory, below managed policy).
- Files: `crates/vox-orchestrator/src/memory/project_file.rs` (new), modify `crates/vox-orchestrator/src/memory/mod.rs`; document the convention in `docs/src/architecture/where-things-live.md` (add the row) and `AGENTS.md`.

**Effort:** S. **Risk:** low. **Note:** keep it distinct from the existing global MEMORY.md; this is workspace DNA, checked into the repo.

**Verification:** a workspace with a `VOX.md` containing a build-command note and an `@docs/conventions.md` import has both contents present in the agent's initial context; depth >5 or a missing import degrades gracefully with a logged warning.

---

## TIER 2

### Item 5: Gradient checkpointing in the Candle QLoRA trainer

**Why:** Absent today; it is the single biggest VRAM lever. Activations dominate the budget (`ACT_GIB_PER_KTOK_PER_SQRTB = 9.5` in `memory_budget.rs`); recomputation trades compute for memory, enabling larger models / longer sequences on the 16 GB 4080 (directly addresses the 4B-OOM finding).

**What's involved:**
- In `vox-plugin-mens-candle-cuda` model forward (`model.rs` / `training_loop`), wrap transformer blocks so activations are dropped on the forward pass and recomputed during backward. Candle does not ship a turnkey `checkpoint()`; implement by re-running block forwards inside the backward closure for checkpointed segments (segment every N layers, N configurable).
- Add `gradient_checkpointing: bool` + `gc_segment: usize` to `training_config.rs`; recalibrate `memory_budget.rs` activation coefficient when GC is on (lower effective activation VRAM → allow larger model/seq on the auto-retreat ladder).
- Files: modify `crates/vox-plugin-mens-candle-cuda/src/model.rs`, `.../training_loop/mod.rs`, `crates/vox-populi/src/mens/tensor/{training_config.rs,memory_budget.rs}`.

**Effort:** L. **Risk:** high — correctness of recompute vs. autograd graph; verify loss curves match a non-GC baseline within tolerance on a tiny preset before trusting it. Pairs naturally with flash-attention (Item 12).

**Verification:** training the `tiny` preset with GC on produces a loss curve within tolerance of GC-off; `memory_budget` admits a larger variant/seq under GC; measured peak VRAM drops materially on a `safe`/`4080` run.

### Item 6: User-defined hooks (PreToolUse/PostToolUse lifecycle)
**Why:** "Determinism wrapping probabilism" — guaranteed format/guard actions the model can't skip ([hooks](https://code.claude.com/docs/en/hooks)). Vox has *internal* guardrails (`guardrail_kernel`, `mutation_classifier`) but no *user-defined* hook surface. **What's involved:** a hook config (in `~/.vox/config.toml` and/or `.vox/hooks.toml`), lifecycle events (`PreToolUse` can allow/deny/modify, `PostToolUse`, `SessionStart/End`, `PreCompact`), and a `.vox`-script handler runner (honor the VoxScript-first rule — handlers are `.vox`, not shell). Exit-code-2-style blocking semantics fed back to the agent. Files: `crates/vox-orchestrator/src/hooks/` (new), wired at the tool-dispatch site. **Effort:** M. **Risk:** medium (must not deadlock the loop; bounded handler timeouts via activity runtime).

### Item 7: Prompt-cache structuring in `vox-actor-runtime` llm egress
**Why:** Cache reads at 0.1× (90% off) are what make a long loop economical ([prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)). **What's involved:** make `llm/mod.rs` emit provider cache-control breakpoints structured on the stable `tools → system → messages` prefix, with the conversation breakpoint auto-advancing; expose cache-hit telemetry (we already have `METRIC_TYPE_CACHE_HIT_PREDICTION`). Provider-specific (Anthropic-style `cache_control`); no-op for providers without caching. Files: `crates/vox-actor-runtime/src/llm/{mod,types,cascade}.rs`, provider clients in `vox-orchestrator-mcp/src/llm_bridge/providers/`. **Effort:** S–M. **Risk:** low–medium (don't put breakpoints on per-request content — that yields zero hits).

### Item 8: Grammar-constrained decoding (Structured Outputs) + Vox `generate(schema:)`
**Why:** Structured Outputs makes schema-violating tokens *impossible* ([docs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)); invaluable for tool-calling reliability from our own served model and for a first-class Vox generation primitive. **What's involved:** a logits-mask/grammar layer in `vox-inference` generate path (compile a JSON-schema/EBNF to a token-level mask), then surface a `generate(schema:)` effect in the Vox language (effect-governed, `GpuCompute`). Files: `crates/vox-inference/src/{generate.rs, grammar.rs(new)}`; language surface via the effect system. **Effort:** L. **Risk:** high (tokenizer-aware grammar masking is fiddly). Start with JSON-only.

### Item 9: Extended/interleaved thinking surface
**Why:** Reasoning *between* tool calls improves multi-step debugging ([extended thinking](https://docs.claude.com/en/docs/build-with-claude/extended-thinking)). Absent in Vox. **What's involved:** a `thinking_budget` field on `LlmConfig`, request wiring for providers that support it, and a planning-mode toggle (`PlanModeTrigger` already exists to gate when it's worth it). Surface thinking traces in `vox-gui` reasoning panel. Files: `crates/vox-actor-runtime/src/llm/types.rs`, `vox-orchestrator/src/planning/`, `vox-gui` surface. **Effort:** M. **Risk:** low–medium (cache invalidation interplay with Item 7 — document it).

## TIER 3

### Item 10: Worktree-isolated parallel sub-agents + user-facing rewind
**Why:** Conflict-free parallelism on disjoint files; rewind as an ambitious-edit safety net ([sub-agents](https://code.claude.com/docs/en/sub-agents), [checkpointing](https://code.claude.com/docs/en/checkpointing)). Vox has dispatch + a checkpoint *engine* but no worktree isolation and no user-facing rewind. **What's involved:** integrate `vox-vcs`/jj worktrees into `spawn_agent` for parallel mutating agents; a `vox agent rewind` CLI + GUI control over the existing `checkpoint_engine`/oplog. **Note the CC limitation to replicate-or-improve:** their checkpoints don't track bash-driven file changes — our jj-backed VCS can do better. **Effort:** L. **Risk:** medium-high (ties into the jj-first-class-VCS initiative — sequence after P1).

### Item 11: Federated LoRA-delta averaging (LAN training)
**Why:** The realistic LAN-training path is MB-scale LoRA-delta sync, not FSDP/TP (per the MENS pipeline audit). `vox-distributed-training` is contracts-only (`all_reduce` unsupported for world_size>1). **What's involved:** implement periodic LoRA-adapter delta averaging across nodes over the existing mesh transport (`vox-populi`), signed `GradientShard`/`CheckpointBundle` already defined. Throughput stays single-device; the win is data parallelism / ensemble. **Effort:** L. **Risk:** medium (convergence semantics; start with synchronous round-based averaging).

### Item 12: Flash-attention in the Candle trainer
**Why:** Activation/throughput win; pairs with Item 5. Absent. **What's involved:** adopt a Candle flash-attention kernel for the Qwen attention path under the `cuda` feature; recalibrate `memory_budget`. **Effort:** L. **Risk:** high (kernel availability/correctness across Candle versions; the candle 0.10 bump did not retire the kernels patch).

---

## Self-review notes

- **Spec coverage:** every beloved feature from the research §3 maps to an item (agent loop→1, plan mode→already have + Item 9 thinking, CLAUDE.md→4, headless→already have, sub-agents→already have + Item 10 worktrees, large-context→compaction have + Item 2/7 discipline, skills/plugins→have + Item 2 disclosure, hooks→6, MCP→have + Item 2, permission modes→already have/superior, checkpoints→have + Item 10 rewind, cost→have, prompt caching→7, structured outputs→8, thinking→9). MENS/Candle gaps (GC→5, exec-eval→3, federated→11, flash-attn→12, constrained decode→8). Vox language (generate(schema:)→8).
- **Placeholder scan:** Item 1 is fully bite-sized with real code; Items 2–12 are at work-item granularity *by design* (each needs its own brainstorm→spec→plan before implementation) and state files/approach/effort/risk/verification concretely rather than "TBD."
- **Type consistency:** `VerificationOutcome`, `Diagnostic`, `Severity`, `VerificationRunner`, `VerifierKind`, `SelfCorrectPolicy` are used consistently across Item 1 steps.
- **Open dependency flagged:** Item 3 depends on a `.vox` behavioral-output harness (shared with the golden-corpus initiative) — build once.

## Execution handoff

This roadmap intentionally stops at the decision point. **Recommended next step:** pick the keystone (**Item 1**) or another Tier-1 item, and I'll take it through superpowers:brainstorming → writing-plans to produce a full bite-sized plan, then execute it in an isolated worktree. Items 1, 4, and 7 are independent and could be built in parallel.
