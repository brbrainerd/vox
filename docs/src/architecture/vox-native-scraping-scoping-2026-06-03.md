---
title: Native Scraping / Browser Automation in Vox — Scoping & Handoff (2026-06-03)
description: Honest scope for making scrapers/checkers/screenshots easy to write in Vox (and easy for LLMs). Audits what already exists (a chromiumoxide CDP engine + a static fetch/parse path are already in-tree), surveys the Rust landscape, and recommends a majority-of-benefit path that is mostly assembly + ergonomics + governance, not a from-scratch build.
category: "Architecture SSOTs"
---

# Native Scraping / Browser Automation in Vox — Scoping & Handoff

**Goal (from the request):** make it easy to write scrapers, checkers, and "screenshot things" from a `.vox`
script — keeping Vox's advantages and making it easy for LLMs to author them.

**The headline, up front:** Vox is **not** starting from zero. A production **chromiumoxide (Chrome
DevTools Protocol) browser engine already exists in-tree and is already callable from a `.vox` script**, and
a **static fetch + HTML-parse + readability pipeline** also exists. "Add native Playwright to Vox" is
therefore mostly **assembly + ergonomics + governance + docs**, not a months-long reimplementation. Full
Playwright parity *would* be months — and we should not build it.

---

## 1. What already exists (audited)

### 1a. A native browser capability, reachable from Vox today
- `.vox` scripts can call **`Browser.open / goto / click / fill / wait_for / text / html / screenshot /
  visible_text_summary / ax_tree / close`** — these are **registered builtins**
  (`crates/vox-compiler/src/builtin_registry.rs:170+`, namespace `Browser`) that lower to
  `vox_actor_runtime::builtins::vox_browser_*` (`crates/vox-actor-runtime/src/builtins/mod.rs:1237`).
- The actual engine is the **L4 plugin `vox-plugin-browser`** driving **chromiumoxide 0.9.1 (CDP)**
  (`crates/vox-plugin-browser/Cargo.toml:15`, `src/engine.rs`), behind the `BrowserAutomation` sabi_trait
  (`crates/vox-plugin-api/src/extensions/browser_automation.rs:13`). It already supports CSS **and**
  `xpath:` selectors, headless/headful, screenshots, and — notably for LLMs — **`visible_text_summary`**
  (stripped LLM-ready text) and **`ax_tree`** (accessibility tree).
- It's isolated as a runtime-loaded plugin, so the heavy CDP dependency tree does **not** bloat the core
  (the L4-plugin convention is the de-facto guard; `layers.toml:190`). There's even a CI smoke job
  (`vox-browser-cdp-smoke`, `.github/workflows/ci.yml:846`).

### 1b. A static (no-browser) fetch + parse path
- **`vox-search::scraper::fetch_and_extract(url, timeout_ms)`** (`crates/vox-search/src/scraper.rs:14`):
  `reqwest` → `scraper` (html5ever + CSS selectors) → readability-style main-content selection →
  `html2text` → Markdown, with a text-density heuristic. Gated behind vox-search's `web-scrape` feature.
- **`vox-http-client`** (`crates/vox-http-client/src/lib.rs:26`) is the **mandated shared reqwest facade**
  (timeout/retry presets); every new fetch path should build on `client_builder()`.
- URL discovery exists too: `WebSearchDispatcher` (SearXNG → DuckDuckGo → Tavily,
  `crates/vox-search/src/web_dispatcher.rs`).

### 1c. The runtime/effect context (the two real constraints)
- **Native-lane only.** `Browser.*` is compiled to a hard error on `wasm32`
  (`vox-codegen/src/codegen_rust/emit/stmt_expr.rs:895`) because WASI can't open sockets / launch Chrome.
  Browser automation is therefore confined to the **trusted native backend** — the opposite lane from the
  sandbox tiers used for untrusted code. Offering it to sandboxed scripts would need an out-of-process
  broker (a different, larger design). *Structural, not incidental.*
- **It bypasses the effect system.** `Browser.*` is **absent** from `stdlib_module_capability`
  (`vox-compiler/src/typeck/effect_check.rs:506`), so it requires **no `uses net`/`uses spawn`** and is
  invisible to the `caller ⊇ callee` capability check — despite being the most network/process-heavy thing
  a script can do. The fix is small and local (add `Browser`/`scrape` → `Net`(+`Spawn`/`Fs`) rows, mirroring
  `http → Net`). This is the **governance keystone**.
- **Async is already bridged.** Native `main` is `#[tokio::main]`; the plugin wraps async CDP behind its own
  Tokio runtime so the sync ABI methods block (`vox-plugin-browser/src/engine.rs:5`). Vox code calls a
  blocking `Browser.open(...)`; async is an implementation detail.

---

## 2. The Rust landscape (verified, 2025–2026)

| Option | Protocol | External artifact needed | JS | Screenshot | Maint. (latest) | Fit |
|---|---|---|---|---|---|---|
| **chromiumoxide** | CDP (tokio) | a Chromium **binary** (auto-fetchable) | ✅ | ✅ | **active** 0.9.1 (2026-02) | **already in-tree** — the browser pick |
| reqwest + **scraper** | HTTP + CSS | **none** (pure-Rust) | ❌ | ❌ | active (scraper 0.26) | **the static pick** — already in-tree |
| thirtyfour | WebDriver | chromedriver (auto-managed) | ✅ | ✅ | active 0.36 | not needed; CDP already present |
| fantoccini | WebDriver | chromedriver/geckodriver | ✅ | ✅ | active 0.22 | not needed |
| headless_chrome | CDP (**sync**) | Chromium | ✅ | ✅ | revived 1.0.21 | sync model fights tokio; skip |
| spider | HTTP(+CDP) | Chromium only if JS | ✅ | – | active 2.48 | only if we need a crawler frontier |
| **playwright-rust** (padamson) | binds Playwright Node driver | **Node + driver + browsers** | ✅ (true auto-wait) | ✅ | active 0.13 (pre-1.0) | only if true Playwright semantics needed |

**Load-bearing truth:** every JS-capable option needs an external artifact (a Chromium binary, a WebDriver
process, or Node+driver+browsers). Only `reqwest + scraper` is fully self-contained. Vox already chose
**chromiumoxide** — the pure-Rust, tokio-native, no-Node option — which is the right call.

**What is NOT worth building:** a pure-Rust Playwright reimplementation (auto-wait engine, text/role
selector engine, network interception, tracing/video). None exists in Rust; it's **months** of work for
diminishing returns. If Vox ever truly needs those semantics, **bind `padamson/playwright-rust`** and accept
the Node/driver deploy dependency — don't rebuild it.

---

## 3. Tracks (options on the table)

- **Track A — Ergonomize + govern the existing CDP engine (RECOMMENDED).** The engine and the `Browser.*`
  builtins exist; invest in (i) an LLM-friendly stdlib surface, (ii) effect governance, (iii) docs/examples.
  *Mostly assembly.* Low–medium effort.
- **Track B — Promote the static path to a first-class default (RECOMMENDED, cheap).** Lift
  `vox-search::scraper` into a reusable `scrape`/`web` primitive (or a small L2 crate) so the **no-browser**
  tier — which covers the large majority of scrapers/checkers — is the easy default. ~days.
- **Track C — WebDriver (thirtyfour/fantoccini).** Skip: chromiumoxide already covers the JS path without a
  driver process.
- **Track D — Emit-to-Playwright (a Node codegen target).** Genuinely new: today's TS emit profiles are
  browser-SPA and RN/Expo only — there is **no standalone Node-script emitter** and no playwright/puppeteer
  in any emitted `package.json` (audited in `vox-codegen/src/codegen_ts/scaffold.rs`). Defer unless we want
  Vox to *generate* Playwright scripts rather than drive a browser natively.
- **Track E — Bind `padamson/playwright-rust`.** Only if auto-wait/selector-engine/tracing become hard
  requirements. Heavy deploy deps; pre-1.0 churn.

---

## 4. Recommendation — majority of benefit for least code

Ship **one `scrape` (or `web`) stdlib module with two tiers and a single result type**, then make it
LLM-ergonomic and governed. Almost all of this is wiring over code that already exists.

### Tier 1 — static, default, pure-Rust (Track B)
```
let page = scrape.fetch("https://example.com")        // reqwest via vox-http-client
let titles = page.select("h2.title").map(|n| n.text())  // scraper CSS selectors
let price  = page.select_one("#price")?.attr("data-usd")
```
Backed by `vox-search::scraper` + `vox-http-client`. **No external binary.** Covers content/link/uptime
**checkers** and most structured extraction. **Effort: ~1–3 days** (mostly exposing existing functions as
stdlib builtins + a small `Node`/`Page` value type).

### Tier 2 — browser, opt-in (Track A; engine already built)
```
let pg = scrape.render("https://app.example.com")   // headless chromiumoxide (existing engine)
pg.wait_for(".results", 5s)
pg.fill("#q", "vox"); pg.click("button.search")
let shot = pg.screenshot()                           // bytes → save/return
let text = pg.visible_text_summary()                 // LLM-ready stripped text (already implemented!)
```
This is the **existing `Browser.*`** surface, re-presented as the same ergonomic `scrape` API. The ~7 core
verbs (goto / wait_for / select / text / attr / click / fill / eval / screenshot) cover the overwhelming
majority of JS-page scraping and **screenshotting**. Make the Chromium dependency a documented,
feature-gated/auto-fetched opt-in so the default Vox build stays browser-free. **Effort: ~1–2 weeks** for a
polished v1 — and most of that is ergonomics, error mapping, timeouts, and docs, *not* the engine.

### Cross-cutting (small but important)
- **Governance keystone:** add `Browser`/`scrape` → `Net` (+`Spawn` for launch, +`Fs` for screenshot writes)
  to `stdlib_module_capability` so scrapers must declare `uses net, fs` and are visible to effect-checking.
  ~1 file, high value.
- **LLM-friendliness is the real differentiator:** keep the verb set tiny and declarative; make results
  deterministic; lean into the already-built **`visible_text_summary`** and **`ax_tree`** (great for "LLM
  reads the page" checkers); ship a handful of canonical `.vox` examples (a price checker, a link/uptime
  checker, a "screenshot every page" sweep). The GUI screenshot harness we just built
  (`crates/vox-gui/ui/e2e/screenshots.spec.ts`) is exactly the kind of thing that should become a tiny
  `.vox` script using `scrape.render(...).screenshot()`.

### Explicitly out of scope (be honest)
- A real auto-wait engine, text/role/locator selector engine, network interception, tracing/video — these
  are where effort explodes. Use poll-for-selector `wait_for` (already present) and stop there. Revisit only
  by binding `playwright-rust` if a concrete need appears.
- Browser automation for **sandboxed/WASI/untrusted** scripts — structurally impossible without an
  out-of-process broker; keep scraping a **native-lane, trusted-capability** feature.

---

## 5. Suggested sequencing

1. **Track B (static tier)** — expose `vox-search::scraper` + `vox-http-client` as a `scrape.fetch/select`
   stdlib module with a `Page`/`Node` value type. Cheap, big coverage, no external deps.
2. **Governance** — add the `scrape`/`Browser` → `Net`/`Spawn`/`Fs` effect rows.
3. **Track A (browser tier)** — re-present `Browser.*` as `scrape.render(...)` with the unified result type;
   feature-gate/auto-fetch Chromium; map errors; document.
4. **LLM enablement** — canonical examples + the `visible_text_summary`/`ax_tree` ergonomics; port the GUI
   screenshot sweep to a `.vox` script as the flagship demo.
5. **Defer** Track D (Node/Playwright emit) and Track E (playwright-rust binding) until a concrete need for
   true Playwright semantics emerges.

**Net:** ~2–3 weeks gets a governed, LLM-friendly, two-tier scraping/screenshot surface in Vox by assembling
existing in-tree machinery — versus months and high risk to chase Playwright parity from scratch.
