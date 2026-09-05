//! Stable diagnostic ID catalog for Vox lints, type rules, and runtime traps.
//!
//! IDs follow the `vox/<category>/<kebab-name>` scheme and are **append-only**.
//! To rename a diagnostic, add the new ID constant and deprecate the old one
//! with a `_DEPRECATED` suffix; do not delete until two minor versions have passed.
//!
//! Every ID here has a corresponding `--explain` page planned at
//! `docs/src/reference/diagnostics/<category>-<name>.md` and a stable URL at
//! `https://voxlang.org/diag/<id>`.

// ---------------------------------------------------------------------------
// Security — immediate `Error` severity
// ---------------------------------------------------------------------------

/// Direct HTTP call to a known LLM provider hostname, bypassing `populi.*`.
/// Phase 2 / audit item A.5.
pub const LLM_DIRECT_PROVIDER_CALL: &str = "vox/llm/direct-provider-call";

/// `env.get(...)` call with a secret-shaped argument name (KEY, SECRET, TOKEN, …).
/// Phase 2 / audit item A.6.
pub const SECRET_ENV_GET_SHAPE: &str = "vox/secret/env-get-shape";

/// Import or dependency referencing a banned cryptography crate.
/// Phase 2 / audit item A.30.
pub const CRYPTO_BANNED_CRATE_IMPORT: &str = "vox/crypto/banned-crate-import";

/// iroh pattern that defeats the mesh transport security model (ADR-046):
/// `presets::N0`, `N0DisableRelay`, or `into_0rtt`.
pub const MESH_TRANSPORT_UNSAFE_IROH: &str = "vox/mesh/unsafe-iroh-pattern";

/// `@secret`-tagged struct field appearing as a `tracing` span attribute or log argument.
/// Phase 2 (descoped runtime-only from A.11 full taint).
pub const SECRET_LEAKED_TO_SPAN: &str = "vox/secret/leaked-to-span";

// ---------------------------------------------------------------------------
// Style / design — `Warning` by default
// ---------------------------------------------------------------------------

/// Identifier contains a repeated prefix segment, e.g. `user_user_id`.
/// Phase 2 / audit item A.16.
pub const STYLE_DUPLICATE_PREFIX_NAME: &str = "vox/style/duplicate-prefix-name";

/// Variable definition and its last use are > 80 lines apart (long-range coupling).
/// Phase 2 / audit item A.17.
pub const STYLE_LONG_RANGE_COUPLING: &str = "vox/style/long-range-coupling";

// ---------------------------------------------------------------------------
// Control flow — `Note` at land, `Warning` after burn-down
// ---------------------------------------------------------------------------

/// `match` over a `Result` or `Option` that can be replaced with `?` or a combinator.
/// Phase 2 / audit item A.19.
pub const CONTROL_FLOW_QUESTION_MARK_OPPORTUNITY: &str =
    "vox/control-flow/question-mark-opportunity";

/// `match opt { Some(x) => f(x), None => default }` can become `opt.map(f).unwrap_or(default)`.
/// Phase 2 / audit item A.20.
pub const CONTROL_FLOW_OPTION_COMBINATOR_OPPORTUNITY: &str =
    "vox/control-flow/option-combinator-opportunity";

// ---------------------------------------------------------------------------
// Documentation — `Note` in non-critical crates; `Warning` in runtime/compiler
// ---------------------------------------------------------------------------

/// Public function / type in a critical crate lacks an `ADR-NNN` or `TASK-N.M` citation.
/// Phase 2 / audit item A.22.
pub const DOC_MISSING_ADR_CITATION: &str = "vox/doc/missing-adr-citation";

// ---------------------------------------------------------------------------
// Decorator / syntax — `Warning` at land, `Error` after one minor
// ---------------------------------------------------------------------------

/// Decorator used in bare-keyword position, or bare keyword where a decorator is required.
/// Phase 2 / audit item A.27.
pub const DECORATOR_POSITION_MISMATCH: &str = "vox/decorator/position-mismatch";

/// `@require(complex_expr)` without adequate trailing justification prose (≥ 40 chars).
/// Phase 2 / audit item A.28.
pub const REQUIRE_JUSTIFICATION_PROSE_REQUIRED: &str = "vox/require/justification-prose-required";

// ---------------------------------------------------------------------------
// Handler / actor safety — `Warning` at land, `Error` after two minors
// ---------------------------------------------------------------------------

/// Known-panicking builtin (`unwrap`, `expect`, `panic`, `unreachable`, `todo`) called
/// inside an actor message handler or workflow activity body.
/// Phase 2 / descoped from A.4.
pub const HANDLER_PANICKING_BUILTIN: &str = "vox/handler/panicking-builtin";

// ---------------------------------------------------------------------------
// State machine (best-effort, not proof-level)
// ---------------------------------------------------------------------------

/// State in a `state_machine { }` block has no outgoing transitions, or a transition
/// targets a state that is not declared. Warning-only (proof-tier deferred).
/// Phase 2 / descoped from A.54.
pub const STATE_MACHINE_UNREACHABLE_STATE: &str = "vox/state-machine/unreachable-state";

// ---------------------------------------------------------------------------
// Auth / endpoint safety
// ---------------------------------------------------------------------------

/// `@endpoint`-decorated function has neither `@auth(...)` nor `@public` decorator.
/// Phase 2 / descoped from A.58.
pub const AUTH_ENDPOINT_MISSING_DECORATOR: &str = "vox/auth/endpoint-missing-decorator";

// ---------------------------------------------------------------------------
// Runtime (Phase 4)
// ---------------------------------------------------------------------------

/// Per-call fuel budget exhausted during interpreter execution.
pub const RUNTIME_FUEL_EXHAUSTED: &str = "vox/runtime/fuel-exhausted";

/// Allocation cap exceeded during interpreter execution.
pub const RUNTIME_ALLOC_CAP_EXCEEDED: &str = "vox/runtime/alloc-cap-exceeded";

/// Stack depth cap exceeded (runaway recursion).
pub const RUNTIME_STACK_OVERFLOW: &str = "vox/runtime/stack-overflow";

/// Interpreter host panic caught and converted to a structured diagnostic.
pub const RUNTIME_HOST_PANIC: &str = "vox/runtime/host-panic";

/// Builtin called outside its declared effect set at runtime.
pub const RUNTIME_CAPABILITY_VIOLATION: &str = "vox/runtime/capability-violation";

// ---------------------------------------------------------------------------
// Effect system (Phase 5)
// ---------------------------------------------------------------------------

/// Public fn transitively calls a `net`-effect builtin but lacks `@uses(net)`.
pub const EFFECT_MISSING_NET_DECL: &str = "vox/effect/missing-net-decl";

/// Public fn declares `@uses(net)` but makes no transitive `net`-effect call.
pub const EFFECT_UNJUSTIFIED_NET_DECL: &str = "vox/effect/unjustified-net-decl";

/// Non-deterministic builtin (`time`, `random`, `net`) called inside a `workflow` body.
pub const WORKFLOW_NON_DETERMINISTIC_BUILTIN: &str = "vox/workflow/non-deterministic-builtin";

/// `@pure fn` transitively calls an impure builtin or callee.
pub const EFFECT_PURE_VIOLATED: &str = "vox/effect/pure-violated";

// ---------------------------------------------------------------------------
// Core type errors — emitted by vox-compiler typeck (v0.6 LLM-target)
// ---------------------------------------------------------------------------
// These constants mirror `vox_compiler::typeck::diagnostics::codes::TYPES_*`
// so that `--explain` and `--list-diagnostics` can surface them.
// String values MUST stay in sync with the compiler crate.

/// Type mismatch in `let`, `return`, `state`, `derived`, or assignment.
pub const TYPES_TYPE_MISMATCH: &str = "vox/types/type-mismatch";
/// Undefined variable referenced in expression position.
pub const TYPES_UNDEFINED_VARIABLE: &str = "vox/types/undefined-variable";
/// Wrong number of arguments at a call site.
pub const TYPES_ARG_COUNT_MISMATCH: &str = "vox/types/arg-count-mismatch";
/// Argument type does not match the callee's declared parameter type.
pub const TYPES_ARG_TYPE_MISMATCH: &str = "vox/types/arg-type-mismatch";
/// Field access on a type that does not have that field.
pub const TYPES_FIELD_NOT_FOUND: &str = "vox/types/field-not-found";
/// Method call on a type that does not have that method.
pub const TYPES_METHOD_NOT_FOUND: &str = "vox/types/method-not-found";
/// Type of an expression could not be fully resolved (unresolved type variable).
pub const TYPES_UNRESOLVED_TYPE: &str = "vox/types/unresolved-type";

// ---------------------------------------------------------------------------
// Type rules (Phase 3)
// ---------------------------------------------------------------------------

/// `str`-typed ID parameter at an API boundary (`@endpoint`, `@table`, `@activity`, actor message).
pub const TYPES_ID_REQUIRED_AT_BOUNDARY: &str = "vox/types/id-required-at-boundary";

/// `Result[T, str]` or anonymous error type on a public boundary.
pub const TYPES_ANONYMOUS_ERROR_TYPE: &str = "vox/types/anonymous-error-type";

/// `.vox` file `syntax_version` does not match the workspace version in `Vox.toml`.
pub const SYNTAX_VERSION_MISMATCH: &str = "vox/syntax/version-mismatch";

/// Call to a `@deprecated` symbol; severity escalates with version distance.
pub const API_DEPRECATED_CALLSITE: &str = "vox/api/deprecated-callsite";

/// `training_eligible: true` file imports (transitively) a `training_eligible: false` file.
pub const CORPUS_TRAINING_INELIGIBLE_IMPORT: &str = "vox/corpus/training-ineligible-import";

/// Decorator name in bare-keyword position or vice-versa (parser-policy enforcement).
pub const SYNTAX_DECORATOR_POSITION_POLICY: &str = "vox/syntax/decorator-position-policy";

// ---------------------------------------------------------------------------
// Retirement guard (CR-L6) — `Warning` at land, `Error` after one minor
// ---------------------------------------------------------------------------

/// Source file uses a retired decorator or import form per AGENTS.md §Retired Surfaces.
/// First 5 patterns covered: `@component fn`, `@server fn`, `@query fn`,
/// `@mutation fn`, `@py.import`. Remaining 5 (`recall()`, `@capacitor/*`,
/// `axum::serve`, `rust-embed`, `vox-sherpa-transcribe`) land in P1.4.
pub const RETIRED_DECORATOR_USAGE: &str = "vox/retired/decorator-usage";

/// Source file references a retired Vox crate name (vox-ludus, vox-sherpa-transcribe).
/// Covers the `vox-ludus-crate` and `vox-sherpa-transcribe-plugin` rows of
/// `contracts/retirement/retired-surfaces.v1.yaml`. Phase 1.4 detector.
pub const RETIRED_CRATE_IMPORT: &str = "vox/retired/crate-import";

/// Source file references a retired Vox env-var name (TURSO_URL / VOX_TURSO_URL /
/// VOX_TURSO_TOKEN). Covers the `turso-env-vars` row. Phase 1.4 detector.
pub const RETIRED_ENV_VAR: &str = "vox/retired/env-var";

/// Source file calls a retired memory-API (`recall()` / `recall_async()`).
/// Covers the `recall-fn-api` row. Phase 1.4 detector.
pub const RETIRED_MEMORY_API: &str = "vox/retired/memory-api";

/// Source file imports a retired `@capacitor/*` package or runs `npx cap sync`.
/// Covers the `capacitor-imports` and `cap-sync-cli` rows. Phase 1.4 detector.
pub const RETIRED_CAPACITOR: &str = "vox/retired/capacitor";

/// A Vox file contains a circular `import` dependency (self-import or multi-file cycle).
/// Phase J.19 / CR-L gate.  Multi-file cycles are detected by
/// `detectors::import_cycles::detect_import_cycles_in_batch`.
pub const IMPORT_CYCLE: &str = "vox/import/cycle";

/// A function body is byte-identical across two or more (non-platform-sibling) crates —
/// a split-brain risk where the copies drift apart silently. Detected by
/// `detectors::cross_crate_dup::detect_cross_crate_dup_in_batch`. Info severity.
pub const CROSS_CRATE_DUP: &str = "vox/cross-crate/duplicate-logic";

/// A `match` wildcard arm (`_ =>`) returns a neutral/empty value while other arms do real
/// work — unmatched cases are silently swallowed (the value vanishes). Detected by
/// `detectors::catch_all_swallow`. Info severity.
pub const CATCH_ALL_SWALLOW: &str = "vox/catch-all-swallow";

// ---------------------------------------------------------------------------
// Codegen (Phase 1)
// ---------------------------------------------------------------------------

/// A `@generated-hash` header in a generated file does not match the file's actual content.
pub const CODEGEN_GENERATED_FILE_DRIFT: &str = "vox/codegen/generated-file-drift";

/// A decorator-shaped public Rust fn under `vox-compiler/src/lower/decorators/`
/// lacks a `#[vox_decorator]` attribute.
pub const CODEGEN_DECORATOR_WITHOUT_ATTRIBUTE: &str = "vox/codegen/decorator-without-attribute";

/// `@ai(task_category=...)` payload contains an unrecognized category.
pub const AI_UNKNOWN_TASK_CATEGORY: &str = "vox/ai/unknown-task-category";
/// `@prompt(stage=...)` stage value is invalid.
pub const PROMPT_INVALID_STAGE: &str = "vox/prompt/invalid-stage";
/// `@prompt(...)` omitted required redaction policy for sensitive payloads.
pub const PROMPT_SECRET_LEAKAGE: &str = "vox/prompt/secret-leakage";
/// `@subagent(...)` requested chain depth exceeds configured limits.
pub const SUBAGENT_CHAIN_DEPTH_EXCEEDED: &str = "vox/subagent/chain-depth-exceeded";
/// `@subagent(policy = distributed)` requires mesh / `populi-transport` workspace wiring.
pub const SUBAGENT_DISTRIBUTED_NOT_WIRED: &str = "vox/subagent/distributed-not-wired";
/// `@search(corpus=...)` attempted a denied or unsupported corpus.
pub const SEARCH_CORPUS_DENIED: &str = "vox/search/corpus-denied";
/// `@search(corpus=memory, ...)` used an invalid memory key.
pub const SEARCH_MEMORY_KEY_INVALID: &str = "vox/search/memory-key-invalid";
/// `@search(corpus=web, ...)` violated web policy constraints.
pub const SEARCH_WEB_POLICY_DENIED: &str = "vox/search/web-policy-denied";
/// `@hole(...)` fixture has not been filled.
pub const FIXTURE_UNFILLED_HOLE: &str = "vox/fixture/unfilled-hole";
/// `@hole(...)` fixture is stale per ledger policy.
pub const FIXTURE_STALE_HOLE: &str = "vox/fixture/stale-hole";
/// TypeScript codegen observed AI fixtures without TS lowering support.
pub const CODEGEN_MISSING_TS_AI_LOWERING: &str = "vox/codegen/missing-ts-ai-lowering";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// All Phase-2 diagnostic IDs in the order they were introduced.
/// Used by `--explain` to enumerate known IDs.
pub const ALL_PHASE2_IDS: &[&str] = &[
    LLM_DIRECT_PROVIDER_CALL,
    SECRET_ENV_GET_SHAPE,
    CRYPTO_BANNED_CRATE_IMPORT,
    SECRET_LEAKED_TO_SPAN,
    STYLE_DUPLICATE_PREFIX_NAME,
    STYLE_LONG_RANGE_COUPLING,
    CONTROL_FLOW_QUESTION_MARK_OPPORTUNITY,
    CONTROL_FLOW_OPTION_COMBINATOR_OPPORTUNITY,
    DOC_MISSING_ADR_CITATION,
    DECORATOR_POSITION_MISMATCH,
    REQUIRE_JUSTIFICATION_PROSE_REQUIRED,
    HANDLER_PANICKING_BUILTIN,
    STATE_MACHINE_UNREACHABLE_STATE,
    AUTH_ENDPOINT_MISSING_DECORATOR,
];

/// All known diagnostic IDs across all phases.
pub const ALL_KNOWN_IDS: &[&str] = &[
    // Core type errors (v0.6)
    TYPES_TYPE_MISMATCH,
    TYPES_UNDEFINED_VARIABLE,
    TYPES_ARG_COUNT_MISMATCH,
    TYPES_ARG_TYPE_MISMATCH,
    TYPES_FIELD_NOT_FOUND,
    TYPES_METHOD_NOT_FOUND,
    TYPES_UNRESOLVED_TYPE,
    LLM_DIRECT_PROVIDER_CALL,
    SECRET_ENV_GET_SHAPE,
    CRYPTO_BANNED_CRATE_IMPORT,
    MESH_TRANSPORT_UNSAFE_IROH,
    SECRET_LEAKED_TO_SPAN,
    STYLE_DUPLICATE_PREFIX_NAME,
    STYLE_LONG_RANGE_COUPLING,
    CONTROL_FLOW_QUESTION_MARK_OPPORTUNITY,
    CONTROL_FLOW_OPTION_COMBINATOR_OPPORTUNITY,
    DOC_MISSING_ADR_CITATION,
    DECORATOR_POSITION_MISMATCH,
    REQUIRE_JUSTIFICATION_PROSE_REQUIRED,
    HANDLER_PANICKING_BUILTIN,
    STATE_MACHINE_UNREACHABLE_STATE,
    AUTH_ENDPOINT_MISSING_DECORATOR,
    RUNTIME_FUEL_EXHAUSTED,
    RUNTIME_ALLOC_CAP_EXCEEDED,
    RUNTIME_STACK_OVERFLOW,
    RUNTIME_HOST_PANIC,
    RUNTIME_CAPABILITY_VIOLATION,
    EFFECT_MISSING_NET_DECL,
    EFFECT_UNJUSTIFIED_NET_DECL,
    WORKFLOW_NON_DETERMINISTIC_BUILTIN,
    EFFECT_PURE_VIOLATED,
    TYPES_ID_REQUIRED_AT_BOUNDARY,
    TYPES_ANONYMOUS_ERROR_TYPE,
    SYNTAX_VERSION_MISMATCH,
    API_DEPRECATED_CALLSITE,
    CORPUS_TRAINING_INELIGIBLE_IMPORT,
    SYNTAX_DECORATOR_POSITION_POLICY,
    CODEGEN_GENERATED_FILE_DRIFT,
    CODEGEN_DECORATOR_WITHOUT_ATTRIBUTE,
    AI_UNKNOWN_TASK_CATEGORY,
    PROMPT_INVALID_STAGE,
    PROMPT_SECRET_LEAKAGE,
    SUBAGENT_CHAIN_DEPTH_EXCEEDED,
    SUBAGENT_DISTRIBUTED_NOT_WIRED,
    SEARCH_CORPUS_DENIED,
    SEARCH_MEMORY_KEY_INVALID,
    SEARCH_WEB_POLICY_DENIED,
    FIXTURE_UNFILLED_HOLE,
    FIXTURE_STALE_HOLE,
    CODEGEN_MISSING_TS_AI_LOWERING,
    RETIRED_DECORATOR_USAGE,
    RETIRED_CRATE_IMPORT,
    RETIRED_ENV_VAR,
    RETIRED_MEMORY_API,
    RETIRED_CAPACITOR,
    IMPORT_CYCLE,
];

/// Find the explain URL for a given diagnostic ID.
pub fn explain_url(id: &str) -> String {
    format!("https://voxlang.org/diag/{}", id)
}

/// Returns true if `id` is a known stable diagnostic ID.
pub fn is_known_id(id: &str) -> bool {
    ALL_KNOWN_IDS.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ids_have_correct_prefix() {
        for id in ALL_KNOWN_IDS {
            assert!(
                id.starts_with("vox/"),
                "diagnostic ID must start with 'vox/': {id}"
            );
            let parts: Vec<&str> = id.splitn(3, '/').collect();
            assert_eq!(
                parts.len(),
                3,
                "diagnostic ID must have exactly 3 slash-separated parts: {id}"
            );
            assert!(
                !parts[1].is_empty() && !parts[2].is_empty(),
                "category and name must be non-empty in: {id}"
            );
        }
    }

    #[test]
    fn no_duplicate_ids() {
        let mut seen = std::collections::HashSet::new();
        for id in ALL_KNOWN_IDS {
            assert!(seen.insert(id), "duplicate diagnostic ID: {id}");
        }
    }

    #[test]
    fn explain_url_format() {
        let url = explain_url(LLM_DIRECT_PROVIDER_CALL);
        assert_eq!(url, "https://voxlang.org/diag/vox/llm/direct-provider-call");
    }

    #[test]
    fn is_known_id_works() {
        assert!(is_known_id(LLM_DIRECT_PROVIDER_CALL));
        assert!(!is_known_id("vox/fake/nonexistent"));
    }
}
