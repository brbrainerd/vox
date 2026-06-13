//! CR-F2c — emit-routing gate.
//!
//! Vox emits along a domain boundary: logic/backend → Rust (`codegen_rust`),
//! browser/GUI → TypeScript (`codegen_ts`), never both. A program is one
//! `HirModule`; each of its top-level construct collections belongs to exactly
//! ONE emit arm (or `Both` for a shared contract such as a typed client SDK).
//!
//! Risk this gate prevents: a NEW top-level construct kind is added to
//! `HirModule` but nobody decides which arm emits it, so it is silently dropped
//! (or double-emitted). The gate makes that a hard, fast failure.
//!
//! This is a pure struct/HIR-inspection test — no cargo builds, no Node — and
//! runs in normal `cargo test`.

use vox_compiler::hir::HirModule;

/// Which emit arm a top-level `HirModule` construct routes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// Logic / backend → `codegen_rust`.
    RustArm,
    /// Browser / GUI → `codegen_ts`.
    TsArm,
    /// Shared contract emitted on both arms (e.g. typed client SDK + Rust impl,
    /// or shared types referenced by both backend and frontend).
    Both,
}
use Arm::*;

/// Maintained routing-classification table: every top-level construct field of
/// `HirModule` → its emit arm, with a one-line rationale per entry.
///
/// When a future field is added to `HirModule`, the exhaustiveness test below
/// reports it as unclassified until a row is added here.
fn classify(field: &str) -> Option<Arm> {
    let arm = match field {
        // --- logic / backend → Rust ---
        "functions" => RustArm,      // free fns are logic
        "types" => Both,             // shared type defs referenced by both arms
        "tests" => RustArm,          // @test runs as Rust
        "examples" => RustArm,       // @example reference fns run as Rust
        "foralls" => RustArm,        // @forall properties run as Rust
        "endpoint_fns" => Both,      // @server fn = Rust impl + TS client stub
        "tables" => RustArm,         // data layer
        "indexes" => RustArm,        // data layer
        "collections" => RustArm,    // doc store data layer
        "vector_indexes" => RustArm, // data layer
        "search_indexes" => RustArm, // data layer
        "mcp_tools" => RustArm,      // MCP handlers run as Rust
        "mcp_resources" => RustArm,  // MCP resource handlers run as Rust
        "agents" => RustArm,         // native agents run as Rust
        "environments" => RustArm,   // container env specs are backend infra

        // --- browser / GUI → TypeScript ---
        "components" => TsArm,       // reactive UI components
        "client_routes" => TsArm,    // client routing blocks
        "url_decls" => Both,         // typed URLs used by both server + client
        "state_machines" => TsArm,   // UI state machines emit to TSX
        "fragments" => TsArm,        // parametric UI fragments → React components
        "reactive_modules" => TsArm, // .vox.ui modules → React context/hooks
        "forms" => TsArm,            // @form lowers to UI forms
        "back_button" => TsArm,      // native shell / mobile UI primitive
        "deep_link" => TsArm,        // native shell / mobile UI primitive
        "push" => TsArm,             // push-notification UI wiring
        "token_decls" => TsArm,      // design tokens consumed by UI emit
        "route_ids" => TsArm,        // typed route ids → codegen-ts route module

        _ => return None,
    };
    Some(arm)
}

/// Serialized keys that are NOT top-level emittable constructs and are therefore
/// intentionally excluded from the routing table.
const IGNORED_KEYS: &[&str] = &[
    "imports",          // resolved import entries (resolution metadata, not a construct)
    "rust_imports",     // declared Rust crate imports (passthrough metadata)
    "inferred_types",   // type-checker span→type map (codegen optimization metadata)
    "legacy_ast_nodes", // not-yet-typed decls (future/unknown kinds, no stable arm)
];

/// Enumerate the top-level field names of a representative `HirModule` via a
/// serde round-trip. This cannot silently drift: a newly added serialized field
/// shows up here automatically.
fn hir_module_field_names() -> Vec<String> {
    let module = HirModule::default();
    let value = serde_json::to_value(&module).expect("HirModule serializes");
    let obj = value
        .as_object()
        .expect("HirModule serializes to a JSON object");
    obj.keys().cloned().collect()
}

#[test]
fn every_hir_module_field_is_routed_or_ignored() {
    let mut unclassified = Vec::new();
    for field in hir_module_field_names() {
        if IGNORED_KEYS.contains(&field.as_str()) {
            continue;
        }
        if classify(&field).is_none() {
            unclassified.push(field);
        }
    }
    assert!(
        unclassified.is_empty(),
        "Unclassified top-level HirModule construct(s): {unclassified:?}.\n\
         Each must be added to `classify()` with an emit arm (RustArm / TsArm / Both) \
         and a one-line rationale, OR to IGNORED_KEYS if it is not an emittable \
         construct. See CR-F2c emit-routing gate."
    );
}

#[test]
fn ignored_keys_are_real_fields() {
    // Guards the ignore-list against drift: if a field is renamed/removed, its
    // ignore entry should be removed too.
    let fields = hir_module_field_names();
    for ignored in IGNORED_KEYS {
        assert!(
            fields.iter().any(|f| f == ignored),
            "IGNORED_KEYS entry {ignored:?} is not a real HirModule field anymore"
        );
    }
}

#[test]
fn known_logic_and_gui_fields_route_to_expected_arms() {
    // Guards the table against being trivially empty/wrong.
    assert_eq!(
        classify("functions"),
        Some(RustArm),
        "free fns are logic → Rust"
    );
    assert_eq!(
        classify("components"),
        Some(TsArm),
        "UI components → TypeScript"
    );
    assert_eq!(
        classify("endpoint_fns"),
        Some(Both),
        "@server fn = Rust impl + TS client stub"
    );
}
