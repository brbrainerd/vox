//! One-time user-owned **config** files (never overwrite if present).
//!
//! These are project toolchain files (Vite, Tailwind, tsconfig, package.json)
//! that a host project may want as a starting point. The app *bootstrap* itself
//! — `entry.tsx` / `vox-app.tsx` / `app-hooks.tsx` / error boundary / SW — is
//! **emitted on every build** by `web_entry`, so `main.tsx` / `App.tsx`
//! are no longer scaffolded (they would shadow the generated router).

use std::path::Path;


/// Files relative to project root (`app/`, `vite.config.ts`, etc.).
pub type ScaffoldFile = (String, String);

/// One-shot toolchain config files written next to the build output when
/// `--emit-config` (alias `--scaffold`) is set. Config only — no bootstrap.
#[must_use]
pub fn web_config_files(_project_name: &str) -> Vec<ScaffoldFile> {
    vec![
        (
            "app/globals.css".to_string(),
            format!(
                "@import \"tailwindcss\";\n\n{}",
                crate::web_ir::layer_emit::emit_layer_stylesheet()
            ),
        ),
        (
            "app/vox-layer-roots.tsx".to_string(),
            crate::web_ir::layer_emit::emit_layer_portal_roots(),
        ),
        (
            "app/vox-layer-resolver.ts".to_string(),
            crate::web_ir::layer_emit::emit_layer_portal_resolver(),
        ),
        (
            "app/vox-layer-types.ts".to_string(),
            crate::web_ir::layer_emit::emit_layer_type_alias(),
        ),
        (
            "app/components.json".to_string(),
            r#"{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tailwind": {
    "config": "",
    "css": "app/globals.css",
    "baseColor": "slate",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui"
  }
}
"#
            .to_string(),
        ),
        (
            "vite.config.ts".to_string(),
            r#"import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import path from "path"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "app") },
    // Force a single React copy so imported external component libraries
    // (MUI, Radix, etc.) and the app share one react/react-dom — duplicates
    // cause React's "Invalid hook call" runtime error.
    dedupe: ["react", "react-dom"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: process.env.VITE_API_URL ?? "http://127.0.0.1:4000",
        changeOrigin: true,
      },
    },
  },
})
"#
            .to_string(),
        ),
        (
            "tsconfig.json".to_string(),
            r#"{
  "compilerOptions": {
    "jsx": "react-jsx",
    "moduleResolution": "Bundler",
    "module": "ESNext",
    "target": "ES2022",
    "skipLibCheck": true,
    "strictNullChecks": true,
    "paths": { "@/*": ["./app/*"] }
  },
  "include": ["app", "dist"]
}
"#
            .to_string(),
        ),
        (
            "package.json".to_string(),
            static_package_json().to_string(),
        ),
    ]
}

/// Inject extra npm package names into the static `package.json` template.
#[cfg(feature = "standalone")]
///
/// `extra_packages` is a list of bare npm specifiers (e.g. `@radix-ui/react-dialog`).
/// Each is inserted into the `dependencies` block with a `"*"` version placeholder,
/// suitable for `vox build` to refine later.
#[must_use]
pub fn package_json_with_extra_deps(extra_packages: &[&str]) -> String {
    if extra_packages.is_empty() {
        return static_package_json().to_string();
    }
    let extra: String = extra_packages
        .iter()
        .map(|pkg| format!("    \"{pkg}\": \"*\""))
        .collect::<Vec<_>>()
        .join(",\n");
    // Inject before the closing `}` of the `dependencies` block.
    static_package_json().replacen(
        "    \"lucide-react\": \"^0.400.0\"\n  },",
        &format!("    \"lucide-react\": \"^0.400.0\",\n{extra}\n  }},"),
        1,
    )
}

/// Collect the set of extra npm packages (and their peers) implied by
/// the `es_module_specifier` fields in `imports`.
#[cfg(feature = "standalone")]
#[must_use]
pub fn extra_deps_from_imports(
    imports: &[vox_compiler::hir::HirImport],
) -> Vec<String> {
    use crate::external_libs::{bare_package, LIBRARIES};
    let mut pkgs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for imp in imports {
        let Some(spec) = &imp.es_module_specifier else {
            continue;
        };
        let Some(pkg) = bare_package(spec) else {
            continue;
        };
        // Skip packages already in the static template.
        if matches!(pkg, "react" | "react-dom" | "lucide-react") {
            continue;
        }
        pkgs.insert(pkg.to_string());
        // Add declared peers from the LIBRARIES table.
        if let Some(lib) = LIBRARIES.iter().find(|l| l.package == pkg) {
            for peer in lib.peers {
                pkgs.insert(peer.to_string());
            }
        }
    }
    pkgs.into_iter().collect()
}

/// Return the static package.json template (shared between production and tests).
fn static_package_json() -> &'static str {
    r#"{
  "name": "vox-app",
  "type": "module",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "lucide-react": "^0.400.0"
  },
  "devDependencies": {
    "@tailwindcss/vite": "^4.0.0",
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "tailwindcss": "^4.0.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0"
  }
}"#
}

/// Parse `src` as a Vox module and return the `package.json` content that
/// would be scaffolded for that project, including any imported npm packages.
///
/// Intended for integration tests.
#[cfg(feature = "standalone")]
#[must_use]
pub fn package_json_for_test(src: &str) -> String {
    use vox_compiler::{hir::lower_module, lexer::lex, parser::parse};
    let tokens = lex(src);
    let ast = match parse(tokens) {
        Ok(m) => m,
        Err(_) => return static_package_json().to_string(),
    };
    let hir = lower_module(&ast);
    let extras = extra_deps_from_imports(&hir.imports);
    let extra_refs: Vec<&str> = extras.iter().map(String::as_str).collect();
    package_json_with_extra_deps(&extra_refs)
}

/// Write one-shot config files under `project_root` if missing.
pub fn write_scaffold_if_missing(project_root: &Path, project_name: &str) -> std::io::Result<()> {
    for (rel, content) in web_config_files(project_name) {
        let path = project_root.join(&rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GA-26 criterion 6: the generated `globals.css` embeds exactly the seven
    /// layered `[data-vox-layer="…"]` selectors with the fixed z-index ladder.
    #[test]
    fn globals_css_embeds_seven_tier_ladder() {
        let files = web_config_files("vox-app");
        let (_, css) = files
            .iter()
            .find(|(rel, _)| rel == "app/globals.css")
            .expect("globals.css scaffold file present");
        for tier in [
            "background",
            "content",
            "chrome",
            "popover",
            "modal",
            "toast",
            "system-overlay",
        ] {
            assert!(
                css.contains(&format!("[data-vox-layer=\"{tier}\"]")),
                "globals.css missing tier selector {tier}"
            );
        }
        // Tailwind import must still be present.
        assert!(css.contains("@import \"tailwindcss\""));
    }

    /// The portal-root + resolver + type-alias TS files ship so overlays have a
    /// mount target (and the semantic_ui Dialog import resolves).
    #[test]
    fn scaffold_ships_layer_portal_runtime() {
        let files = web_config_files("vox-app");
        let names: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
        for expected in [
            "app/vox-layer-roots.tsx",
            "app/vox-layer-resolver.ts",
            "app/vox-layer-types.ts",
        ] {
            assert!(names.contains(&expected), "scaffold missing {expected}");
        }
    }

    /// The emitted `vox-app.tsx` ships a dependency-free router, and
    /// `web_entry.rs` asserts no react-router import. The scaffold's
    /// `package.json` must therefore NOT pull in `react-router`.
    #[test]
    fn package_json_has_no_react_router() {
        let files = web_config_files("vox-app");
        let (_, pkg) = files
            .iter()
            .find(|(rel, _)| rel == "package.json")
            .expect("package.json scaffold file present");
        let json: serde_json::Value =
            serde_json::from_str(pkg).expect("scaffold package.json must be valid JSON");
        for section in ["dependencies", "devDependencies"] {
            if let Some(deps) = json.get(section).and_then(|v| v.as_object()) {
                assert!(
                    !deps.contains_key("react-router"),
                    "scaffold package.json `{section}` must not depend on react-router; emitted router is dependency-free"
                );
            }
        }
    }
}
