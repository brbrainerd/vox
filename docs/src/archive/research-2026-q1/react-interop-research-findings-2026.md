---
title: "Vox React/v0 Interop Research Findings"
description: "Comprehensive research findings (20+ web searches) on the React ecosystem, v0.dev anatomy, framework landscape, stable API surfaces, and what features Vox must actually support to achieve a maintainable 90-95% frontend shell. This is the research foundation for the Minimal React Interop strategy."
category: "Architecture SSOTs"
last_updated: "2026-04-07"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox React / v0 Interop: Research Findings

**Purpose:** Ground the "Minimal Shell" strategy in actual facts about what the React ecosystem, v0.dev, and modern framework conventions require—and what Vox can safely ignore. This replaces speculative assumptions.

---

## 1. v0.dev Anatomy: What It Actually Emits

### How v0.dev Delivers Code

v0.dev has two delivery mechanisms:
1. **"Add to Codebase" button** → generates a one-time `npx` command you run locally
2. **Direct copy-paste** → copy the component TSX from the editor

The generated `npx` command resolves to the **shadcn/cli v4** (`npx shadcn@latest add [URL]`). As of March 2026, shadcn/cli v4 introduces presets, `--dry-run`, `--diff`, and `--view` flags for safe inspection before writing.

### File Structure v0.dev Creates

When you use v0 to scaffold a full project (via "Add to Codebase" for a page or layout), files land at:

```
components/
  ui/              ← shadcn base primitives (Button, Card, Dialog, etc.)
  [YourBlock].tsx  ← the specific generated component

app/
  page.tsx         ← only if Next.js App Router is detected
  layout.tsx

lib/
  utils.ts         ← `cn()` class-merging utility (clsx + tailwind-merge)

components.json    ← shadcn registry configuration
tailwind.config.ts ← updated with any new theme tokens
```

### What v0 Output Actually Looks Like

A typical v0 component:

```tsx
// vox:skip
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

export function LoginForm() {
  return (
    <Card className="w-[350px]">
      <CardHeader>
        <CardTitle>Sign In</CardTitle>
      </CardHeader>
      <CardContent>
        <Input placeholder="Email" type="email" />
        <Button className="w-full mt-4">Sign In</Button>
      </CardContent>
    </Card>
  )
}
```

**Critical observations:**
- Always **named exports** (not default exports). This is a hard contract.
- Uses `@/components/ui/*` path alias — standard shadcn import path.
- Uses `className` (React JSX attribute, not `class`).
- Tailwind utility classes are the only styling mechanism.
- Imports from `lucide-react` for icons.
- Components compose shadcn primitives; they do NOT import from any routing library or framework.
- **No routing, no data fetching, no server functions** — pure presentational components.

### The `components.json` Contract

The `components.json` file is what shadcn/cli uses to understand where to put files. Key fields:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tailwind": {
    "config": "tailwind.config.ts",
    "css": "src/globals.css",
    "baseColor": "slate",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils"
  }
}
```

The `rsc: false` field is critical — when `true`, v0 can emit `"use client"` directives. When `false`, it emits plain client-side React. **Vox should set `rsc: false`** to keep output framework-agnostic.

archived_date: 2026-04-18
---

## 2. The Stable React API Surface (What Will Not Change)

Research confirms React maintains extremely strong backward compatibility for stable features. Since 16.8 (2019), the following have **never had a breaking API change**:

### Stable Forever (Safe to Target)
- **Functional components** — the fundamental authoring model
- **JSX** syntax — `<Component prop="value">` is bedrock
- **`useState`, `useEffect`, `useContext`, `useRef`, `useMemo`, `useCallback`** — stable since 16.8
- **Named exports** — React itself recommends named exports for libraries
- **Context API** (`createContext`, `useContext`, `Provider`) — stable
- **`React.FC<Props>` / typed function components** — stable TypeScript pattern
- **`children` prop** — fundamental to composition

### Unstable / Volatile (Do NOT Generate These)
- **`"use server"` / `"use client"` directives** — RSC-specific, Next.js-specific
- **`createServerFn`** — TanStack Start specific, v1 API
- **File-based routing conventions** — change with every major version of every framework
- **`loader` / `action` functions** — Remix/RR7-specific
- **`getServerSideProps`, `getStaticProps`** — Next.js Pages Router (already being deprecated)
- **`generateMetadata`** — Next.js App Router specific
- **`server.proxy` Vite config shapes** — change with Vite major versions

**Conclusion:** Vox should target the stable forever surface, and emit the volatile wiring only as user-owned scaffold files that Vox generates once and never touches again.

---

## 3. Tailwind CSS: The One Styling Dependency We Must Accept

Tailwind v4 (released 2024, now standard) introduces:
- New engine (Rust-based, fast)
- CSS-first config (`@import "tailwindcss"` and `@theme {}` instead of `tailwind.config.js`)
- Automatic content detection (no `content: []` array needed)
- Some class renames (`bg-gradient-to-*` → `bg-linear-to-*`, `flex-shrink-0` → `shrink-0`)

**For Vox specifically:**
- Vox does NOT generate Tailwind class names — it passes JSX/className strings through from the Vox source verbatim
- The Tailwind configuration itself belongs in user-owned scaffold files (`tailwind.config.ts`, `globals.css`)
- Because v0 uses Tailwind and shadcn, **Vox must ensure the generated scaffold includes proper Tailwind setup** — but Vox itself is Tailwind-agnostic
- The shadcn dependency on Tailwind is a **user-facing requirement**, not a compiler requirement

archived_date: 2026-04-18
---

## 4. shadcn/ui: The Component Distribution Layer

### What shadcn Actually Is

shadcn/ui is NOT an npm package. It is a **code distribution system**: you run `npx shadcn@latest add button` and it copies `button.tsx` source code into your project under `components/ui/`. You own the code permanently.

This is architecturally perfect for Vox because:
- Vox generates components that import from `@/components/ui/*`
- The user runs `npx shadcn@latest add [component]` to install the primitives
- Vox never has to know about or generate the shadcn primitives themselves

### What Vox Must Support for shadcn Compatibility

1. Emit a `components.json` file (scaffold, written once) with correct `aliases`
2. Use `@/components/ui/...` import paths in generated TSX
3. Ensure path aliases (`@/` → `src/`) are configured in `vite.config.ts` (scaffold, written once)
4. Ensure generated files use **named exports** (already the Path C convention)

### The New Shadcn CLI v4 Features (March 2026)

- `--dry-run`, `--diff`, `--view` flags for inspection before install
- **Presets** for instant project configuration
- **Skills** — AI coding agents (Cursor, Copilot, v0) can now load `shadcn/skills` to understand your local registry, drastically reducing hallucinations

This means the future of v0 → Vox interop gets **better over time**, not worse, as AI context improves.

---

## 5. Framework Landscape: What We Actually Need to Track

### The Big Three (and their volatility)

| Framework | What Changes Frequently | What Is Stable |
|-----------|------------------------|----------------|
| **Next.js** | App Router RSC conventions, `page.tsx` file contracts, Metadata API, `"use server"` shape | React components, `fetch` calls, named exports |
| **TanStack Start** | Virtual file routes, `createServerFn` API (v1 is very new), Vinxi internals | React Router's route object shape, `loader` concept |
| **React Router v7** | Framework mode file conventions, `loader`/`action` API shape | Library mode: `<Routes>`, `<Route>`, `useNavigate`, `useParams` |

**The critical insight:** ALL three frameworks import and render **plain React functional components** with **named exports** in exactly the same way. The routing and data-fetching wrappers are what differ — and those wrappers are the volatile parts.

### React Router v7: Library Mode as the Safe Default

React Router v7 has two modes:
- **Library Mode:** You own the setup (Vite + `<RouterProvider>`). This is effectively the old RRv6 API.
- **Framework Mode:** Full-stack (Remix-derived). Opinionated file conventions.

**Library Mode is the correct choice for Vox.** It wraps `<RouterProvider>` from `react-router`, which is incredibly stable. Vox can emit an abstract route manifest and a single `App.tsx` that sets up `<RouterProvider>` from that manifest. This works without framework-specific wiring.

archived_date: 2026-04-18
---

## 6. The Route Manifest Pattern: The Key Abstraction

Instead of generating `__root.tsx` + `index.route.tsx` + `posts.route.tsx` (TanStack virtual file routes), generate:

```ts
// generated/routes.manifest.ts (regenerated on every vox build)
import { Home } from "./Home"
import { PostList } from "./PostList"
import { PostDetail } from "./PostDetail"

export type VoxRoute = {
  path: string
  component: React.ComponentType<any>
  loader?: () => Promise<any>
  pendingComponent?: React.ComponentType
  children?: VoxRoute[]
}

export const voxRoutes: VoxRoute[] = [
  { path: "/", component: Home },
  { path: "/posts", component: PostList, loader: () => fetch("/api/query/getPosts").then(r => r.json()) },
  { path: "/posts/:id", component: PostDetail, loader: ({ params }) => fetch(`/api/query/getPost?id=${params.id}`).then(r => r.json()) },
]
```

Then a **user-owned, once-generated `App.tsx`** consumes this manifest:

```tsx
// vox:skip
// app/App.tsx (scaffold — written once, never overwritten)
// This file is yours to modify. Vox never overwrites it.
// It adapts the voxRoutes manifest to your chosen router.
import { BrowserRouter, Routes, Route } from "react-router"
import { voxRoutes } from "../generated/routes.manifest"

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        {voxRoutes.map(r => (
          <Route key={r.path} path={r.path} element={<r.component />} />
        ))}
      </Routes>
    </BrowserRouter>
  )
}
```

If a user wants TanStack Router, they change the `App.tsx` adapter themselves. Vox never needs to change.

---

## 7. Server Functions: The API Client Pattern

Rather than generating `createServerFn` (TanStack-specific) or `"use server"` (Next.js-specific), generate a typed **API client** using standard `fetch`:

```ts
// generated/vox-client.ts (regenerated on every vox build)
const BASE = import.meta.env.VITE_API_URL ?? "http://localhost:4000"

export const voxClient = {
  // @query fn getPosts() -> list[Post]
  async getPosts(): Promise<Post[]> {
    const r = await fetch(`${BASE}/api/query/getPosts`)
    if (!r.ok) throw new Error(`getPosts failed: ${r.status}`)
    return r.json()
  },
  
  // @mutation fn createPost(title: str, body: str) -> Post  
  async createPost(data: { title: string; body: string }): Promise<Post> {
    const r = await fetch(`${BASE}/api/mutation/createPost`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    })
    if (!r.ok) throw new Error(`createPost failed: ${r.status}`)
    return r.json()
  },
}
```

This is **zero-dependency**, works in any environment (SPA, TanStack Start, Next.js client component, Expo React Native), and the interface is perfectly stable because it's just `fetch`.

A user integrating TanStack Query writes:
```ts
const posts = useQuery({ queryKey: ["posts"], queryFn: voxClient.getPosts })
```

Vox has no opinion on whether they use TanStack Query, SWR, React Query, or raw `useState`.

archived_date: 2026-04-18
---

## 8. Type Sharing: Rust → TypeScript

Research confirms this is well-solved via **`ts-rs`** crate:

```rust
use ts_rs::TS;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "frontend/src/generated/types.ts")]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
}
```

This auto-generates `types.ts` from `@table Post { title: str, body: str }` Vox declarations. The Vox compiler currently generates `types.ts` from HIR types. This pattern should complement the existing approach.

---

## 9. Axum ↔ React: The Topology That Always Works

Research confirms the canonical pattern for Axum + React SPA:

**Development:**
```
Browser → Vite dev server (port 5173) → proxy /api/* → Axum (port 4000)
```
Vite's `server.proxy` config handles this. No CORS needed in dev.

**Production:**
```
Browser → nginx/caddy → Axum (serves built dist/ as static fallback)
              ↓ /api/*
            Axum handlers
```
Axum's `ServeDir::new("dist").fallback(...)` serves `index.html` for all non-API paths. This is a single binary deployment.

This topology is **completely independent of routing framework choice.** Whether the SPA uses React Router, TanStack Router, or nothing, Axum just serves `index.html` and the browser handles the rest.

archived_date: 2026-04-18
---

## 10. Islands Architecture: Vox's Perfect Match

Research confirms the island architecture (Astro's model) maps exactly to Vox's `@island` model:

- "Sea": server-rendered static HTML (currently Axum + Askama/Tera templates, or a generated shell)
- "Islands": isolated interactive React components (`@island Name { prop: T }`)

Each island is hydrated independently — no routing library needed. The island pattern is the most stable web architecture available because:
- Islands are just React components (stable)
- Mounting is a single `ReactDOM.createRoot().render()` call per island (stable)
- No framework coordination needed
- v0 components are natural islands

**Vox's island system is already at 95% of the optimal architecture** for long-term stability.

---

## 11. What Vox Can Retire: The Confirmed List

Based on research, the following Vox constructs have NO stable framework analog and should be hard-retired:

| Vox Construct | Why Retire |
|--------------|-----------|
| `@component fn` (classic) | `@component fn` is literally just `@component Name()` minus 10% of the syntax. Migration is trivial. |
| `context: Name { }` | Context API is user-controlled. Vox generating context wrappers creates unmaintainable code. |
| `@hook fn` | React hooks are inside `@island` TypeScript — Vox cannot safely abstract them. |
| `@provider fn` | Providers belong in user-owned `App.tsx`. |
| `page: "path" { }` | No framework supports this exact construct. Use `routes { }`. |
| `layout: fn` (standalone, detached from routes) | A layout with no route context is meaningless. Wire to `routes { }` or retire. |

What should NOT be retired (contrary to some earlier thinking):
- `loading: fn` → becomes the `pendingComponent` value in the route manifest
- `not_found: fn` → becomes a registered fallback in `App.tsx`  
- `error_boundary: fn` → becomes an error boundary in user `App.tsx`
- `@island` → **Core feature, do not touch**
- `@v0` → **Keep, maps cleanly to an island stub**
- `routes { }` → **Core feature, emit route manifest from it**
- `@query`, `@mutation`, `@server` → **Keep, emit vox-client.ts entries**

archived_date: 2026-04-18
---

## 12. Tailwind v4 Impact on Vox

Vox emits JSX with `className="..."` strings from Path C component `view:` JSX directly. The actual Tailwind classes come from the user's Vox source code — Vox does not interpret or validate them.

Therefore, the Tailwind v4 migration concerns (class renames) affect **Vox users' source code**, not the Vox compiler itself. The only compiler concern is:
- The generated `tailwind.config.ts` scaffold must target v4 syntax (`@import "tailwindcss"`)
- The generated `globals.css` scaffold must use `@import "tailwindcss"` not the old `@tailwind base` / `@tailwind components` / `@tailwind utilities` directives

A single update to `scaffold.rs` covers this permanently.

---

## 13. Vite as the Build Universal

Vite is now the universal build tool across all major React frameworks:
- React Router v7 library mode: Vite
- TanStack Start: Vite (via Vinxi)  
- Next.js: custom (Turbopack) — the one framework NOT on Vite
- Plain SPA React: Vite

**Vox should generate Vite config as scaffold.** Because Vite's `defineConfig({...})` shape is very stable (unlike routing file conventions), a once-generated `vite.config.ts` with proxy setup will work long-term.

The only Vite-specific codegen concern is the `server.proxy` entry pointing to `VITE_API_URL`, which belongs in the scaffold.

archived_date: 2026-04-18
---

## 14. The Greenfield Migration Path

Research on compiler dead-code retirement confirms:
- Hard parser errors (not warnings) on truly retired syntax is the right approach
- Migration tooling (`vox migrate`) is important for adoption
- Golden examples do the most training signal work

For Vox's greenfield migration:
1. Retire `@component fn` with a hard error + automated migration command
2. Retire `context:`, `@hook`, `@provider`, `page:` with hard errors + migration guides
3. Add `loading:`, `not_found:` as first-class syntax within `routes { }` body
4. Change `routes { }` codegen from (broken) TanStack virtual files to route manifest

---

## 15. Summary of What Vox Must Support for 90-95% Modern React

| Layer | What to Support | Mechanism |
|-------|----------------|-----------|
| Components | Pure named-export React TSX | Path C → `.tsx` emitter (already exists) |
| v0 Interop | `@island` + named export contract + `@/components/ui/*` imports | `@island` + scaffold `components.json` |
| Styling | Tailwind class passthrough | No compiler work; scaffold `globals.css` + `vite.config.ts` |
| Routing | Route manifest (`voxRoutes[]`) | New codegen: `routes.manifest.ts` |
| Data | Typed fetch client | New codegen: `vox-client.ts` |
| Types | ADT types as TS interfaces | Existing `types.ts` emitter |
| Backend | Axum HTTP endpoints | Existing routes + server fn emitters |
| Hydration | Per-island `ReactDOM.createRoot()` | Existing `vox-islands-meta.ts` |
| Scaffold | `vite.config.ts`, `App.tsx`, `main.tsx`, `components.json`, `globals.css` | New scaffold emitter (one-time write) |

Everything in this table maps to stable, long-lived APIs. The only volatile part was the routing layer — now replaced by an abstract manifest that a user-owned `App.tsx` adapts.


