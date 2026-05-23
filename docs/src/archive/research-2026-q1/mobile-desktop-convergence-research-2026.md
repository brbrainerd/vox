---
title: "Mobile/Desktop Convergence & Language Extension Research 2026"
description: "Research findings on mobile–desktop view convergence, device API unification, and parser gaps for agent/environment declarations. Informs future implementation planning."
category: "Architecture SSOTs"
last_updated: "2026-04-06"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Mobile/Desktop Convergence & Language Extension Research 2026

> **Status**: Research only. Not an implementation plan. Informs future planning decisions.
>
> **Scope**: (1) Parser gaps for `agent` and `environment` declarations, (2) current mobile support inventory and its limitations, (3) a path to a unified browser-based frontend for both desktop and mobile with a standardized device API surface.

---

## 1. Executive Summary

Vox's current mobile story has three disconnected layers:

1. **`@mobile.native` annotation** — parses onto any `fn`, sets `is_mobile_native: bool`, and emits a Capacitor `VoxNative.invoke` bridge stub in `mobile-bridge.ts`. This is purely a codegen hint; there is no runtime, no stdlib module, no type system integration.
2. **`std.mobile` namespace** — imported in golden examples (`examples/golden/mobile_camera.vox`, `examples/golden/mobile_test.vox`) and used as `mobile.take_photo()`, `mobile.vibrate()`, `mobile.notify()`. There is **no Rust implementation** of this namespace anywhere in the codebase. It is aspirational syntax only.
3. **`agent` and `environment` AST nodes** — fully specified in `ast/decl/logic.rs` and `ast/decl/config.rs` but have **zero parser coverage**. The golden examples that use them (`ref_agents.vox`, `ref_orchestrator.vox`) have been `.skip`-ed from the test suite.

The gap between what the syntax promises and what is implemented is large. The good news: the target architecture (browser-based unified frontend via WebView/PWA, device access via well-supported Web APIs) is achievable with low technical debt if we pick the right primitives.

archived_date: 2026-04-18
---

## 2. Current State Inventory

### 2.1 What Exists (Implemented)

| Feature | File(s) | Status |
|---|---|---|
| `@mobile.native` token | `lexer/cursor.rs`, `token.rs` | ✅ Lexes |
| `@mobile.native` annotation on `fn` | `parser/descent/decl/head.rs` | ✅ Parses; sets `is_mobile_native` |
| `FnDecl.is_mobile_native` AST field | `ast/decl/fundecl.rs` | ✅ Present |
| `HirFn.is_mobile_native` HIR field | `hir/nodes/decl.rs` | ✅ Present |
| `emit_mobile_bridge_fn` codegen | `codegen_ts/hir_emit/mod.rs` | ✅ Emits Capacitor invoke stub |
| `mobile-bridge.ts` file emission | `codegen_ts/emitter.rs` | ✅ Emits if any `@mobile.native` fns present |
| `import * as mobile from "./mobile-bridge"` | `codegen_ts/component.rs` | ✅ Auto-injected when `mobile.*` ident used |
| `AgentDecl` AST struct | `ast/decl/logic.rs` | ✅ Struct defined |
| `AgentHandler`, `MigrationRule` structs | `ast/decl/logic.rs` | ✅ Structs defined |
| `EnvironmentDecl` AST struct | `ast/decl/config.rs` | ✅ Struct defined with full fields |
| `Decl::Agent`, `Decl::AgentDef`, `Decl::Environment` | `ast/decl/types.rs` | ✅ Enum variants exist |

### 2.2 What Does Not Exist (Gap)

| Feature | Expected Location | Gap |
|---|---|---|
| `std.mobile` stdlib module | `vox-runtime/src/` | ❌ Not implemented anywhere |
| `mobile.take_photo()` type signature | `typeck/builtins.rs`, `builtin_registry.rs` | ❌ No registration |
| `mobile.vibrate()`, `mobile.notify()` sigs | Same | ❌ No registration |
| `agent` keyword parsing | `parser/descent/mod.rs` | ❌ Falls through to "unexpected token" |
| `parse_agent()` function | `parser/descent/decl/mid.rs` | ❌ Missing entirely |
| `environment` keyword parsing | `parser/descent/mod.rs` | ❌ Same |
| `parse_environment()` function | `parser/descent/decl/mid.rs` | ❌ Missing entirely |
| `Token::Agent`, `Token::Environment` tokens | `lexer/token.rs` | ❌ Not in lexer |
| HIR lowering for `AgentDecl` | `hir/lower/decl.rs` | ❌ Not lowered |
| HIR lowering for `EnvironmentDecl` | `hir/lower/decl.rs` | ❌ Not lowered |
| Codegen for `AgentDecl` | `codegen_ts/` | ❌ Not emitted |
| Codegen for `EnvironmentDecl` (→ Dockerfile) | `vox-container` | ❌ Not wired |
| Mobile capability type-checking | `typeck/` | ❌ No `mobile` namespace typeck |
| `@ionic/pwa-elements` integration | generated scaffold | ❌ Not in templates |

### 2.3 The `std.mobile` Fiction Problem

`mobile_camera.vox` calls `mobile.take_photo()`, `mobile.notify()`, `mobile.vibrate()`. These are imported from `std.mobile`. The compiler emits `import * as mobile from "./mobile-bridge"` when it detects the `mobile` ident, which in turn requires `@mobile.native`-annotated functions to exist. But the `mobile_camera.vox` golden uses them as a normal library, not as user-declared bridge functions.

**This means**: the golden example currently passes the parser test but would produce non-functional code. There is an abstraction gap: the compiler treats `mobile.*` as "use a Capacitor bridge" but has no notion of `std.mobile` as a standard module with defined methods.

---

## 3. Mobile Support Limitations Analysis

### 3.1 The Three Deployment Scenarios

| Scenario | Current Support | Target |
|---|---|---|
| **Browser (desktop)** | React TSX via Vite, full web platform | ✅ Good |
| **Mobile browser (PWA)** | Same TSX output; no mobile-specific scaffolding | 🔶 Partial — works but no native hardware |
| **Mobile native (iOS/Android)** | `@mobile.native` → Capacitor bridge stub | ❌ Requires user to wire Capacitor project manually |
| **Electron/desktop native** | Not addressed | ❌ No story |

### 3.2 PWA Capabilities vs. Gaps (2026 Research)

The browser is a viable cross-platform runtime for Vox's use cases. As of 2026:

**What works on both desktop browsers and mobile browsers (no native wrapper required):**

| Capability | API | Desktop | Mobile (Android) | Mobile (iOS Safari) |
|---|---|---|---|---|
| Camera/microphone access | `navigator.mediaDevices.getUserMedia()` | ✅ | ✅ | ✅ (HTTPS required) |
| Photo capture | `MediaDevices` + video stream | ✅ | ✅ | ✅ |
| Geolocation | `navigator.geolocation` | ✅ | ✅ | ✅ (foreground only) |
| Accelerometer / DeviceMotion | `DeviceMotionEvent` | ✅ (if HW present) | ✅ | ✅ (requires permission request) |
| Device orientation | `DeviceOrientationEvent` | ✅ (if HW present) | ✅ | ✅ |
| Vibration | `navigator.vibrate()` | Partial (Chrome only) | ✅ | ❌ |
| Push notifications | Push API + Service Worker | ✅ | ✅ | ✅ (iOS 16.4+, home screen only) |
| Offline / storage | Cache API, IndexedDB | ✅ | ✅ | ✅ |
| Speech recognition | Web Speech API | ✅ Chrome | ✅ | ✅ Safari |
| Clipboard | Clipboard API | ✅ | ✅ | ✅ |
| Background sync | Background Sync API | ✅ | ✅ | ❌ iOS |

**Hard gaps that require a native wrapper (Capacitor/Tauri) for production quality:**

| Capability | Gap |
|---|---|
| Background execution / wake | iOS blocks all background PWA activity |
| Silent push notifications | Not available on iOS PWA |
| Background location (geofencing) | iOS only in native apps |
| Advanced camera controls (zoom, manual focus, RAW) | Native SDKs only |
| Bluetooth / NFC | Limited/no browser support |
| File system access | Sandboxed on mobile browsers |
| Haptic feedback (real haptics) | Vibration API inadequate; need native |
| App Store distribution | Requires native wrapper |

### 3.3 The Convergence Strategy

**Key insight**: For Vox's stated use cases (photo upload, notifications, basic sensors), the Web API tier is sufficient and covers both desktop and mobile browsers with a single code path. This aligns with the goal of a "browser-based view for maintainability."

The recommendation is a **three-tier model**:

```
Tier 1: Pure Web API (default)
  → Works on desktop browsers, mobile browsers, Capacitor web tier
  → navigator.mediaDevices.getUserMedia()
  → navigator.geolocation.getCurrentPosition()
  → DeviceMotionEvent
  → Web Vibration API (where supported)

Tier 2: Capacitor Enhancement (opt-in, progressive)
  → Wraps the same Web APIs but adds native UX polish
  → @capacitor/camera → better native camera sheet on iOS
  → @capacitor/haptics → real haptic engine on mobile
  → @ionic/pwa-elements → camera UI on desktop web fallback

Tier 3: Native Extension (@mobile.native annotation)
  → For anything not in Tiers 1-2
  → User-defined Capacitor plugin with Swift/Kotlin impl
  → Vox declares the interface; native code implements it
```

This is the key insight for **why the std.mobile namespace matters**: it should map Tier 1 (Web API) by default with a Capacitor enhancement for Tier 2.

archived_date: 2026-04-18
---

## 4. Agent Declaration Gap Analysis

### 4.1 What the AST Expects

The `AgentDecl` struct supports:
- **Name** (`name: String`)
- **Version** (`version: Option<String>`)
- **State fields** (typed fields, same as ADT variants)
- **Handlers** (`on EventName(params) -> ReturnType { body }`)
- **Migration rules** (`migrate from "previous_version" { body }`)
- **Deprecation flag**

This closely matches 2026 industry patterns for stateful, versioned agent DSLs. The design is sound.

### 4.2 What the Parser Needs

The `agent` keyword doesn't exist in the lexer. The full gap is:

**Step 1: Lexer** (`lexer/cursor.rs`, `token.rs`)
- Add `Token::Agent` mapping `"agent"`
- Add `Token::Migrate` mapping `"migrate"`  
- Add `Token::Version` mapping `"version"` (as identifier-safe keyword, like `on`/`state`)
- `from` may already exist or can be treated as an ident

**Step 2: Parser** (`parser/descent/decl/mid.rs`)
- `parse_agent()` — new function mirroring `parse_actor()` structure:
  - Advance past `agent`
  - Parse name (TypeIdent, since agents are PascalCase)
  - Parse optional `version "x.y.z"` string
  - Parse `{` body with loop over:
    - `on EventName(params) -> rettype { body }` → `AgentHandler`
    - `migrate from "ver" { body }` → `MigrationRule`
    - state fields (typed `name: Type`) → push to `state_fields`
  - Close `}`

**Step 3: Top-level dispatch** (`parser/descent/mod.rs`)
- Add `Token::Agent => self.parse_agent()` arm
- Add `Token::Agent` to `recover_to_top_level()` break list

**Step 4: HIR lowering** (`hir/lower/decl.rs`)
- `AgentDecl` → some HIR representation (can reuse actor lowering shape or define `HirAgent`)
- `MigrationRule` needs a HIR migration node or can be a special `HirFn` with a tag

**Step 5: Codegen** (TBD — not researched for this pass)
- TypeScript codegen: agent → class with versioned constructor + event dispatch methods
- Or: emit as an orchestrator worker registration

### 4.3 Complexity Estimate (Parser Only)

| Work item | Effort | Risk |
|---|---|---|
| 3 new tokens in lexer | 30 min | Low |
| `parse_agent()` function | 2h | Low (mirrors `parse_actor()`) |
| Top-level dispatch + recovery | 30 min | Low |
| Golden example `ref_agents.vox` restored | 1h | Low |
| HIR lowering stub | 1h | Low (can stub empty for now) |
| **Total parser+HIR stub** | **~5h** | **Low** |

---

## 5. Environment Declaration Gap Analysis

### 5.1 What the AST Expects

`EnvironmentDecl` is the most fully-specified unimplemented node. It models a Dockerfile in Vox syntax:

```vox
// vox:skip
environment production {
    base "node:22-alpine"
    packages ["curl", "git"]
    env NODE_ENV = "production"
    env PORT = "3000"
    expose [3000, 443]
    volumes ["/data"]
    workdir "/app"
    run "npm install --production"
    cmd ["node", "server.js"]
}
```

This maps directly to Docker/OCI concepts. The `EnvironmentDecl` struct has all these fields:
`base_image`, `packages`, `env_vars` (Vec of k/v tuples), `exposed_ports`, `volumes`, `workdir`, `cmd`, `copy_instructions`, `run_commands`.

### 5.2 What the Parser Needs

**Step 1: Lexer**
- Add `Token::Environment` mapping `"environment"`
- `base`, `packages`, `expose`, `volumes`, `workdir`, `run`, `cmd` — these are **not** reserved words and can be parsed as bare idents inside the block body (like `view:` uses ident dispatch)

**Step 2: Parser** (`parser/descent/decl/mid.rs` or new `config.rs`)
- `parse_environment()`:
  - Advance past `environment`
  - Parse name as a plain ident (production, staging, dev)
  - Expect `{`
  - Loop parsing "directive idents" as a switch:
    - `base "string"` → parse string literal
    - `packages [...]` → parse list of string literals
    - `env IDENT = "val"` → parse env var pair
    - `expose [...]` → parse list of integer literals
    - `volumes [...]` → parse list of strings
    - `workdir "string"` → parse string
    - `run "string"` → parse string, push to run_commands
    - `cmd [...]` → parse list of strings
    - `copy "src" "dest"` → parse two strings
  - Close `}`

**Step 3: Top-level dispatch**
- Add `Token::Environment => self.parse_environment()` arm

**Step 4: Codegen** (`vox-container` crate — pre-existing)
- `vox-container` already exists; this is where `EnvironmentDecl` → Dockerfile emission belongs

### 5.3 Complexity Estimate

| Work item | Effort | Risk |
|---|---|---|
| 1 new token (`environment`) in lexer | 15 min | Low |
| `parse_environment()` function | 3h | Medium (many directive arms) |
| Top-level dispatch + recovery | 15 min | Low |
| `vox-container` wiring | 2h | Medium |
| Golden example `ref_orchestrator.vox` fix | 1h | Low |
| **Total** | **~7h** | **Medium** |

archived_date: 2026-04-18
---

## 6. The `std.mobile` Module Design

### 6.1 What It Should Be

`std.mobile` should be a **compiler-known namespace module** (like `std.math`, `std.fs`), not a user-declared Capacitor bridge. The compiler resolves `import std.mobile` → inject the Web API or Capacitor bridge module at codegen time.

### 6.2 Proposed Method Surface

```vox
// vox:skip
// The std.mobile API Vox authors see
import std.mobile

// Camera
mobile.take_photo() -> Result[str]          // Returns URI/data URL of captured photo
mobile.take_photo_from_gallery() -> Result[str]

// Sensors
mobile.vibrate() -> unit                    // Best-effort (silently no-ops on unsupported)
mobile.vibrate(duration_ms: int) -> unit

// Notifications  
mobile.notify(title: str, body: str) -> unit
mobile.notify(title: str, body: str, icon: str) -> unit

// Location
mobile.get_location() -> Result[Location]   // { lat: dec, lng: dec, accuracy: dec }

// Sensors
mobile.accelerometer() -> Result[AccelData] // { x: dec, y: dec, z: dec }
mobile.orientation() -> Result[Orientation] // { alpha: dec, beta: dec, gamma: dec }

// Clipboard
mobile.copy_to_clipboard(text: str) -> unit
mobile.read_clipboard() -> Result[str]

// Hardware detection
mobile.has_camera() -> bool
mobile.has_motion_sensor() -> bool
mobile.platform() -> str                    // "ios" | "android" | "web" | "desktop"
```

### 6.3 Codegen Strategy

At codegen time, `import std.mobile` → emit different JS depending on target:

| Target | Emitted import | Implementation |
|---|---|---|
| `web` (default) | Inline Web API wrappers | `navigator.mediaDevices`, `DeviceMotionEvent`, etc. |
| `capacitor` (when `@capacitor/core` in project) | `import { Camera, Motion, Haptics } from "@capacitor/*"` | Capacitor plugin calls |
| `@mobile.native` fns in same file | Keep existing bridge generation | Capacitor custom plugin |

The emitted `mobile-utils.ts` file replaces the current `mobile-bridge.ts`. It always includes Web API fallbacks, with Capacitor enhancement where available.

**Key design win**: The `.vox` author writes one API. The compiler decides which runtime to emit. This is the same pattern as `state` → React hooks.

---

## 7. Unified Frontend Architecture

### 7.1 The "Browser View for Both" Goal

The user's stated goal: same or similar frontend for desktop and mobile, using browser-based rendering for maintainability. This fully aligns with:

1. **Vox's existing codegen output** → React + Vite (runs in any modern browser)
2. **Capacitor's model** → wraps the same WebView in a native shell for app stores
3. **Web APIs** → device hardware accessible from the same JS code on both desktop and mobile

The only real work is ensuring Vox's generated scaffold includes:
- Responsive CSS (container queries, mobile-first layout)
- The correct Capacitor scaffold when targeting native
- `@ionic/pwa-elements` for camera UI in pure web deployments
- Proper HTTPS enforcement (required for device APIs)

### 7.2 Template Evolution

Current templates (`spa.rs`, `islands.rs`, `tanstack.rs`) generate plain Vite projects. They need a `mobile` variant that adds:

```json
// Extra deps for mobile-capable generated projects
"@capacitor/core": "6.x",
"@capacitor/camera": "6.x",
"@capacitor/haptics": "6.x",
"@capacitor/geolocation": "6.x",
"@ionic/pwa-elements": "latest"
```

And a `capacitor.config.ts` scaffold. This is additive; it does not change the existing templates.

**`vox new --template mobile-pwa`** → generates the Vite project + PWA manifest + service worker + Capacitor config + mobile-ready CSS.

archived_date: 2026-04-18
---

## 8. Quantified Win Summary

| Improvement | Maintainability Delta | Support Delta |
|---|---|---|
| **std.mobile namespace** (compiler-resolved) | Eliminates manual Capacitor wiring per-function; single API forever | Adds camera, location, motion to all projects |
| **Web API tier-1 default** | Zero native dependencies for 80% of use cases | Camera + location + motion on desktop + mobile browsers |
| **Capacitor tier-2 opt-in** | Same `.vox` code; compiler switches it backend to native | App Store viability; real haptics; background push |
| **agent declaration parser** | Restores golden example; enables vox-orchestrator agent authoring in .vox | Agents can be declared in-language rather than hand-coded Rust/TS |
| **environment declaration parser** | Restores golden example; enables Dockerfile generation from vox | Single-file full-stack+infra definition |
| **Responsive CSS in templates** | Nothing extra to remember; mobile layout is the default | Look & feel parity desktop ↔ mobile |

### Maintainability Scores (1-10, 10 = very maintainable)

| Item | Before | After (estimated) |
|---|---|---|
| Mobile hardware access pattern | 3 (manual per-fn bridge) | 8 (compiler-resolved namespace) |
| Desktop/mobile code divergence | 4 (separate concerns) | 8 (same std.mobile, same JS output) |
| Agent authoring | 1 (not in language) | 7 (first-class `.vox` syntax) |
| Environment/infra specification | 1 (external YAML only) | 7 (in-language, compiler-validated) |
| Cross-platform device test coverage | 2 (no stubs) | 6 (Web API polyfillable in test env) |

---

## 9. Open Questions (for Implementation Planning)

1. **Token namespace for `agent`**: Should `version`, `migrate`, `from` be reserved keywords or parsed contextually as idents? Contextual is safer (fewer regressions); reserved is cleaner.
2. **`environment` directive parsing**: Some directives (`run`, `cmd`, `workdir`) clash with common English words. Should they only be keywords inside `environment { }` blocks (contextual)?
3. **HIR representation for agents**: Should `AgentDecl` lower to a `HirActor` (reusing existing machinery) or to a new `HirAgent` node? The semantic difference is the versioning/migration concept.
4. **`std.mobile` scope**: Should `std.mobile` be a marker import that the compiler replaces wholesale, or should it be a real module the runtime exposes? The former is simpler (no Rust dispatch); the latter enables testing.
5. **Capacitor coupling**: Should `std.mobile` → Capacitor scaffold be opt-in (`vox new --mobile`) or automatically injected when `std.mobile` is imported? Auto-inject risks bloating non-mobile projects.
6. **iOS PWA EU law gap**: Due to EU DMA rules (iOS 17.4+), PWAs may not function in standalone mode in the EU. For App Store distribution path (Tier 2), Capacitor is mandatory. Document this as a known limit.
7. **`mobile.platform()` implementation**: Desktop browsers don't expose a reliable "I am desktop" vs "I am mobile" signal. `navigator.userAgentData.mobile` is the closest (Chromium only). Need fallback strategy.

archived_date: 2026-04-18
---

## 10. Related Documents

- [Vox Cross-Platform Runbook](vox-cross-platform-runbook.md) — lane definitions (S/A/M/R)
- [Web Architecture Analysis 2026](web-architecture-analysis-2026.md) — frontend convergence path (Path C)
- [Vox Android Platform Support Research](../../..) — `vox_android_platform_support` KI
- [Vox Web Architecture and TypeScript SDK Interop](../../..) — `vox_web_architecture_and_ts_interop` KI
- `docs/src/reference/mobile-edge-ai.md` — mobile/edge AI SSOT
- `crates/vox-container/` — Dockerfile generation target for `EnvironmentDecl`
- `crates/vox-compiler/src/ast/decl/logic.rs` — `AgentDecl` struct (awaiting parser)
- `crates/vox-compiler/src/ast/decl/config.rs` — `EnvironmentDecl` struct (awaiting parser)
- `contracts/terminal/exec-policy.v1.yaml` — shell policy (relevant to `environment` codegen)


