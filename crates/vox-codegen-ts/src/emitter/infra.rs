
pub fn generate_tsconfig() -> String {
    r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "outDir": "dist",
    "rootDir": "src",
    "baseUrl": ".",
    "paths": {
      "@vox/ui": ["./packages/vox-ui/src/index.tsx"]
    }
  },
  "include": ["src", "tests"],
  "exclude": ["node_modules", "dist"]
}
"#
    .to_string()
}

pub fn generate_vitest_config() -> String {
    r#"import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./tests/setup.ts"],
  },
});
"#
    .to_string()
}

pub fn generate_package_json(
    all_ts: &str,
    has_vitest_tests: bool,
    _has_routes: bool,
) -> String {
    let has_convex = all_ts.contains("convex/react") || all_ts.contains("from \"convex\"");
    let has_react_router = all_ts.contains("react-router-dom");
    let has_react_markdown = all_ts.contains("react-markdown") || all_ts.contains("ReactMarkdown");
    let has_recharts = all_ts.contains("recharts");
    let has_radix = all_ts.contains("@radix-ui");
    let has_lucide = all_ts.contains("lucide-react");

    let mut deps: Vec<(&str, &str)> = vec![
        ("react", "^19.0.0"),
        ("react-dom", "^19.0.0"),
    ];
    if has_react_router { deps.push(("react-router-dom", "^7.0.0")); }
    if has_convex { deps.push(("convex", "^1.17.0")); }
    if has_react_markdown { deps.push(("react-markdown", "^9.0.0")); }
    if has_recharts { deps.push(("recharts", "^2.14.0")); }
    if has_radix {
        deps.push(("@radix-ui/react-checkbox", "^1.1.0"));
        deps.push(("@radix-ui/react-select", "^2.1.0"));
        deps.push(("@radix-ui/react-slider", "^1.2.0"));
        deps.push(("@radix-ui/react-switch", "^1.1.0"));
    }
    if has_lucide { deps.push(("lucide-react", "^0.471.0")); }

    let mut dev_deps: Vec<(&str, &str)> = vec![
        ("@types/react", "^19.0.0"),
        ("@types/react-dom", "^19.0.0"),
        ("typescript", "^5.7.0"),
        ("vite", "^6.1.0"),
        ("@vitejs/plugin-react", "^4.3.0"),
    ];
    if has_vitest_tests {
        dev_deps.push(("vitest", "^3.0.0"));
        dev_deps.push(("@vitest/ui", "^3.0.0"));
        dev_deps.push(("jsdom", "^26.0.0"));
        dev_deps.push(("@testing-library/react", "^16.0.0"));
        dev_deps.push(("@testing-library/user-event", "^14.6.0"));
    }

    let deps_str = deps.iter().map(|(k, v)| format!("    \"{}\": \"{}\"", k, v)).collect::<Vec<_>>().join(",\n");
    let dev_deps_str = dev_deps.iter().map(|(k, v)| format!("    \"{}\": \"{}\"", k, v)).collect::<Vec<_>>().join(",\n");

    format!(r#"{{
  "name": "vox-app",
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:ui": "vitest --ui",
    "type-check": "tsc --noEmit"
  }},
  "dependencies": {{
{}
  }},
  "devDependencies": {{
{}
  }}
}}
"#, deps_str, dev_deps_str)
}

pub fn generate_dockerfile() -> String {
    vox_container::generate::generate_default_dockerfile()
}

pub fn generate_docker_compose() -> String {
    vox_container::generate::generate_compose_file()
}

pub fn generate_env_example(all_ts: &str) -> String {
    use std::collections::BTreeSet;
    let mut vars: BTreeSet<String> = BTreeSet::new();
    // Always include base vars
    vars.insert("NODE_ENV=development".to_string());
    vars.insert("VOX_PORT=3001".to_string());
    // Scan generated TS for process.env.X references
    let mut remaining = all_ts;
    while let Some(idx) = remaining.find("process.env.") {
        remaining = &remaining[idx + 12..];
        let name: String = remaining.chars()
            .take_while(|c| c.is_ascii_uppercase() || *c == '_' || c.is_ascii_digit())
            .collect();
        if !name.is_empty() {
            // Provide sensible placeholder comments for known vars
            let placeholder = match name.as_str() {
                "OPENROUTER_API_KEY" => "OPENROUTER_API_KEY=",
                "ANTHROPIC_API_KEY" => "ANTHROPIC_API_KEY=",
                "OPENAI_API_KEY" => "OPENAI_API_KEY=",
                "CONVEX_DEPLOYMENT" => "CONVEX_DEPLOYMENT=dev:your-deployment-name",
                "CLERK_SECRET_KEY" => "CLERK_SECRET_KEY=",
                "CLERK_PUBLISHABLE_KEY" => "CLERK_PUBLISHABLE_KEY=",
                "DATABASE_URL" => "DATABASE_URL=libsql://your-db.turso.io",
                "TURSO_AUTH_TOKEN" => "TURSO_AUTH_TOKEN=",
                "JWT_SECRET" => "JWT_SECRET=changeme",
                "VOX_API_KEY" => "VOX_API_KEY=",
                "VOX_PORT" => continue,
                _ => {
                    vars.insert(format!("{}=", name));
                    continue;
                }
            };
            vars.insert(placeholder.to_string());
        }
    }
    vars.into_iter().collect::<Vec<_>>().join("\n") + "\n"
}
