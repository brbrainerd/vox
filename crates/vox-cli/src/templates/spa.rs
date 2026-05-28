//! Vite + React SPA scaffold snippets (`index.html`, `package.json`, …).

/// Minimal **shadcn/ui**-compatible `components.json` (**`rsc`: false**) for Vite scaffolds.
/// Pair with `tsconfig_json` path alias `@/*` → `./src/*` and optional `lucide-react` in `package_json`.
pub fn components_json_shadcn_client() -> &'static str {
    r#"{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/index.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
"#
}

/// Shared `@tanstack/react-router` semver range (SPA + Start `package_json`).
pub const TANSTACK_REACT_ROUTER_RANGE: &str = "^1.120.0";
/// `@tanstack/react-query` semver range (generated `vox-tanstack-query.tsx`).
pub const TANSTACK_REACT_QUERY_RANGE: &str = "^5.62.0";
/// `@tanstack/react-start` semver range (TanStack Start scaffold only).
pub const TANSTACK_REACT_START_RANGE: &str = "^1.120.0";
/// `@tanstack/router-cli` semver range (`tsr generate` / `pnpm run routes:gen`).
pub const TANSTACK_ROUTER_CLI_RANGE: &str = "^1.120.0";

/// Default `index.html` for a Vite + React scaffold.
pub fn index_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />
  <meta name="color-scheme" content="dark light" />
  <title>Vox App</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet">
</head>
<body>
  <div id="root"></div>
  <script type="module" src="/src/main.tsx"></script>
</body>
</html>
"#
}

/// `main.tsx` entry that mounts the generated React component.
/// SPA entry when `routes.manifest.ts` is present: TanStack Router driven by `voxRoutes`.
pub fn main_tsx_manifest_entry() -> &'static str {
    r#"import React from "react";
import ReactDOM from "react-dom/client";
import { VoxManifestApp } from "./vox-manifest-router";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <VoxManifestApp />
  </React.StrictMode>
);
"#
}

/// Shared helpers to turn `voxRoutes` into TanStack `createRoute` trees (SPA + Start scaffolds).
pub fn vox_manifest_route_adapter_tsx() -> &'static str {
    r#"import { createRoute } from "@tanstack/react-router";
import type { VoxRoute } from "./generated/routes.manifest";

/** TanStack uses `$param` segments; Vox manifest uses `:param`. */
function voxPathToChildPath(path: string): string {
  const p = path.trim();
  if (p === "/" || p === "") return "/";
  const rest = p.startsWith("/") ? p.slice(1) : p;
  const segs = rest.split("/").filter(Boolean);
  const mapped = segs.map((s) =>
    s.startsWith(":") ? `$${s.slice(1).replace(/\?$/, "")}` : s,
  );
  return mapped.join("/");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function buildChildRoutes(parent: any, nodes: VoxRoute[]): any[] {
  return nodes.map((r) => {
    const path = r.path.trim() === "/" ? "/" : voxPathToChildPath(r.path);
    const route = createRoute({
      getParentRoute: () => parent,
      path,
      component: r.component,
      loader: r.loader,
      pendingComponent: r.pendingComponent,
      errorComponent: r.errorComponent,
      ...(r.index ? { index: true } : {}),
    } as never);
    if (r.children?.length) {
      return route.addChildren(buildChildRoutes(route, r.children));
    }
    return route;
  });
}
"#
}

/// Programmatic route tree from compiler `routes.manifest.ts` (Vite SPA shell).
pub fn vox_spa_manifest_router_tsx() -> &'static str {
    r#"import {
  RouterProvider,
  Outlet,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { voxRoutes } from "./generated/routes.manifest";
import { VoxQueryProvider } from "./generated/vox-tanstack-query";
import { buildChildRoutes } from "./vox-manifest-route-adapter";

const rootRoute = createRootRoute({
  component: () => (
    <VoxQueryProvider>
      <Outlet />
    </VoxQueryProvider>
  ),
});

const routeTree = rootRoute.addChildren(buildChildRoutes(rootRoute, voxRoutes));

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export function VoxManifestApp() {
  return <RouterProvider router={router} />;
}
"#
}

pub fn main_tsx(component_name: &str) -> String {
    format!(
        r#"import React from "react";
import ReactDOM from "react-dom/client";
import {{ VoxQueryProvider }} from "./generated/vox-tanstack-query";
import {{ {component_name} }} from "./generated/{component_name}";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <VoxQueryProvider>
      <{component_name} />
    </VoxQueryProvider>
  </React.StrictMode>
);
"#
    )
}

/// Base stylesheet for the generated dark-theme app shell.
pub fn index_css() -> &'static str {
    r#"/* Vox Generated App — Dark Theme Design System */
:root {
  --bg-primary: #0f1117;
  --bg-secondary: #1a1d27;
  --bg-tertiary: #252836;
  --bg-accent: #2d3142;
  --text-primary: #e8eaf0;
  --text-secondary: #9ca3b4;
  --text-muted: #6b7280;
  --accent: #6366f1;
  --accent-hover: #818cf8;
  --accent-glow: rgba(99, 102, 241, 0.25);
  --success: #34d399;
  --error: #f87171;
  --border: #2e3244;
  --border-focus: #6366f1;
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --shadow-sm: 0 1px 3px rgba(0,0,0,0.3);
  --shadow-md: 0 4px 12px rgba(0,0,0,0.4);
  --shadow-lg: 0 8px 32px rgba(0,0,0,0.5);
  --font: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --transition: 200ms cubic-bezier(0.4, 0, 0.2, 1);
}

*, *::before, *::after {
  margin: 0; padding: 0; box-sizing: border-box;
}

html, body, #root {
  height: 100%; width: 100%;
  font-family: var(--font);
  background: var(--bg-primary);
  color: var(--text-primary);
  -webkit-font-smoothing: antialiased;
  text-size-adjust: 100%;
  -webkit-text-size-adjust: 100%;
}

/* Mobile baseline: keep tap targets and spacing usable by default. */
button,
[role="button"],
input[type="button"],
input[type="submit"],
input[type="reset"],
a.button {
  min-height: 44px;
  min-width: 44px;
}

input,
select,
textarea,
button {
  font: inherit;
}
"#
}

/// `package.json` with React, Vite, and TypeScript dev dependencies.
///
/// When `tanstack_start` is true, adds **`@tanstack/react-start`** and uses `vite dev` / `vite build`
/// ([TanStack Start](https://tanstack.com/start/latest/docs/framework/react/build-from-scratch)).
///
/// When `file_route_tsr_pregen` is true (file-based `src/routes/*` without programmatic `VoxTanStackRouter.tsx`),
/// `dev` / `build` run **`pnpm run routes:gen`** first so `routeTree.gen.ts` stays in sync.
pub fn package_json(tanstack_start: bool, file_route_tsr_pregen: bool) -> String {
    if tanstack_start {
        let (dev_cmd, build_cmd) = if file_route_tsr_pregen {
            (
                "pnpm run routes:gen && vite dev",
                "pnpm run routes:gen && vite build",
            )
        } else {
            ("vite dev", "vite build")
        };
        return format!(
            r#"{{
  "name": "vox-generated-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "{dev_cmd}",
    "build": "{build_cmd}",
    "preview": "vite preview",
    "dev:ssr-upstream": "vite --port 3001 --strictPort",
    "routes:gen": "tsr generate"
  }},
  "dependencies": {{
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "lucide-react": "^0.468.0",
    "@tanstack/react-router": "{tr}",
    "@tanstack/react-start": "{ts}",
    "@tanstack/react-query": "{rq}",
    "@tauri-apps/api": "^2.0.0",
    "zod": "^3.23.0"
  }},
  "devDependencies": {{
    "@types/node": "^22.0.0",
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "@tanstack/router-cli": "{tcli}",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }}
}}
"#,
            dev_cmd = dev_cmd,
            build_cmd = build_cmd,
            tr = TANSTACK_REACT_ROUTER_RANGE,
            ts = TANSTACK_REACT_START_RANGE,
            tcli = TANSTACK_ROUTER_CLI_RANGE,
            rq = TANSTACK_REACT_QUERY_RANGE
        );
    }
    format!(
        r#"{{
  "name": "vox-generated-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "dev:ssr-upstream": "vite --port 3001 --strictPort"
  }},
  "dependencies": {{
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "lucide-react": "^0.468.0",
    "@tanstack/react-router": "{tr}",
    "@tanstack/react-query": "{rq}",
    "@tauri-apps/api": "^2.0.0",
    "zod": "^3.23.0"
  }},
  "devDependencies": {{
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }}
}}
"#,
        tr = TANSTACK_REACT_ROUTER_RANGE,
        rq = TANSTACK_REACT_QUERY_RANGE
    )
}

/// Vite config with `/api` proxy to the given backend port.
pub fn vite_config(backend_port: u16, tanstack_start: bool) -> String {
    if tanstack_start {
        return format!(
            r#"import path from "node:path";
import {{ fileURLToPath }} from "node:url";
import {{ defineConfig }} from "vite";
import {{ tanstackStart }} from "@tanstack/react-start/plugin/vite";
import react from "@vitejs/plugin-react";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({{
  resolve: {{
    alias: {{
      "@": path.resolve(__dirname, "./src"),
    }},
  }},
  plugins: [
    tanstackStart(),
    react(),
  ],
  server: {{
    proxy: {{
      "/api": {{
        target: "http://127.0.0.1:{backend_port}",
        changeOrigin: true,
      }},
    }},
  }},
  build: {{
    outDir: "dist",
  }},
}});
"#
        );
    }
    format!(
        r#"import path from "node:path";
import {{ fileURLToPath }} from "node:url";
import {{ defineConfig }} from "vite";
import react from "@vitejs/plugin-react";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({{
  resolve: {{
    alias: {{
      "@": path.resolve(__dirname, "./src"),
    }},
  }},
  plugins: [react()],
  server: {{
    proxy: {{
      "/api": {{
        target: "http://127.0.0.1:{backend_port}",
        changeOrigin: true,
      }},
    }},
  }},
  build: {{
    outDir: "dist",
  }},
}});
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_includes_mobile_viewport_contract() {
        let html = index_html();
        assert!(html.contains("name=\"viewport\""));
        assert!(html.contains("width=device-width, initial-scale=1.0, viewport-fit=cover"));
        assert!(html.contains("name=\"color-scheme\""));
    }

    #[test]
    fn index_css_includes_mobile_tap_target_baseline() {
        let css = index_css();
        assert!(css.contains("Mobile baseline"));
        assert!(css.contains("min-height: 44px;"));
        assert!(css.contains("min-width: 44px;"));
    }

    #[test]
    fn vite_config_includes_resolve_alias_at_to_src() {
        let spa = vite_config(4000, false);
        let start = vite_config(4000, true);
        for cfg in [spa, start] {
            assert!(
                cfg.contains("resolve:") && cfg.contains("alias:"),
                "expected resolve.alias: {cfg}"
            );
            assert!(cfg.contains(r#""@": path.resolve(__dirname, "./src")"#));
            assert!(cfg.contains("node:path"));
            assert!(cfg.contains("fileURLToPath"));
        }
    }

    #[test]
    fn manifest_route_adapter_exports_build_child_routes() {
        let ad = vox_manifest_route_adapter_tsx();
        assert!(ad.contains("export function buildChildRoutes"));
        assert!(ad.contains("from \"./generated/routes.manifest\""));
    }

    #[test]
    fn spa_manifest_router_imports_shared_adapter() {
        let r = vox_spa_manifest_router_tsx();
        assert!(r.contains("from \"./vox-manifest-route-adapter\""));
        assert!(r.contains("buildChildRoutes"));
    }

    #[test]
    fn components_json_shadcn_client_disables_rsc() {
        let j = components_json_shadcn_client();
        assert!(j.contains("\"rsc\": false"), "{j}");
    }
}
