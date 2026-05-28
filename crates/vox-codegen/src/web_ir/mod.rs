//! Internal **WebIR** schema — **ADR 012** (normative types, lowering, validation, TSX preview emit).
//!
//! - [`lower::lower_hir_to_web_ir`] — HIR → `WebIrModule` (views, routes, behaviors).
//! - [`validate::validate_web_ir`] — structural checks before target emission.
//! - [`emit_tsx::emit_component_view_tsx`] — JSX string from a lowered view root (parity / tests).
//!
//! ## Schema completeness checklist (OP-0049), by family
//!
//! - **DOM ([`DomNode`])** — Phase 1: [`DomNode::Element`], [`DomNode::Text`], [`DomNode::Expr`],
//!   Structural: [`DomNode::Fragment`], [`DomNode::Slot`],
//!   [`DomNode::Conditional`], [`DomNode::Loop`] (validator walks edges when linked from [`WebIrModule::view_roots`]).
//! - **Behavior ([`BehaviorNode`])** — state / derived / effect / handlers / actions; lowering must preserve
//!   names for codegen binding (`validate` will deepen).
//! - **Style ([`StyleNode`])** — rules and selectors; optional in Phase 1.
//! - **Routes ([`RouteNode`])** — [`RouteContract`] shape (`id`, `pattern`, `meta` JSON); see invariants on [`RouteContract`].
//! - **Interop ([`InteropNode`])** — refs and escape hatches; count for token / audit budgets (OP-0053).
//! - **Shell ([`WebIrModule`])** — arena + [`WebIrModule::view_roots`]; [`WebIrVersion`] and [`SourceSpanTable`] versioning (OP-0055).
//!
//! **Interop policy (OP-S053 / OP-S185):** populated [`WebIrModule::interop_nodes`] are validated inside
//! [`validate::validate_web_ir`] (non-empty specifiers / escape-hatch reasons). Route+data shape
//! notes: [`RouteContract`] JSON in [`RouteNode::RouteTree`] must stay serde-stable for tooling (OP-S153).

pub mod async_state;
pub mod emit_tsx;
pub mod href_emit;
pub mod layer_emit;
pub mod lower;
pub mod paginated_emit;
pub mod primitives;
pub mod semantic_ui_emit;
pub mod validate;
pub mod validate_a11y;
pub mod validate_keys;
pub mod validate_overlay;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Module root
// ---------------------------------------------------------------------------

/// Increment when the schema gains breaking layout changes. ADR 012 CP-002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum WebIrVersion {
    /// Initial frozen placeholder (Phase 0).
    #[default]
    V0_1,
}

/// Normative Z-tiers for the seven-tier ladder (ADR 034 / VUV Layered Layout Discipline).
///
/// **Rule 2:** within-tier ordering is deliberately unspecified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZTier {
    Background,
    Content,
    Chrome,
    Popover,
    Modal,
    Toast,
    SystemOverlay,
}

impl ZTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "background" => Some(Self::Background),
            "content" => Some(Self::Content),
            "chrome" => Some(Self::Chrome),
            "popover" => Some(Self::Popover),
            "modal" => Some(Self::Modal),
            "toast" => Some(Self::Toast),
            "system_overlay" => Some(Self::SystemOverlay),
            _ => None,
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Content => "content",
            Self::Chrome => "chrome",
            Self::Popover => "popover",
            Self::Modal => "modal",
            Self::Toast => "toast",
            Self::SystemOverlay => "system_overlay",
        }
    }
}

/// Web-facing projection IR — same serde layout as [`WebIrModule`]; use this name at core/projection boundaries.
pub type WebProjectionIR = WebIrModule;

/// Opaque id into [`SourceSpanTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpanId(pub u32);

/// Phase 0 span payload (byte offsets into a resolved source file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file_id: u32,
    pub start: u32,
    pub end: u32,
}

/// Byte-offset table for WebIR nodes (OP-0055). **Constraints:** ids are dense `0..len-1` from
/// [`SourceSpanTable::push_span`]; consumers must not synthesize ids without inserting spans. Version
/// bumps when file_id interpretation changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceSpanTable {
    pub spans: Vec<SourceSpan>,
}

impl SourceSpanTable {
    pub fn push_span(&mut self, span: SourceSpan) -> SourceSpanId {
        let id = SourceSpanId(self.spans.len() as u32);
        self.spans.push(span);
        id
    }

    pub fn get(&self, id: SourceSpanId) -> Option<&SourceSpan> {
        self.spans.get(id.0 as usize)
    }
}

/// Lowered `@scheduled("…") fn …` for worker / manifest tooling (WebIR shell; ADR 012).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJobSpec {
    /// Function name from source.
    pub name: String,
    /// Interval or cron string from `@scheduled("…")`.
    pub interval: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpanId>,
}

/// Classify optionality for validator + emit boundary (ADR 012 nullability policy).
///
/// **Fail-fast (OP-0050):** invalid combinations (e.g. `Required` with no initializer where the
/// target requires one) should surface as `WebIrDiagnostic` at validate/emit, not silent defaults.
///
/// **Validator (OP-S011 / OP-S017):** the behavior stage of [`validate_web_ir`](validate::validate_web_ir) rejects
/// `Required` [`BehaviorNode::StateDecl`] rows with no `initial`; `Optional` / `Defaulted` deepen with emit caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldOptionality {
    /// Must have a concrete lowered value before codegen (see `web_ir_validate.behavior.required_state_without_initial`).
    Required,
    /// May omit initializer at the WebIR boundary when the target allows undefined.
    Optional,
    /// Compile-time default or placeholder policy (validator may require explicit RHS per ADR 012).
    Defaulted,
}

/// Stable node id within the dom node arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct DomNodeId(pub u32);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebIrModule {
    /// DOM arena indexed by [`DomNodeId`]; [`DomNode::Element`] carries a matching `id` after lowering.
    pub dom_nodes: Vec<DomNode>,
    /// Reactive or classic component name → entry [`DomNodeId`] into [`Self::dom_nodes`] for its `view:` / JSX tail.
    #[serde(default)]
    pub view_roots: Vec<(String, DomNodeId)>,
    /// Lowered reactive behaviors in pipeline order (`StateDecl`, `DerivedDecl`, `EffectDecl`, …).
    pub behavior_nodes: Vec<BehaviorNode>,
    /// CSS from classic `@component` `style { }` and related surfaces (may be empty for Path-C-only modules).
    pub style_nodes: Vec<StyleNode>,
    /// Stage **R**: client [`RouteNode::RouteTree`] plus HTTP loaders and RPC-shaped contracts.
    pub route_nodes: Vec<RouteNode>,
    /// `@scheduled` jobs from HIR ([`vox_compiler::hir::HirFn::schedule_interval`]).
    #[serde(default)]
    pub scheduled_jobs: Vec<ScheduledJobSpec>,
    /// External / escape-hatch nodes (Phase 1 may leave empty; reserved for interop audits — OP-S053).
    pub interop_nodes: Vec<InteropNode>,
    /// Lowering-time notes (e.g. unlowered AST); not a substitute for [`validate::validate_web_ir`] diagnostics.
    pub diagnostic_nodes: Vec<WebIrDiagnostic>,
    /// Byte-offset back-references for tools; ids reference rows in this table.
    pub spans: SourceSpanTable,
    pub version: WebIrVersion,
}

// ---------------------------------------------------------------------------
// DOM
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomNode {
    Element {
        id: DomNodeId,
        tag: String,
        /// Attribute name → IR-level value text (lowering fills escaping policy).
        attrs: Vec<(String, String)>,
        children: Vec<DomNodeId>,
        span: Option<SourceSpanId>,
    },
    Text {
        content: String,
        span: Option<SourceSpanId>,
    },
    Fragment {
        children: Vec<DomNodeId>,
        span: Option<SourceSpanId>,
    },
    Slot {
        name: Option<String>,
        span: Option<SourceSpanId>,
    },
    Conditional {
        /// Predicate expression placeholder (HIR/lowering supplies concrete form).
        predicate: String,
        then_children: Vec<DomNodeId>,
        else_children: Vec<DomNodeId>,
        span: Option<SourceSpanId>,
    },
    Loop {
        /// Iterator binding placeholder.
        iterator: String,
        /// Optional React key expression string (from `key=expr` clause).
        key: Option<String>,
        body: Vec<DomNodeId>,
        span: Option<SourceSpanId>,
    },
    /// Dynamic TypeScript/JSX fragment already in render position (lowering fallback / expression leaf).
    Expr {
        ts: String,
        span: Option<SourceSpanId>,
    },
}

// ---------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BehaviorNode {
    StateDecl {
        name: String,
        initial: Option<String>,
        optionality: FieldOptionality,
        span: Option<SourceSpanId>,
    },
    DerivedDecl {
        name: String,
        expr: String,
        span: Option<SourceSpanId>,
    },
    EffectDecl {
        deps: Vec<String>,
        body: String,
        span: Option<SourceSpanId>,
    },
    EventHandler {
        target_dom: Option<DomNodeId>,
        event: String,
        handler: String,
        span: Option<SourceSpanId>,
    },
    Action {
        name: String,
        payload_expr: Option<String>,
        span: Option<SourceSpanId>,
    },
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

/// Typed-ish CSS value bucket (Phase 2 deepens this per blueprint CP-031; OP-0059 extension hook).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StyleDeclarationValue {
    /// Unparsed CSS text as emitted.
    Raw(String),
    /// Design-token or variable reference name.
    TokenRef(String),
    /// Parsed color value.
    Color(CssColor),
    /// Numeric length with unit.
    Length(f64, LengthUnit),
    /// Valid keyword (e.g., "flex", "none", "auto").
    Keyword(String),
    /// Unitless number.
    Number(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CssColor {
    Hex(String),
    Rgb(u8, u8, u8),
    Rgba(u8, u8, u8, f32),
    Named(String),
    Hsl(f32, f32, f32),
    Var(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LengthUnit {
    Px,
    Rem,
    Em,
    Percent,
    Vw,
    Vh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StyleNode {
    Rule {
        selector: StyleSelector,
        declarations: Vec<(String, StyleDeclarationValue)>,
        /// Specificity score (A, B, C) where A=ID, B=Class/Pseudo-class, C=Element/Pseudo-element.
        specificity: (u8, u8, u8),
        /// Came from a `raw_css { }` escape hatch — raw CSS values are warnings, not errors.
        is_raw_css: bool,
        span: Option<SourceSpanId>,
    },
    Selector(StyleSelector),
    Declaration {
        property: String,
        value: StyleDeclarationValue,
        important: bool,
        span: Option<SourceSpanId>,
    },
    TokenRef {
        name: String,
        span: Option<SourceSpanId>,
    },
    AtRule {
        name: String,
        prelude: String,
        block: Option<String>,
        span: Option<SourceSpanId>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StyleSelector {
    Class(String),
    Id(String),
    Element(String),
    /// Full selector text as authored in a `style { }` block (e.g. `.btn`, `h1`, `.a > .b`).
    Unparsed(String),
    Compound(Vec<StyleSelector>),
    Pseudo {
        base: Box<StyleSelector>,
        pseudo: String,
    },
}

// ---------------------------------------------------------------------------
// Lowering / validation telemetry (OP-0078, OP-0094)
// ---------------------------------------------------------------------------

/// Counts produced alongside [`lower::lower_hir_to_web_ir_with_summary`] for gates and pipeline fixtures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebIrLowerSummary {
    /// Count of [`RouteNode::RouteTree`] from `hir.client_routes`.
    pub client_route_trees: usize,
    pub http_loader_contracts: usize,
    pub server_fn_contracts: usize,
    pub query_fn_contracts: usize,
    pub mutation_contracts: usize,
    /// Path C reactive components in HIR (each may contribute `view_roots` + `behavior_nodes`).
    pub components: usize,
    /// Classic `@component fn` bodies lowered into [`WebIrModule::view_roots`] (OP-0179).
    pub classic_component_views_lowered: usize,
    /// Classic components without a JSX view in the supported emit shape (remainder).
    pub classic_components_deferred: usize,
    pub style_rules_lowered: usize,
    /// `DomNode::Expr` leaves produced when JSX lowering falls back to TS snippets.
    pub dom_expr_fallbacks: usize,
    /// Rows appended to [`WebIrModule::diagnostic_nodes`] from lowering gaps.
    pub lowering_diagnostics: usize,
    /// Count of `routes { }` entries with a `with loader:` / `loader:` binding (manifest parity).
    pub route_entries_with_loader: usize,
    /// Count of route entries with explicit `pending:` / pending component binding.
    pub route_entries_with_pending: usize,
    /// `routes { }` blocks that declare `not_found:`.
    pub route_blocks_with_not_found: usize,
    /// `routes { }` blocks that declare `error:`.
    pub route_blocks_with_error: usize,
    /// Rows in [`WebIrModule::scheduled_jobs`] from lowering.
    pub scheduled_jobs_lowered: usize,
}

/// Populated by [`validate::validate_web_ir_with_metrics`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebIrValidateMetrics {
    pub view_roots_walked: usize,
    pub dom_nodes_traversed: usize,
    pub route_contract_ids_checked: usize,
    pub behavior_nodes_checked: usize,
    pub style_nodes_checked: usize,
    pub scheduled_jobs_checked: usize,
}

// ---------------------------------------------------------------------------
// Route / data
// ---------------------------------------------------------------------------

/// Route and data-contract envelope (OP-0061). Keep payloads small JSON-shaped; deep RPC schemas
/// stay on HIR/server layers—this enum is for router-facing summaries only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteNode {
    RouteTree {
        routes: Vec<RouteContract>,
        span: Option<SourceSpanId>,
    },
    LoaderContract {
        route_id: String,
        contract: String,
        span: Option<SourceSpanId>,
    },
    ServerFnContract(ServerFnContract),
    MutationContract(MutationContract),
}

/// Client route entry in a [`RouteNode::RouteTree`] (OP-0051).
///
/// **Invariants:** `id` is stable within the owning module (often `route_{i}` from lowering); `pattern`
/// is the URL pattern string as authored; `meta` holds small JSON only (e.g. target component name)—keep
/// it object-shaped for tooling; oversized payloads belong in HIR diagnostics, not opaque blobs here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteContract {
    /// Stable within a module; duplicates fail the route stage of [`validate_web_ir`](validate::validate_web_ir) (`web_ir_validate.route.duplicate_contract_id`).
    pub id: String,
    /// URL pattern string as authored on the client route entry.
    pub pattern: String,
    /// Small JSON attachment (e.g. target component name); keep router-shaped, not full RPC schemas.
    pub meta: serde_json::Value,
    /// Nested client routes (same shape as HIR [`vox_compiler::ast::decl::RouteEntry::children`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<RouteContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerFnContract {
    pub name: String,
    pub export_path: String,
    pub signature: String,
    pub span: Option<SourceSpanId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationContract {
    pub name: String,
    pub payload_type: String,
    pub span: Option<SourceSpanId>,
}

// ---------------------------------------------------------------------------
// Interop
// ---------------------------------------------------------------------------

/// Cross-language and escape surfaces (OP-0053): every variant should be explicit in audits—prefer
/// narrowing imports over [`InteropNode::EscapeHatchExpr`]; raw expr carries policy risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteropNode {
    ReactComponentRef {
        component: String,
        import_source: String,
        props: Vec<(String, String)>,
        span: Option<SourceSpanId>,
    },
    ExternalModuleRef {
        specifier: String,
        named: Option<String>,
        span: Option<SourceSpanId>,
    },
    EscapeHatchExpr {
        /// Raw target-language fragment; validator enforces policy (ADR 012 Phase 3).
        expr: String,
        reason: String,
        span: Option<SourceSpanId>,
    },
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Severity level for a [`WebIrDiagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebIrDiagnosticSeverity {
    /// Hard compile-time error; the output is unusable.
    Error,
    /// Advisory violation that should be fixed but does not break output.
    Warning,
    /// Informational hint only.
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebIrDiagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<SourceSpanId>,
    /// Dashboard facet, e.g. `dom`, `route`, `behavior`, `style`, `lower`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl WebIrDiagnostic {
    /// Derive severity from the code: advisory-only codes are `Warning`,
    /// everything else is `Error`.
    pub fn severity(&self) -> WebIrDiagnosticSeverity {
        // Codes explicitly designated as warnings (advisory violations).
        if matches!(
            self.code.as_str(),
            "web_ir_validate.a11y.anchor_missing_href"
                | "web_ir_validate.a11y.input_missing_label"
                | "web_ir_validate.a11y.low_contrast"
                | "vox/codegen/missing-ts-ai-lowering"
        ) {
            WebIrDiagnosticSeverity::Warning
        } else {
            WebIrDiagnosticSeverity::Error
        }
    }
}

// Lifecycle: bump [`WebIrVersion`] when breaking serialized layout; keep [`validate_web_ir`] in sync
// with new edges; document consumer contracts in ADR 012 and the internal Web IR blueprint (OP-0063).

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use crate::web_ir::validate::validate_web_ir;

    #[test]
    fn web_ir_module_default_validates() {
        let module = WebIrModule::default();
        assert!(validate_web_ir(&module).is_empty());
    }
}
