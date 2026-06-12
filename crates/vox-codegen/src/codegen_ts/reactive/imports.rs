use vox_compiler::hir::*;

/// Emit the external-React/TS import statements for a module's `import react …`
/// declarations (Phase 5). Default and namespace imports become one line each;
/// named imports are grouped per module specifier. Output is fully ordered
/// (lines sorted, members sorted) for byte-deterministic codegen. Shared by the
/// web ([`generate_reactive_component`]) and React-Native component emitters so
/// both targets agree on the import surface.
pub fn emit_react_es_import_lines(imports: &[HirImport]) -> String {
    use std::collections::BTreeMap;
    let mut lines: Vec<String> = Vec::new();
    // Named imports grouped per specifier: spec -> (local -> imported).
    let mut named: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
    for imp in imports {
        let (Some(spec), Some(kind)) = (
            imp.es_module_specifier.as_deref(),
            imp.es_import_kind.as_ref(),
        ) else {
            continue;
        };
        match kind {
            EsImportKind::Default => {
                lines.push(format!("import {} from \"{spec}\";", imp.item));
            }
            EsImportKind::Namespace => {
                lines.push(format!("import * as {} from \"{spec}\";", imp.item));
            }
            EsImportKind::Named { imported } => {
                named
                    .entry(spec)
                    .or_default()
                    .insert(imp.item.as_str(), imported.as_str());
            }
        }
    }
    for (spec, members) in &named {
        let parts: Vec<String> = members
            .iter()
            .map(|(local, imported)| {
                if local == imported {
                    (*local).to_string()
                } else {
                    format!("{imported} as {local}")
                }
            })
            .collect();
        lines.push(format!(
            "import {{ {} }} from \"{spec}\";",
            parts.join(", ")
        ));
    }
    lines.sort();
    lines.dedup();
    let mut out = String::new();
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// Emit support lines for known external libraries referenced by `import react …`
/// (Phase 5 SSOT, see [`crate::codegen_ts::external_libs`]): required CSS-file
/// imports (e.g. Mantine `@mantine/core/styles.css`, which is mandatory and not
/// runtime-injected) plus one-line setup guidance for mandatory providers
/// (Chakra/Mantine/Paper/Tamagui). `target_is_rn` filters web-only vs RN-only
/// libraries. Deterministic order. Shared by the web and RN component emitters.
pub fn emit_external_lib_support(imports: &[HirImport], target_is_rn: bool) -> String {
    use crate::codegen_ts::external_libs::{lookup, valid_for_target};
    use std::collections::BTreeSet;
    let mut css: BTreeSet<&str> = BTreeSet::new();
    let mut guidance: BTreeSet<String> = BTreeSet::new();
    for imp in imports {
        let Some(spec) = imp.es_module_specifier.as_deref() else {
            continue;
        };
        let Some(lib) = lookup(spec) else { continue };
        if !valid_for_target(lib, target_is_rn) {
            continue;
        }
        for c in lib.css_imports {
            css.insert(*c);
        }
        if let (Some(p), true) = (lib.provider, lib.provider_mandatory) {
            guidance.insert(format!(
                "// vox-interop: \"{}\" requires <{p}> mounted at your app root.",
                lib.package
            ));
        }
    }
    let mut out = String::new();
    for c in &css {
        out.push_str(&format!("import \"{c}\";\n"));
    }
    for g in &guidance {
        out.push_str(g);
        out.push('\n');
    }
    out
}

