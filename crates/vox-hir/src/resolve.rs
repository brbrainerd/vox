use std::collections::HashMap;
use crate::hir::*;

/// Resolves cross-module imports by matching `HirImport` entries against available symbols.
///
/// Returns a mapping from (source_module, import_index) to (target_module, target_id).
/// If a symbol is not found, it contributes to diagnostics.
pub fn resolve_imports(modules: &[HirModule]) -> HashMap<(String, usize), (String, DefId)> {
    let mut resolved = HashMap::new();
    let mut module_symbols = HashMap::new();

    // 1. Collect all exported symbols from all modules
    for module in modules {
        let mut symbols = HashMap::new();

        for f in &module.functions { symbols.insert(f.name.clone(), f.id); }
        for t in &module.types { symbols.insert(t.name.clone(), t.id); }
        for c in &module.consts { symbols.insert(c.name.clone(), c.id); }
        for a in &module.actors { symbols.insert(a.name.clone(), a.id); }
        for w in &module.workflows { symbols.insert(w.name.clone(), w.id); }
        for sf in &module.server_fns { symbols.insert(sf.name.clone(), sf.id); }
        for tbl in &module.tables { symbols.insert(tbl.name.clone(), tbl.id); }
        for tr in &module.traits { symbols.insert(tr.name.clone(), tr.id); }
        for msg in &module.messages { symbols.insert(msg.name.clone(), msg.id); }

        module_symbols.insert(module.name.clone(), symbols);
    }

    // 2. Resolve imports
    for module in modules {
        for (i, import) in module.imports.iter().enumerate() {
            if import.module_path.is_empty() {
                continue; // Local or built-in, handled elsewhere for now
            }

            // Assume the first segment is the module name for now
            let target_mod_name = &import.module_path[0];
            if let Some(targets) = module_symbols.get(target_mod_name) {
                if let Some(def_id) = targets.get(&import.item) {
                    resolved.insert((module.name.clone(), i), (target_mod_name.clone(), *def_id));
                }
            }
        }
    }

    resolved
}

/// Resolves imports and populates the `resolved_imports` field of each module.
pub fn resolve_imports_in_place(modules: &mut [HirModule]) {
    let mut module_symbols = HashMap::new();

    // 1. Collect all exported symbols
    for module in modules.iter() {
        let mut symbols = HashMap::new();
        for f in &module.functions { symbols.insert(f.name.clone(), f.id); }
        for t in &module.types { symbols.insert(t.name.clone(), t.id); }
        for c in &module.consts { symbols.insert(c.name.clone(), c.id); }
        for a in &module.actors { symbols.insert(a.name.clone(), a.id); }
        for w in &module.workflows { symbols.insert(w.name.clone(), w.id); }
        for sf in &module.server_fns { symbols.insert(sf.name.clone(), sf.id); }
        for tbl in &module.tables { symbols.insert(tbl.name.clone(), tbl.id); }
        for tr in &module.traits { symbols.insert(tr.name.clone(), tr.id); }
        for msg in &module.messages { symbols.insert(msg.name.clone(), msg.id); }
        module_symbols.insert(module.name.clone(), symbols);
    }

    // 2. Resolve entries
    for module in modules.iter_mut() {
        let mut resolved = HashMap::new();
        for (i, import) in module.imports.iter().enumerate() {
            if import.module_path.is_empty() { continue; }
            let target_mod_name = &import.module_path[0];
            if let Some(targets) = module_symbols.get(target_mod_name) {
                if let Some(def_id) = targets.get(&import.item) {
                    resolved.insert(i, (target_mod_name.clone(), *def_id));
                }
            }
        }
        module.resolved_imports = resolved;
    }
}
