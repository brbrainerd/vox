//! Path C reactive components → React TSX via `hir_emit`.
//!
//! **Web IR (OP-0193+):** the `view:` body is taken from
//! [`super::web_ir::emit_tsx::emit_component_view_tsx`] after lowering the module Web IR graph.
//! When [`validate_web_ir`](super::web_ir::validate::validate_web_ir) reports **blocking** diagnostics
//! (non-advisory), or when no Web IR view root exists for the component, emit **fails fast**: the
//! return uses a small placeholder fragment and the diagnostics are appended to
//! `ReactiveViewBridgeStats::reactive_view_emit_failures`.
//!
//! Legacy [`emit_hir_expr`](crate::hir_emit::emit_hir_expr) is computed **only** to
//! classify `ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch` vs `WebIrViewEmitted`
//! (whitespace-normalized compare); it is never selected as the emitted view body.
//!
//! **Diagnostics:** `VOX_WEBIR_REACTIVE_TRACE=1` logs one stderr line per reactive view decision
//! (component name + pathway). Aggregate counts: `ReactiveViewBridgeStats`.
//!
//! **Behavior adapter (OP-S037):** non-`view:` reactive members still flow through `emit_block_stmts` /
//! `emit_hir_expr`; `behavior_nodes` / preview emit from [`super::web_ir`] is the structural mirror—keep
//! pathway counters and parity guards updated together.
//!
//! **Route→behavior map (OP-S073) + notes B/C (OP-S163 / S195):** reactive `view:` bodies are keyed by component
//! name for [`super::web_ir::emit_tsx::emit_component_view_tsx`] selection; do not rename without updating
//! [`super::web_ir::WebIrModule::view_roots`] lowering.

mod bindings;
mod effects;
mod hooks;
mod imports;
mod view;

pub use bindings::collect_component_import_refs;
pub use effects::generate_reactive_component;
pub use imports::{emit_external_lib_support, emit_react_es_import_lines};
pub use view::{ReactiveViewBridgeStats, ReactiveViewEmitPathway, normalize_reactive_view_jsx_ws};

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::lower::lower_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse;

    fn compile(src: &str) -> Vec<(String, String)> {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse error");
        let hir = lower_module(&module);
        let bundle = crate::codegen_ts::projection_bundle::project_bundle_from_hir(&hir);
        let mut stats = ReactiveViewBridgeStats::default();
        hir.components
            .iter()
            .map(|rc| generate_reactive_component(&hir, rc, &bundle.web, &mut stats))
            .collect()
    }

    fn get(files: &[(String, String)], name: &str) -> String {
        files
            .iter()
            .find(|(f, _)| f == name)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    #[test]
    fn test_cross_component_import_emitted() {
        let files = compile(
            "component Inner() { view: panel() { text() { \"hi\" } } }\n\
             component Outer() { view: column() { Inner() } }",
        );
        let outer = get(&files, "Outer.tsx");
        assert!(
            outer.contains("import { Inner } from \"./Inner\";"),
            "expected import for Inner in Outer.tsx, got:\n{outer}"
        );
    }

    #[test]
    fn imported_react_component_used_in_view_emits_es_import_not_sibling_import() {
        // Phase 5 S2: an `import react …` component referenced in a `view:` must
        // (a) emit the ES import, (b) render as a component tag, and (c) NOT emit a
        // bogus sibling `./Name` import (the ES import already binds it).
        let files = compile(
            "import react MyButton from \"@acme/btn\"\n\
             component Page() { view: column() { MyButton() } }",
        );
        let page = get(&files, "Page.tsx");
        assert!(
            page.contains("import MyButton from \"@acme/btn\";"),
            "expected ES import for MyButton, got:\n{page}"
        );
        assert!(
            page.contains("<MyButton"),
            "expected MyButton rendered as a component tag, got:\n{page}"
        );
        assert!(
            !page.contains("from \"./MyButton\""),
            "must NOT emit a sibling ./MyButton import, got:\n{page}"
        );
    }

    #[test]
    fn imported_react_namespace_emits_namespace_import() {
        // Phase 5 S2: a namespace react import emits `import * as X from "<spec>"`.
        //
        // LIMITATION (documented, not a stub): using a namespace *member* as a JSX
        // element (`<Dialog.Root>`) is not yet supported — `Dialog.Root()` in a
        // `view:` lowers to a call expression (`{Dialog.Root()}`), not a
        // `<Dialog.Root/>` tag, because dotted member-call → JSX-element lowering
        // is not implemented. The supported path for Radix-style component sets is
        // the NAMED form, which renders as tags:
        //   `import react { Dialog, DialogContent } from "@radix-ui/react-dialog"`.
        let files = compile(
            "import react * as Dialog from \"@radix-ui/react-dialog\"\n\
             component Page() { view: column() { text() { \"x\" } } }",
        );
        let page = get(&files, "Page.tsx");
        assert!(
            page.contains("import * as Dialog from \"@radix-ui/react-dialog\";"),
            "expected namespace import line, got:\n{page}"
        );
    }

    #[test]
    fn imported_react_named_components_render_as_tags() {
        // Phase 5 S2: the NAMED form (the supported Radix-style path) emits a
        // grouped named import and renders each name as a component tag.
        let files = compile(
            "import react { Dialog, DialogContent } from \"@radix-ui/react-dialog\"\n\
             component Page() { view: column() { Dialog() DialogContent() } }",
        );
        let page = get(&files, "Page.tsx");
        assert!(
            page.contains("import { Dialog, DialogContent } from \"@radix-ui/react-dialog\";"),
            "expected grouped named import, got:\n{page}"
        );
        assert!(
            page.contains("<Dialog"),
            "expected <Dialog> tag, got:\n{page}"
        );
        assert!(
            page.contains("<DialogContent"),
            "expected <DialogContent> tag, got:\n{page}"
        );
        assert!(
            !page.contains("from \"./Dialog\""),
            "must not emit sibling import for an external named component, got:\n{page}"
        );
    }

    #[test]
    fn mantine_import_injects_css_and_provider_guidance() {
        // Phase 5 SSOT: importing a known css_file lib auto-injects its required
        // CSS import; a mandatory-provider lib emits setup guidance.
        let files = compile(
            "import react { Button } from \"@mantine/core\"\n\
             component Page() { view: column() { Button() } }",
        );
        let page = get(&files, "Page.tsx");
        assert!(
            page.contains("import \"@mantine/core/styles.css\";"),
            "expected Mantine CSS import to be injected, got:\n{page}"
        );
        assert!(
            page.contains("requires <MantineProvider>"),
            "expected MantineProvider guidance, got:\n{page}"
        );
    }

    #[test]
    fn radix_import_emits_no_css_or_provider_guidance() {
        // Headless/unstyled libs (Radix) need no CSS import and no provider.
        let files = compile(
            "import react { Dialog } from \"@radix-ui/react-dialog\"\n\
             component Page() { view: column() { Dialog() } }",
        );
        let page = get(&files, "Page.tsx");
        assert!(
            !page.contains("styles.css"),
            "Radix must not inject a CSS import, got:\n{page}"
        );
        assert!(
            !page.contains("vox-interop:"),
            "Radix must not emit provider guidance, got:\n{page}"
        );
    }

    #[test]
    fn test_no_import_for_html_primitives() {
        let files = compile("component Card() { view: panel() { text() { \"x\" } } }");
        let card = get(&files, "Card.tsx");
        // 'panel' and 'text' are primitives, must not generate import lines
        assert!(
            !card.contains("import { panel }"),
            "primitive 'panel' should not be imported"
        );
        assert!(
            !card.contains("import { text }"),
            "primitive 'text' should not be imported"
        );
    }

    #[test]
    fn derived_calling_reactive_callee_includes_state_in_dep_array() {
        // Phase E end-to-end: a `derived` that calls a `@reactive`-annotated free function
        // which reads a reactive `state` binding should include that binding in the
        // emitted React `useMemo` dep array. Without the wiring (or without `@reactive`)
        // the dep array would be empty, leaving the memo stale on state updates.
        let files = compile(
            "@reactive fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            counter.contains("useMemo(() => double_it(count), [count])"),
            "expected useMemo dep array to include `count` traced through @reactive callee:\n{counter}"
        );
    }

    #[test]
    fn derived_calling_non_reactive_callee_emits_dep_inference_over_track_hint() {
        // Phase E tier-2: when `derived` calls a visible in-module fn that is not
        // `@reactive`, emit a `// dep_inference.over_track` hint comment above the
        // useMemo line so downstream readers (humans + AI) see why the dep array might
        // miss reactive reads through the call.
        let files = compile(
            "fn opaque(x: int) to int { x + 1 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = opaque(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            counter.contains("// dep_inference.over_track"),
            "expected over_track hint comment:\n{counter}"
        );
        assert!(
            counter.contains("`opaque`"),
            "expected hint to name the offending callee:\n{counter}"
        );
    }

    #[test]
    fn derived_with_only_reactive_callees_omits_dep_inference_hint() {
        // Counterpart: when every called in-module fn is `@reactive`, no hint comment
        // appears (the analyzer can fully descend, no over-tracking risk).
        let files = compile(
            "@reactive fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        assert!(
            !counter.contains("dep_inference.over_track"),
            "did not expect over_track hint:\n{counter}"
        );
    }

    #[test]
    fn derived_calling_non_reactive_callee_omits_state_from_dep_array() {
        // Counterpart: without `@reactive`, the analyzer must NOT recurse into the callee
        // body (conservative under-tracking). The dep array is empty and the memo will be
        // stale — opt-in is the policy.
        let files = compile(
            "fn double_it(c: int) to int { c * 2 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = double_it(count)\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter = get(&files, "Counter.tsx");
        // The arg `count` still appears as a direct read, so dep array is `[count]`. To
        // truly demonstrate the under-tracking, use an arg that doesn't reference state:
        let files2 = compile(
            "fn opaque() to int { 42 }\n\
             component Counter() {\n\
               state count: int = 0\n\
               derived doubled = opaque()\n\
               view: text() { \"v\" }\n\
             }",
        );
        let counter2 = get(&files2, "Counter.tsx");
        assert!(
            counter2.contains("useMemo(() => opaque(), [])"),
            "expected empty dep array (no @reactive on `opaque`):\n{counter2}"
        );
        // And the first compile should still find `count` via the direct argument read:
        assert!(
            counter.contains("[count]"),
            "expected `count` dep from the direct argument read:\n{counter}"
        );
    }

    #[test]
    fn test_import_inside_if_branch() {
        let files = compile(
            "component Badge() { view: text() { \"x\" } }\n\
             component Host(show: bool) {\n\
               state s: bool = false\n\
               view: if s { Badge() } else { text() { \"no\" } }\n\
             }",
        );
        let host = get(&files, "Host.tsx");
        assert!(
            host.contains("import { Badge } from \"./Badge\";"),
            "expected Badge import inside if branch:\n{host}"
        );
    }
}
