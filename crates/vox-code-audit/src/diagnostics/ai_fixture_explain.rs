//! Extended `--explain` prose for AI-first fixture diagnostics (typecheck + TS codegen).

use super::catalog;

/// Return multi-line rationale for stable AI-fixture-related diagnostic IDs.
#[must_use]
pub fn explain_ai_fixture_diagnostic(id: &str) -> Option<&'static str> {
    Some(match id {
        x if x == catalog::AI_UNKNOWN_TASK_CATEGORY => {
            "`@ai(task_category = …)` must use a category present in `contracts/orchestration/model-routing.v1.yaml` \
(`task_categories`). Unknown labels are rejected so intent routing and telemetry stay aligned with the router SSOT.\n\
\n\
Bad:\n\
  @ai(task_category = FancyLabel)\n\
\n\
Good:\n\
  @ai(task_category = CodeGen)\n\
  @uses(net)\n"
        }
        x if x == catalog::PROMPT_INVALID_STAGE => {
            "`@prompt(stage = …)` must name a member of `ResearchStage` (`Planner`, `ClaimExtraction`, \
`Verification`, `Synthesis`, `Judge`, `SelfVerification`). Invalid stages previously fell back silently \
to `Planner`; typecheck now blocks them.\n\
\n\
Bad:\n\
  @prompt(stage = Draft, schema = Foo)\n\
\n\
Good:\n\
  @prompt(stage = Planner, schema = PlanBlob, redact = [api_key])\n"
        }
        x if x == catalog::PROMPT_SECRET_LEAKAGE => {
            "`@prompt(..., redact = […])` entries that look like secret/env variable names should pair with \
an explicit `uses env` clause (read bound) so reviewers see that secret-shaped tokens are intentional.\n\
\n\
Bad:\n\
  @prompt(stage = Planner, schema = X, redact = [OPENROUTER_API_KEY])\n\
\n\
Good:\n\
  @prompt(stage = Planner, schema = X, redact = [OPENROUTER_API_KEY])\n\
  @uses(net, env)\n"
        }
        x if x == catalog::SUBAGENT_CHAIN_DEPTH_EXCEEDED => {
            "`@subagent(max_depth = N)` seeds `DispatchSignal.chain_depth`. Values **≥** \
`DispatchConfig::default().max_chain_depth` (5) are rejected immediately by `DispatchRouter::route`, \
so the compiler flags them up front.\n\
\n\
Bad:\n\
  @subagent(policy = parallel, max_depth = 9)\n\
\n\
Good:\n\
  @subagent(policy = parallel, max_depth = 3)\n"
        }
        x if x == catalog::SUBAGENT_DISTRIBUTED_NOT_WIRED => {
            "`@subagent(policy = distributed)` routes across the mesh and requires the `populi-transport` \
feature on `vox-orchestrator` (plus generated bundle deps). Until workspace metadata confirms that wiring, \
emitting this fixture is warned so CI/docs stay honest.\n\
\n\
Fix:\n\
  Enable `populi-transport` on `vox-orchestrator` for bundles that use distributed policy, per packaging SSOT.\n"
        }
        x if x == catalog::SEARCH_CORPUS_DENIED => {
            "`@search(corpus = …)` only supports `memory`, `docs`, or `web`. Other corpus labels were placeholders.\n\
\n\
Good:\n\
  @search(corpus = docs, query = \"runtime policy\", into = str)\n"
        }
        x if x == catalog::SEARCH_MEMORY_KEY_INVALID => {
            "Memory recall keys must follow `scope:account:key` (three non-empty colon segments) so \
`MemoryManager::lookup_fact_by_key` receives a stable composite key.\n\
\n\
Bad:\n\
  @search(corpus = memory, query = \"onboarding\", into = str)\n\
\n\
Good:\n\
  @search(corpus = memory, query = \"project:default:onboarding\", into = str)\n"
        }
        x if x == catalog::SEARCH_WEB_POLICY_DENIED => {
            "Web search fixtures perform outbound retrieval through the LLM cascade and must declare network effects.\n\
\n\
Bad:\n\
  @search(corpus = web, query = \"wasm\", into = str)\n\
\n\
Good:\n\
  @search(corpus = web, query = \"wasm\", into = str)\n\
  @uses(net)\n"
        }
        x if x == catalog::CODEGEN_MISSING_TS_AI_LOWERING => {
            "The TypeScript emitter does not lower `@ai` / `@prompt` / `@subagent` / `@search` bodies yet; \
generated TS would omit those call paths. Builds continue with a warning by default; set `VOX_TS_STRICT_AI=1` \
(or pass strict codegen options) to fail fast.\n\
\n\
Track: `docs/src/architecture/ai-fixtures-ts-lowering-follow-on-2026.md`.\n"
        }
        _ => return None,
    })
}
