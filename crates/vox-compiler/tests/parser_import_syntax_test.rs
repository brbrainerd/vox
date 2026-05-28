//! Tests for VUV import syntax extensions (OP-S XXX / Phase 2.6 fix).
//!
//! Verifies:
//!   1. `/` is accepted as a path separator in import statements.
//!   2. `as { Name1, Name2 }` destructured multi-import expands into separate imports.
//!   3. Both forms can coexist with the legacy dotted form and with `rust:` imports.

use vox_compiler::hir::{HirImport, lower_module};
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_imports(src: &str) -> Vec<HirImport> {
    let tokens = lex(src);
    let module = parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let hir = lower_module(&module);
    hir.imports
}

// ── slash separator ───────────────────────────────────────────────────────────

#[test]
fn slash_separated_import_parses_like_dot() {
    let dot = parse_imports("import lib.chrome.StateChip");
    let slash = parse_imports("import lib/chrome/StateChip");
    assert_eq!(dot.len(), 1);
    assert_eq!(slash.len(), 1);
    assert_eq!(
        dot[0].module_path, slash[0].module_path,
        "module_path should be the same regardless of separator"
    );
    assert_eq!(
        dot[0].item, slash[0].item,
        "item should be the same regardless of separator"
    );
}

// ── destructured `as { }` ─────────────────────────────────────────────────────

#[test]
fn destructured_import_expands_to_separate_imports() {
    let imports =
        parse_imports(r#"import lib/chrome as { StateChip, TopBar, LeftRail, StatusBar }"#);
    assert_eq!(imports.len(), 4, "four names → four HirImports");
    let items: Vec<&str> = imports.iter().map(|i| i.item.as_str()).collect();
    assert!(items.contains(&"StateChip"), "StateChip must be present");
    assert!(items.contains(&"TopBar"), "TopBar must be present");
    assert!(items.contains(&"LeftRail"), "LeftRail must be present");
    assert!(items.contains(&"StatusBar"), "StatusBar must be present");
    // All four should share the same module path.
    for imp in &imports {
        assert_eq!(
            imp.module_path,
            &["lib", "chrome"],
            "all items from the same module path"
        );
    }
}

#[test]
fn destructured_import_single_item() {
    let imports = parse_imports(r#"import surfaces/mesh as { MeshSurface }"#);
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].item, "MeshSurface");
    assert_eq!(imports[0].module_path, &["surfaces", "mesh"]);
}

// ── mixed separators ──────────────────────────────────────────────────────────

#[test]
fn mixed_dot_and_slash_in_same_path() {
    // `import surfaces/mesh.MeshSurface` — unusual but should parse as two segments + item.
    let imports = parse_imports("import surfaces/mesh.MeshSurface");
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].item, "MeshSurface");
    assert_eq!(imports[0].module_path, &["surfaces", "mesh"]);
}

// ── coexistence with legacy dotted form ───────────────────────────────────────

#[test]
fn legacy_dotted_form_unchanged() {
    let imports = parse_imports("import react.use_state");
    assert_eq!(imports.len(), 1);
    // `react` is the module, `use_state` is the item.
    assert_eq!(imports[0].module_path, &["react"]);
    assert_eq!(imports[0].item, "use_state");
    assert!(imports[0].es_module_specifier.is_none());
}

// ── Phase 5 React `.tsx` bridge ───────────────────────────────────────────────

#[test]
fn react_component_import_lowers_with_es_specifier() {
    let imports = parse_imports(r#"import react MyButton from "../ui/MyButton.tsx""#);
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].item, "MyButton");
    assert_eq!(imports[0].module_path, Vec::<String>::new());
    assert_eq!(
        imports[0].es_module_specifier.as_deref(),
        Some("../ui/MyButton.tsx")
    );
}

#[test]
fn react_component_import_coexists_with_react_dot_import() {
    let src = r#"
import react.use_state
import react Sheet from "./Sheet.tsx"
"#;
    let imports = parse_imports(src);
    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0].module_path, &["react"]);
    assert_eq!(imports[0].item, "use_state");
    assert_eq!(imports[1].item, "Sheet");
    assert_eq!(
        imports[1].es_module_specifier.as_deref(),
        Some("./Sheet.tsx")
    );
}

// ── full app.vox import block ─────────────────────────────────────────────────

#[test]
fn app_vox_import_block_parses_cleanly() {
    // Mirrors the real app.vox import section to prevent regression.
    let src = r#"
import lib/chrome as { TopBar, LeftRail, StatusBar }
import surfaces/speak as { SpeakSurface }
import surfaces/mesh as { MeshSurface }
import surfaces/forge as { ForgeSurface }
import surfaces/code as { CodeSurface }
import surfaces/models as { ModelsSurface }
import surfaces/runs as { RunsSurface }
import surfaces/settings as { SettingsSurface }
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap_or_else(|e| panic!("app.vox import block must parse: {e:?}"));
    let hir = lower_module(&module);
    // 3 chrome items + 7 surface items = 10 total imports
    assert_eq!(
        hir.imports.len(),
        10,
        "expected 10 HirImports from the app.vox import block, got {}",
        hir.imports.len()
    );
}
