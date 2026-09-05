//! Registry of all built-in detection rules.

/// Subtle AI cop-out patterns: placeholder-string returns, "implement later" comments,
/// mock-named functions in non-test code, custom-type default returns, conditional stub
/// branches, assertion-only / early-return-only function bodies. Complements `stub`,
/// `empty_body`, `hollow_fn`, and `victory_claim`.
pub mod ai_laziness;
/// Flags use of APIs or patterns marked deprecated in project policy.
pub mod deprecated_usage;
/// Heuristic duplicate / near-duplicate logic across files.
pub mod dry_violation;
/// Functions or handlers with empty or trivial bodies (`pass`-style placeholders).
pub mod empty_body;
/// `lib.rs` / module files with too many top-level definitions or oversized type dumps.
pub mod file_organization;
/// Single files that exceed line or method-count thresholds (“god object” smell).
pub mod god_object;
/// Functions with trivially-default return values (compile but do nothing meaningful).
pub mod hollow_fn;
/// CR / CRLF in source files vs LF policy (`vox ci line-endings` parity).
pub mod line_endings;
/// Suspicious literals (large ints, long strings) that should be named constants.
pub mod magic_value;
/// Functions that are declared but not called anywhere in the crate.
pub mod reachability;
/// Scaling risks: blocking I/O in async, unbounded reads, SQL/HTTP heuristics.
pub mod scaling;
/// Optional JSON-schema cross-check when a schema path is configured.
pub mod schema_compliance;
/// High-entropy strings and common secret patterns (keys, tokens).
pub mod secrets;
/// Directory sprawl: too many files per folder or banned generic filenames (`utils.rs`, …).
pub mod sprawl;
/// String constants where an enum or ADT would be clearer.
pub mod stringly_typed_enum;
/// `TODO` / `unimplemented!` / obvious stub markers left in shipped code.
pub mod stub;
/// References to symbols that are not defined or imported in the current compilation unit.
pub mod unresolved_ref;
/// Modules declared but never imported or wired into the build graph.
pub mod unwired_module;
/// Heuristic `.unwrap()` in Rust (informational nudge).
pub mod unwrap_call;
/// Premature “done” comments or victory language without matching tests or implementation.
pub mod victory_claim;

/// Enforces Cargo.toml workspace dependencies inheritance and detects orphan crates
pub mod workspace_drift;

/// Exported `fn` in `examples/golden/` Vox files must be called from an `@test` block.
pub mod no_test_for_pub_fn;

/// Rust library files with `pub fn` declarations but no `#[test]` blocks (advisory Info).
pub mod untested_pub_api;

/// Identifiers with duplicated prefix segments (e.g. `user_user_id`).
pub mod duplicate_prefix;

/// Public functions in critical crates missing ADR/TASK citations in doc comments.
pub mod adr_citation;

/// Decorator/keyword position mismatches in Vox files (`durable fn` → `@durable fn`).
pub mod decorator_position;

/// `match` over `Result`/`Option` that can be replaced with `?` or a combinator.
pub mod question_mark;

/// Complex `@require(...)` expressions lacking adequate justification prose.
pub mod require_justification;

/// Panicking builtins inside actor handlers or workflow activities.
pub mod panicking_builtin;

/// Variables defined and last-used more than 80 lines apart.
pub mod long_range_coupling;
/// Option/Result match patterns that can use combinators (`.map`, `.unwrap_or`, etc.).
pub mod option_combinator;
/// `@secret`-tagged field names appearing in tracing span attributes or log calls.
pub mod secret_span;
/// Declared states in `state_machine` blocks with no outgoing `->` transitions.
pub mod state_machine_unreachable;

/// `str`-typed ID parameters at API boundaries (`@query`, `@mutation`, `@server`, `@activity`, actor message handlers).
pub mod id_at_boundary;

/// `Result[T, str]` or anonymous error type on a public function boundary in Vox files.
pub mod anonymous_error;

/// Invalid or conflicting `syntax_version` declarations in Vox source file headers.
pub mod syntax_version;

/// `training_eligible: true` files that import from archive/deprecated/legacy module paths.
pub mod training_eligible;

// Phase 2 security detectors (vox/llm/*, vox/secret/*, vox/crypto/*)
/// Imports or dependencies referencing banned cryptography crates (aegis, ring, …).
pub mod crypto_ban;
/// `env.get(...)` with secret-shaped argument names (KEY, SECRET, TOKEN, …).
pub mod env_secret_shape;
/// Direct HTTP calls to known LLM provider hostnames, bypassing `populi.*`.
pub mod llm_provider_call;
pub mod mesh_transport_unsafe;
/// LLM/AI env vars not declared in the vox-llm-config SSOT.
pub mod unregistered_llm_env;

/// Non-deterministic builtins (`time.now`, `random.*`, `uuid()`, etc.) inside a `workflow` body.
pub mod workflow_nondeterministic;

/// `pub fn` or `@query`/`@mutation`/`@server fn` calling HTTP/net builtins without `@uses(net)` decorator.
pub mod effect_net_decl;

/// `@pure fn` that calls an impure builtin (HTTP, I/O, random, log, etc.).
pub mod pure_fn_impure;

/// Retired decorator / import forms per AGENTS.md §Retired Surfaces (CR-L6 parity gate).
/// First 5 patterns: `@component fn`, `@server fn`, `@query fn`, `@mutation fn`, `@py.import`.
pub mod retired_decorator;

/// Retired Vox crate names (`vox-ludus`, `vox-sherpa-transcribe`). P1.4 detector.
pub mod retired_crate_import;

/// Retired TURSO_* env-var names (call-site + bare-literal). P1.4 detector.
pub mod retired_env_var;

/// Retired memory-API call-sites (`recall()` / `recall_async()`). P1.4 detector.
pub mod retired_memory_api;

/// Retired `@capacitor/*` imports + `npx cap sync` CLI invocations. P1.4 detector.
pub mod retired_capacitor;

/// Static import-cycle detector for Vox files.
/// Per-file: detects self-imports. Batch: [`import_cycles::detect_import_cycles_in_batch`]
/// detects multi-file cycles across a full workspace scan.  Phase J.19 / CR-L gate.
pub mod import_cycles;

/// Cross-crate exact-body split-brain detector. Batch:
/// [`cross_crate_dup::detect_cross_crate_dup_in_batch`] flags functions whose body is
/// byte-identical across two or more (non-platform-sibling) crates. Info severity.
pub mod cross_crate_dup;

/// Catch-all-swallow detector: flags a `match` whose `_ =>` arm returns a neutral/empty value
/// while other arms do real work — the silent-drop shape behind vanishing-value pipeline bugs.
pub mod catch_all_swallow;

/// Flags touch-test anti-patterns: #[test] fns that assert nothing meaningful.
pub mod weak_test;

#[cfg(test)]
mod semcov_wave32_tests;

use crate::rules::DetectionRule;

/// Returns all built-in detectors.
pub fn all_rules(schema_path: Option<std::path::PathBuf>) -> Vec<Box<dyn DetectionRule>> {
    vec![
        Box::new(stub::StubDetector::new()),
        Box::new(empty_body::EmptyBodyDetector::new()),
        Box::new(magic_value::MagicValueDetector::new()),
        Box::new(victory_claim::VictoryClaimDetector::new()),
        Box::new(unwired_module::UnwiredModuleDetector::new()),
        Box::new(dry_violation::DryViolationDetector::new()),
        Box::new(unresolved_ref::UnresolvedRefDetector::new()),
        Box::new(deprecated_usage::DeprecatedUsageDetector::new()),
        Box::new(secrets::SecretDetector::new()),
        Box::new(god_object::GodObjectDetector::default()),
        Box::new(sprawl::SprawlDetector::default()),
        Box::new(schema_compliance::SchemaComplianceDetector::new(
            schema_path,
        )),
        Box::new(file_organization::FileOrganizationDetector::default()),
        Box::new(stringly_typed_enum::StringlyTypedEnumDetector::new()),
        Box::new(unwrap_call::UnwrapCallDetector::new()),
        Box::new(unwrap_call::ExpectCallDetector::new()),
        Box::new(unwrap_call::PanicCallDetector::new()),
        Box::new(line_endings::LineEndingDetector::new()),
        Box::new(scaling::ScalingSurfacesDetector::new()),
        Box::new(hollow_fn::HollowFnDetector::new()),
        Box::new(reachability::ReachabilityDetector::new()),
        Box::new(workspace_drift::WorkspaceDriftDetector::new()),
        Box::new(ai_laziness::AiLazinessDetector::new()),
        Box::new(no_test_for_pub_fn::NoTestForPubFnDetector::new()),
        Box::new(untested_pub_api::UntestedPubApiDetector::new()),
        // Phase 2 security detectors (Error severity)
        Box::new(llm_provider_call::LlmProviderCallDetector::new()),
        Box::new(mesh_transport_unsafe::MeshTransportUnsafeDetector::new()),
        Box::new(env_secret_shape::EnvSecretShapeDetector::new()),
        Box::new(unregistered_llm_env::UnregisteredLlmEnvDetector::new()),
        Box::new(crypto_ban::CryptoBanDetector::new()),
        // Phase 2 style / quality detectors
        Box::new(duplicate_prefix::DuplicatePrefixDetector::new()),
        Box::new(adr_citation::AdrCitationDetector::new()),
        Box::new(decorator_position::DecoratorPositionDetector::new()),
        Box::new(question_mark::QuestionMarkDetector::new()),
        Box::new(require_justification::RequireJustificationDetector::new()),
        Box::new(panicking_builtin::PanickingBuiltinDetector::new()),
        Box::new(option_combinator::OptionCombinatorDetector::new()),
        Box::new(secret_span::SecretSpanDetector::new()),
        Box::new(state_machine_unreachable::StateMachineUnreachableDetector::new()),
        Box::new(long_range_coupling::LongRangeCouplingDetector::new()),
        Box::new(id_at_boundary::IdAtBoundaryDetector::new()),
        Box::new(anonymous_error::AnonymousErrorDetector::new()),
        Box::new(syntax_version::SyntaxVersionDetector::new()),
        Box::new(training_eligible::TrainingEligibleDetector::new()),
        Box::new(workflow_nondeterministic::WorkflowNondeterministicDetector::new()),
        Box::new(effect_net_decl::EffectNetDeclDetector::new()),
        Box::new(pure_fn_impure::PureFnImpureDetector::new()),
        // CR-L6 retirement-guard parity gate (ratified 2026-05-15).
        Box::new(retired_decorator::RetiredDecoratorDetector::new()),
        // P1.4: remaining retirement-guard detectors (council ratified 2026-05-15).
        Box::new(retired_crate_import::RetiredCrateImportDetector::new()),
        Box::new(retired_env_var::RetiredEnvVarDetector::new()),
        Box::new(retired_memory_api::RetiredMemoryApiDetector::new()),
        Box::new(retired_capacitor::RetiredCapacitorDetector::new()),
        // Phase J.19 — static import-cycle detection (per-file; see also
        // `import_cycles::detect_import_cycles_in_batch` for multi-file cycles).
        Box::new(import_cycles::ImportCyclesDetector::new()),
        Box::new(catch_all_swallow::CatchAllSwallowDetector::new()),
        Box::new(weak_test::WeakTestDetector::new()),
    ]
}

/// Returns the number of built-in rules.
pub fn rule_count() -> usize {
    55
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_instantiate() {
        let rules = all_rules(None);
        assert_eq!(rules.len(), rule_count());
        // Every rule must have a non-empty ID and name
        for rule in &rules {
            println!("Rule ID: {}", rule.id());
            assert!(!rule.id().is_empty(), "rule ID must not be empty");
            assert!(!rule.name().is_empty(), "rule name must not be empty");
            assert!(
                !rule.languages().is_empty(),
                "rule must support at least one language"
            );
        }
    }
    #[test]
    fn god_object_detector_catches_large_files() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let detector = god_object::GodObjectDetector::default();
        let content = "fn main() {}\n".repeat(detector.hard_max_lines + 1);
        let file = SourceFile::new(PathBuf::from("large.rs"), content);
        let findings = detector.detect(&file, None);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("too large"));
    }

    #[test]
    fn god_object_detector_ignores_blank_only_padding_lines() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let mut content = String::new();
        for _ in 0..600 {
            content.push('\n');
        }
        content.push_str("fn main() {}\n");
        let file = SourceFile::new(PathBuf::from("padded.rs"), content);
        let detector = god_object::GodObjectDetector::default();
        let findings = detector.detect(&file, None);
        let size_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.message.contains("non-blank lines"))
            .collect();
        assert!(
            size_findings.is_empty(),
            "blank padding should not count toward god-object size"
        );
    }

    #[test]
    fn sprawl_detector_catches_forbidden_names() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let file = SourceFile::new(PathBuf::from("utils.rs"), "fn helper() {}".to_string());
        let detector = sprawl::SprawlDetector::default();
        let findings = detector.detect(&file, None);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("forbidden"));
    }

    #[test]
    fn organization_detector_catches_bloated_lib() {
        use crate::rules::SourceFile;
        use std::path::PathBuf;
        let content =
            "pub struct A; pub struct B; pub struct C; pub struct D;".replace("; ", ";\n");
        let file = SourceFile::new(PathBuf::from("src/lib.rs"), content);
        let detector = file_organization::FileOrganizationDetector::default();
        let findings = detector.detect(&file, None);
        assert!(!findings.is_empty());
        assert!(
            findings[0]
                .message
                .contains("lib.rs contains 4 definitions")
        );
    }
}
