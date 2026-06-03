//! One-time user-owned **config** files (never overwrite if present).
//!
//! These are project toolchain files (Vite, Tailwind, tsconfig, package.json)
//! that a host project may want as a starting point. The app *bootstrap* itself
//! — `entry.tsx` / `vox-app.tsx` / `app-hooks.tsx` / error boundary / SW — is
//! **emitted on every build** by [`super::web_entry`], so `main.tsx` / `App.tsx`
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
            "@import \"tailwindcss\";\n".to_string(),
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
}
"#
            .to_string(),
        ),
    ]
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
