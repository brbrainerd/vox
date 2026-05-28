---
title: "Gradio & Streamlit Research (2026): What VUV Should Steal, Adapt, and Reject"
description: "Primary-source history and architectural review of Gradio and Streamlit, the two dominant Python web-GUI libraries, framed as design input to Vox's VUV authoring layer. Records what each got right, where each falls below Vox's quality bar, and what translates well or badly to LLM-emitted GUI code."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-08"
training_eligible: true
training_rationale: "Canonical reference for VUV's design rationale relative to the dominant prior art. Cited from gui-authoring-syntax-2026.md and external-frontend-interop-plan-2026.md."
---

# Gradio & Streamlit Research (2026)

**Audit date:** 2026-05-08
**Companion to:** [GUI Authoring Syntax (2026): Vox UI as Values (VUV)](gui-authoring-syntax-2026.md) · [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) · [Frontend Convergence Findings (2026)](frontend-convergence-findings-2026.md)

## Why this document exists

Vox's UI authoring layer (VUV) targets two readers at once: a human writing a `.vox` view, and a large language model emitting one. Two existing Python libraries — **Gradio** and **Streamlit** — dominate the niche of "Python script → web GUI" and are, by community observation, among the GUI libraries most successfully emitted by current LLMs. Both are also the libraries whose limitations were the original motivation for VUV existing: this maintainer started here, hit the ceiling, and wanted something else.

This document is the explicit reconciliation: a primary-source-grounded history of how Gradio and Streamlit got to where they are, an architectural review of what they actually do, an enumeration of where each falls below Vox's quality standards, and a row-by-row keep/adapt/reject mapping into the VUV substrate that already exists (typed primitives, token-validated style kwargs, named-arg behavior, Web IR → TSX emit).

We are not trying to clone either tool. We are not trying to write a Vox port of `gr.Blocks` or `st.write`. We are trying to surface every design decision in those libraries that we have an opinion on — keep, adapt, or reject — so that VUV's design choices have explicit, recorded reasons rather than implicit ones.

This doc is an input to VUV's continuing design, not a roadmap. Concrete VUV phases live in [gui-authoring-syntax-2026.md](gui-authoring-syntax-2026.md); concrete interop work lives in [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md). When this doc says "VUV should ___," it means "the next time we change VUV, this should be on the table."

---

## 1. Gradio

### 1.1 Origin and provenance

Gradio was created by **Abubakar Abid** with five collaborators (Ali Abdalla, Ali Abid, Dawood Khan, Abdulrahman Alfozan, James Zou) during Abid's PhD at **Stanford University in James Zou's lab**. The original paper, *"Gradio: Hassle-Free Sharing and Testing of ML Models in the Wild,"* was submitted to arXiv on **2019-06-06** ([arXiv:1906.02569](https://arxiv.org/abs/1906.02569)) and presented at the 2019 ICML Workshop on Human in the Loop Learning.

The origin story is concrete and constraining: Abid needed to share a medical computer-vision model with a non-programmer clinical collaborator. The paper's abstract states the problem as accessibility — "challenging for non-technical collaborators and endpoint users (e.g. physicians) to easily provide feedback on model development" — and lists four design requirements derived from researcher interviews:

1. Support multiple frameworks (TensorFlow, PyTorch, generic Python).
2. URL-based sharing of running models.
3. Interactive inference for non-technical domain experts.
4. iPython/Jupyter notebook integration.

Every later Gradio decision is downstream of this constraint: *share a working ML model with a non-programmer over a URL, from a notebook, in three lines.*

Gradio joined Hugging Face on **2021-12-21** ([huggingface.co/blog/gradio-joins-hf](https://huggingface.co/blog/gradio-joins-hf)). Per Abid's later retrospective ([x.com/abidlabs/status/1745533306492588303](https://x.com/abidlabs/status/1745533306492588303)), the team of five engineers shut down their startup and joined HF — a team-and-IP acqui-hire. The acquisition gave HF the substrate for **Spaces** (their hosted demo platform) and gave Gradio a permanent commercial home. By Trail of Bits' 2024 measurement ([blog.trailofbits.com](https://blog.trailofbits.com/2024/10/10/auditing-gradio-5-hugging-faces-ml-gui-framework/)), Gradio averaged **6.1M PyPI downloads/month** and was the GUI under Stable Diffusion WebUI (141k stars) and text-generation-webui (40k stars) — making it the dominant ML-demo GUI library in the open-source LLM/diffusion ecosystem.

### 1.2 Version-by-version inflection points

**1.x — Interface era (2019 – early 2021).** The paper-era API. One abstraction:

```python
gr.Interface(fn, inputs, outputs).launch()
```

Three lines, full demo. The mental model is a Python callable in the middle, declarative input components on the left, output components on the right, templated layout that you do not get to argue with. 1.x had effectively zero layout vocabulary — by design, not oversight: the goal was to remove decisions, not expose them.

**2.x — Hardening (2021).** Production polish. Hugging Face integration deepened (`gr.Interface.load()` for HF Hub models per [huggingface.co/blog/gradio](https://huggingface.co/blog/gradio)), themes and styling primitives expanded, the queue model started taking shape. Architecturally additive; 1→2 cost-of-change was low.

**3.x — Blocks: the architectural pivot (2022-05-16).** The inflection point. Per the [Gradio 3.0 announcement](https://huggingface.co/blog/gradio-blocks), two simultaneous changes:

1. **Frontend rewrite to Svelte** ("smaller payloads and much faster page loads"). Earlier versions used a non-Svelte frontend that bottlenecked component richness.
2. **The Blocks API** — context-manager-scoped graph builder, lifting three named `Interface` limitations: layout, multi-step pipelines, and dynamic component-property changes ("change the choices in a Dropdown based on user input").

```python
with gr.Blocks() as demo:
    with gr.Tabs():
        with gr.TabItem("Flip Text"):
            text_input = gr.Textbox()
            text_output = gr.Textbox()
            text_input.change(flip_text, inputs=text_input, outputs=text_output)
```

The mental model shifts from *function-with-decorated-inputs* to **components-as-graph-nodes wired by event handlers**. `Interface` survives as a high-level wrapper compiling down to a Blocks graph. `gr.ChatInterface` later in the 3.x line (~3.34/3.35, mid-2023; PR [#3370](https://github.com/gradio-app/gradio/pull/3370)) followed the same pattern — high-level wrapper for the universally-recurring chat-with-streaming-LLM case. By the LLM explosion of 2023, Gradio had a one-liner for the dominant UI shape of the moment, which is half the explanation for its post-2023 dominance. Cost to existing users: modest — `Interface` kept working — but conceptual.

**4.x — Custom Components (2023-10-31).** The "open the box" release ([issue #6339](https://github.com/gradio-app/gradio/issues/6339)). Headline feature: **Custom Components** — a `create`/`dev`/`build`/`publish` workflow for shipping a pip-installable Python class + Svelte frontend pair. The breaking changes were the most aggressive in Gradio's history:

- `Carousel`, `StatusTracker`, `Box`, `Variable` — removed.
- `.style()` and `.update()` methods — removed.
- `gr.Series`, `gr.Parallel`, `Interface.load()`, `enable_queue` on `.launch()` — removed.
- Global `concurrency_count` replaced by per-event `concurrency_limit` (default 1) — every existing app's queue capacity changed semantics.
- Working directory no longer auto-served; explicit `allowed_paths` required (security tightening).
- **Gradio 4.x cannot load Spaces running 3.x** — a hard backward-compatibility break.

Cost to existing users: high.

**5.x — SSR, perf, security audit (2024-10-09).** Framed as stability-and-perf rather than architectural. SSR default-on (Node 20+ required, falls back to CSR) eliminated the persistent loading-spinner UX complaint. Modern design refresh; low-latency streaming over WebSocket; experimental "AI Playground" for LLM-generating Gradio apps ([gradio.app/playground](https://www.gradio.app/playground)). Trail of Bits security audit found 8 high-severity issues (CORS, SSRF, file-leak via post-processing, RCE on Gradio API server via Docker exposure, unencrypted FRP traffic) — all addressed before 5.0 ship. Component breakages: `gr.Audio` no longer auto-converts to `.wav`; `gr.LogoutButton` removed; `gr.DataFrame.height` → `max_height`; `gr.Row.equal_height` defaults flipped to `False`; `gr.Chatbot.likeable` removed; `every` parameter replaced by `gr.Timer`; `concurrency_count` removed entirely. Cost: moderate.

**6.x — In progress.** The [Gradio changelog](https://www.gradio.app/changelog) shows 6.x already shipping (Svelte 5 component migration, `gradio.Server` "server mode," `@gr.cache()` decorator, HTML-as-layout-element). For the purposes of this doc: **assume 12–18 month major-version churn.**

### 1.3 Architecture

A live Gradio app is two processes glued by a known protocol.

**Backend.** A FastAPI server (Uvicorn) hosts the Python app. Each component declared in `Blocks` is a Python object with `preprocess(raw)` and `postprocess(value)` methods spanning the JSON-on-the-wire ↔ Python-in-the-function boundary. Event handlers (`btn.click(fn, inputs=[...], outputs=[...])`) register edges in a function-dispatch graph compiled at `.launch()`.

**Frontend.** A Svelte SPA (since 3.0) served by the same FastAPI process; SSR-rendered when Node 20+ is available. The frontend learns the component tree from a `/config` endpoint at startup and renders Svelte components keyed by component class name. Components are addressed by **integer IDs** assigned at graph-build time — relevant for the type-safety discussion below.

**Wire protocol.** Predictions dispatch over WebSocket through a queue. Direct HTTP `/run/predict` exists; the queue is canonical for any non-trivial app.

**Queue, concurrency, dispatch.** Per [gradio.app/guides/queuing](https://www.gradio.app/guides/queuing): every event listener has its own queue; concurrency is governed by per-listener `concurrency_limit` (default 1) and `concurrency_id` (lets multiple listeners share a queue, e.g., "all three image-gen functions share one GPU"). A worker thread pool executes from queues. `concurrency_limit=None` removes the cap. `batch=True` lets functions receive lists for vectorized inference.

**Share-link tunneling.** On `share=True`, Gradio downloads a precompiled FRP (Fast Reverse Proxy) client, opens a TLS tunnel to `gradio.live`, and gets a routable subdomain — links expire in 72h. Pre-3.x used SSH tunneling; PR [#2509](https://github.com/gradio-app/gradio/pull/2509) switched to FRP for reliability. Self-hosting is supported via HF's [FRP fork](https://github.com/huggingface/frp).

**Component model.** Components are Python classes with `preprocess`/`postprocess`/`api_info`, paired to a Svelte component on the frontend keyed by class name. The set spans rich types (`Image`, `Audio`, `Video`, `Dataframe`, `Chatbot`, `Gallery`, `Plot`, `Model3D`) plus primitives. Layout components (`Row`, `Column`, `Tab`, `Group`, `Accordion`) are themselves graph nodes carrying children but no preprocess/postprocess.

Inputs/outputs map to function args **by position**: `btn.click(fn, inputs=[a, b, c], outputs=[x, y])` calls `fn(a_val, b_val, c_val)` and unpacks the returned tuple. A function may also return `gr.update(...)` — an untyped dict — to mutate component properties (visibility, choices, etc.).

**State.** `gr.State` is an invisible component carrying a per-session in-memory Python value (Gradio assigns a session hash to the WebSocket; the value persists within that session and dies when it closes). No global mutable state by convention; for shared state, use Redis/DB. Each prediction call is per-request stateless from the function's POV.

### 1.4 Where Gradio falls below Vox's bar

**Layout rigidity.** Row/Column with `Tab`, `Group`, `Accordion` is the entire structural vocabulary. No native multi-page navigation; no CSS-grid/flexbox-precise control; no responsive breakpoints in the model. Cross-comparisons consistently call this out ([uibakery.io/blog/streamlit-vs-gradio](https://uibakery.io/blog/streamlit-vs-gradio), [evidence.dev/learn/gradio-vs-streamlit](https://evidence.dev/learn/gradio-vs-streamlit)). The 5.0 `equal_height` default flip is a tell — they are tuning defaults inside a fixed vocabulary rather than expanding it.

**Theming surface.** A single `gr.Theme` object with fixed CSS-variable knobs; beyond that you write raw CSS via `Blocks(css=...)`. The 5.0 split of `button_shadow` into `button_primary_shadow` / `button_secondary_shadow` shows per-variant control being added one knob at a time.

**Type safety, three layers.**

- Inputs/outputs match by **list position**, not by name. Refactor a `Blocks` graph and you can silently swap two inputs.
- `gr.update(...)` is an untyped dict; the frontend does partial-property merging with no static schema.
- The auto-generated `/run/predict` schema reflects whatever components advertise — and component preprocessing accepts varied shapes (`gr.Image` takes PIL/numpy/path/bytes depending on `type=`).

Fine in 30-line demos; the dominant pain in 500-line apps.

**Performance under scale.** Pre-5.0, every app shipped a heavy client bundle and a loading spinner. SSR addresses this. The queue scales to "thousands of concurrent users" per the docs but has well-trodden footguns ([#4399](https://github.com/gradio-app/gradio/issues/4399), [#4841](https://github.com/gradio-app/gradio/issues/4841)): forgetting `concurrency_limit` serializes through 1 worker; setting it too high with GPU functions exhausts VRAM. The per-app scaling story is "deploy a copy per use case."

**Customization is Svelte-locked.** Custom Components 4.0 was a real step up but the workflow is heavy: Python class + Svelte component + package metadata + build + publish. You cannot write a custom component in React or vanilla JS without compiling through the Svelte build chain. Every Gradio major version may invalidate existing component builds ([#12074](https://github.com/gradio-app/gradio/issues/12074)).

**Major-version churn.** 3→4 was harsh; 4→5 milder but real (`concurrency_count` removal, `every` → `gr.Timer`, API route prefixing, file/CORS tightening); 6.x ships with a fresh migration guide. **Operational reality: 12–18 month major-version churn.**

**Accessibility floor.** No published WCAG/ARIA commitments; no auditor's report. Components have basic keyboard nav and labels. The Trail of Bits audit was security, not a11y. The 5.0 redesign did not surface a11y as a feature line, which is itself signal.

### 1.5 What Gradio gets right

- **Time-to-first-demo.** Three lines. Has not been beaten.
- **Block-as-graph mental model.** Once you accept it, `Blocks` is genuinely good design: every UI element is a node, every interaction is an edge, the function in the middle is just Python. Same `with` block lexically scopes the graph and renders the layout. Conceptually load-bearing, has held across two more major versions without revision.
- **HF Spaces deploy.** `git push` to a Space repo with `app.py` is the deploy. Always-on hosting, free CPU, paid GPU, public URL, automatic scaling. No frontend build, no Docker, no DNS. The dominant ML-demo distribution channel in the world.
- **`gr.ChatInterface`.** One-liner for streaming-token chat with history, retry/undo/clear, additional-inputs accordion, optional multimodal. The right abstraction for the right shape at the right time.
- **Gradio Lite ([huggingface.co/blog/gradio-lite](https://huggingface.co/blog/gradio-lite), 2023-10-19).** Whole Gradio app in the browser via Pyodide. 5–15s initial load, then zero-RTT. Real privacy story (data never leaves device), zero hosting cost, embeddable in static pages. Limited by Pyodide's package universe.

---

## 2. Streamlit

### 2.1 Origin and provenance

Streamlit was co-founded in 2018 by **Adrien Treuille** (CEO), **Thiago Teixeira** (CTO), and **Amanda Kelly** (COO). Treuille is a CS PhD who was a CMU associate professor (research on computer graphics, crowd simulation, and crowdsourced science — co-founder of citizen-science projects **Foldit** and **Eterna**), led an AI group at **Google X**, and was **VP of Simulation at Zoox**. The founder trio's collaboration goes back to a 2012 Google X project; the team's professional context was building visualization and ML tooling for self-driving programs (Treuille at Zoox; Teixeira's prior roles included Google X). The popular "originally at Uber" shorthand is imprecise — the canonical origin is Google X plus Zoox; Uber's ML-tooling community was an early adopter, not the employer.

The original problem, per Treuille's launch essay (October 2019, originally on Towards Data Science / `blog.streamlit.io`, now archive-only): ML engineers at AV/ML teams routinely spent weeks gluing **Flask + Jinja + JavaScript + bespoke React** to ship an internal review tool for a model — work entirely orthogonal to the model. The team built and discarded a series of internal frameworks at Zoox/Google X before generalizing one. The pitch: **all you need is a Python script.** The core commitment was to remove every concept the script author had to learn that wasn't already Python — no callbacks, no decorators-for-routing, no template language, no separate frontend file.

Open-source release: **2019-10-01**, alongside a Sequoia-led Series A. Snowflake announced acquisition on **2022-03-02** ([Snowflake press release](https://www.snowflake.com/en/news/press-releases/snowflake-announces-intent-to-acquire-streamlit-to-empower-developers-and-data-scientists-to-mobilize-the-worlds-data/), [TechCrunch](https://techcrunch.com/2022/03/02/snowflake-acquires-streamlit-for-800m-to-help-customers-build-data-based-apps/)); reported price ~$800M, audited cash purchase ~$710M after working-capital adjustments per Snowflake's later 10-Q. Post-acquisition, Streamlit stayed Apache-licensed and OSS — visible product changes: **Streamlit in Snowflake** as a deployment target, `st.connection` with `SnowflakeConnection`, Treuille's expanded Snowflake Director-of-Product-GenAI role. OSS release cadence (3-week minor cycle on `develop`), API surface, and execution model were unchanged.

### 2.2 The execution model

**Run the script top-to-bottom on every interaction.** The single most important architectural decision in Streamlit. When any widget changes, Streamlit re-executes the entire Python script from line 1. There are no callbacks (in the Dash sense), no observers, no `useState`. The script is the source of truth; widgets read like variables (`x = st.slider(...)` simply binds `x` to the current value). The mental model is **a notebook cell that reruns whenever an input changes** — directly inherited from the IPython/Jupyter idiom.

**Widget identity.** Streamlit assigns each widget call a stable widget ID by hashing source location, widget type, and selected parameters (label, options, default). Authors override with explicit `key=`; the key is also the slot in `st.session_state`. Two widgets with the same effective key collide (`DuplicateWidgetID`).

**`st.session_state`** — introduced in **0.84.0 (2021-07-01)** with widget callbacks (`on_change`). Before this, the rerun-from-top model had no first-class place for "state that survives a rerun but isn't a widget"; the standard workaround was the unofficial `streamlit-server-state` package and various global-dict hacks. `st.session_state` solved (i) ephemeral cross-rerun state, (ii) reading/writing widget values by key, (iii) callback timing — `on_change` runs *before* the rerun, with the new value already in session_state.

**Caching evolution.**

- Original `@st.cache` (2019). One decorator, hash-by-arguments-and-source, returned-by-reference. Famously confusing: it tried to be both "memoize this pure computation" *and* "this is a global resource (DB connection, model)" with the same primitive, leading to mutation warnings and `UnhashableTypeError` storms.
- `@st.experimental_memo` and `@st.experimental_singleton` introduced in **0.89.0 (2021-09-22)** to split the two intents.
- Promoted and renamed in **1.18.0 (2023-02-09)**: `@st.cache_data` (replaces `experimental_memo`; copies on return — safe for dataframes) and `@st.cache_resource` (replaces `experimental_singleton`; returns by reference — for connections, models). `@st.cache` deprecated and aliased.

**Rerun primitive.** `st.experimental_rerun` (0.x) → **`st.rerun` GA in 1.27.0 (2023-09-21)**.

**Fragments — partial reruns.** Long-standing pain: even a tiny widget change reran the whole expensive script. **`st.experimental_fragment` in 1.33.0 (2024-04-04)** scopes a function so widget interactions inside it only rerun *that* function. **`st.fragment` GA in 1.37.0 (2024-07-25)**, with nested fragments and callback support. This is the framework's most direct concession that the pure rerun-from-top model had architectural limits.

**Multipage apps.** Original convention (1.10.0, 2022-06-02): a `pages/` directory whose `.py` files become pages with auto-generated sidebar nav, alphabetical ordering (numeric prefixes the de-facto convention). Newer API: **`st.navigation` + `st.Page` in 1.36.0 (2024-06-20)** — declarative, group/label/icon control, dynamic page sets. `pages/` still works; `st.navigation` is recommended.

**Forms.** `st.form` and `st.form_submit_button` in **0.81.1 (2021-04-29)**. Direct response to the "every keystroke triggers a rerun" surprise — batched widget edits, rerun only on submit. Effectively the imperative-script equivalent of "controlled vs. uncontrolled inputs."

### 2.3 Architecture

**Server: Tornado.** Single Python process; `tornado.web.Application` with a `WebSocketHandler`. Each browser tab opens one WebSocket; the server holds an `AppSession` per connection containing a `ScriptRunner` (user code on its own thread), `SessionState`, and a per-session message queue. Unchanged from launch.

**Wire protocol.** Bidirectional WebSocket, **Protocol Buffers**. Two messages: `ForwardMsg` (server → client; deltas, session events, navigation; protobuf union over ~50+ element types) and `BackMsg` (client → server; rerun requests with widget-state snapshot, file uploads, theme info).

**`DeltaGenerator`.** The implementation behind `st`. Every public command is a method that constructs a protobuf `Element` and enqueues a `ForwardMsg`. Containers (`st.columns`, `st.expander`, `st.sidebar`) return *child* DeltaGenerators with paths into the layout tree. This is how the "virtual DOM" gets built — but it is not a diffing VDOM; it is an append-only stream of deltas keyed by tree-path that the React frontend reconciles.

**`ScriptRunner`.** Thread per script run. Each rerun: clears (or reuses) the in-memory element tree, executes the user module from the top, catches exceptions, finalizes. The *previous* rerun's thread is signalled to stop via a `RerunException` — old work is abandoned, not awaited. This is the source of various stop-the-world semantics.

**Frontend.** React + TypeScript. `App.tsx` orchestrates; `AppRoot` holds main/sidebar/event/bottom containers. `ElementNodeRenderer` maps protobuf element types to React components. `WidgetStateManager` owns widget values and form-batching. The bundle ships as static assets from the same Tornado process.

**Custom Components API ([blog.streamlit.io](https://blog.streamlit.io/introducing-streamlit-components-d73f2092ae30), 2020-07-14).** Three primitives in `streamlit.components.v1`: `html(...)` for inline HTML/JS/CSS, `iframe(...)` for embedding URLs, and `declare_component(...)` for fully bidirectional widgets. The third runs the component's code in a **sandboxed iframe** that exchanges JSON with the host page (and via the host page, the Python process). The iframe-isolation choice was deliberate — security from arbitrary npm dependencies — but is the source of much of the extensibility criticism: every component carries its own React tree and its own postMessage handshake.

### 2.4 Where Streamlit falls below Vox's bar

**The rerun-from-top surprise.** Elegant model, undersold. New users hit it the first time a slider triggers a 30-second pandas pipeline; the diagnostic ("add `@st.cache_data`") is real but not discoverable from a stack trace. Dominant theme on the [Streamlit forum performance pain-points thread](https://discuss.streamlit.io/t/what-are-your-performance-pain-points-with-streamlit/8218).

**Performance fragility at scale.** Without aggressive `cache_data`/`cache_resource`/`fragment` discipline, app interactivity is `O(slowest top-level statement)`. The 1.33 fragment release notes name this directly.

**"Magic."** Top-level expressions that aren't assigned auto-render (a bare `df` on its own line). Lovely for demos, surprising in real codebases — disabled by `magic_enabled = false` in production. It is a DSL pretending not to be one: the file is valid Python, but its semantics depend on top-level-side-effect rules a linter doesn't know.

**Layout limits.** `st.columns`, `st.container`, `st.expander`, `st.tabs`, `st.sidebar` is the vocabulary. Nested columns were impossible until late 2022; nested fragments needed 1.37; CSS overrides require either Custom Components or `st.markdown(unsafe_allow_html=True)` hacks. No first-class flexbox/grid escape hatch.

**State-synchronization edge cases.** Setting `st.session_state[key]` *and* passing `value=` is an error; `on_change` fires before the rerun (so reading other widgets in the callback sees stale values); `st.experimental_rerun` inside a callback was undefined behavior for a long time.

**Cache footguns.** `@st.cache_data` deep-copies returns — correct but expensive. `@st.cache_resource` returns by reference; mutating a cached object silently corrupts other sessions. Invalidation is by argument hash plus source-code hash; closures over outer state aren't part of the key, leading to "stale cache" bugs.

**Custom Components ergonomics.** Each component is a full sandboxed iframe with its own React tree and a postMessage handshake. Compared to Gradio's component model (components are first-class types in the host React app, no iframe), Streamlit's bidirectional component is significantly heavier — bigger bundles, slower paint, no shared theming without manual CSS variables. Real isolation, real cost.

**Composition vs. monolith.** Because the script *is* the unit of execution, Streamlit apps trend toward a single 2000-line `app.py`. There is no component model in the React sense; `def my_widget(...)` is just a function whose effects rely on caller context (containers, session_state, key prefixes). Reusable widget libraries exist but require key-namespacing discipline the framework doesn't help with.

**Type safety.** Widget return values are typed by the call (`int`, `str`, `datetime.date`), but `st.session_state["foo"]` is `Any`. There is no static binding between a widget's `key=` and its session-state slot; a typo is a runtime KeyError or, worse, a silent default.

### 2.5 What Streamlit gets right

- **Time-to-first-app for a single user.** `pip install streamlit; streamlit run app.py` produces a working app whose source is recognizable as Python. Lowest-friction path that exists.
- **Caching that solves a real problem.** Once `cache_data`/`cache_resource` were split, the decorator pair captures ~90% of "make this app fast enough" cases without forcing users to learn a queue or a worker model.
- **Magic + imperative reads well to non-frontend Pythonistas.** `st.write(df)` does the right thing for ~15 different inputs (DataFrame, dict, plot, markdown, exception). Discoverability high, ceremony low.
- **Streamlit in Snowflake.** App runs inside the warehouse's security boundary with zero network egress for queries. Unique distribution channel.
- **Chat primitives shipped early.** `st.chat_message` / `st.chat_input` (1.24, 2023-06) and `st.write_stream` (1.31) made Streamlit the default LLM-demo framework throughout 2023–2024.

---

## 3. Side-by-side comparison

| Axis | Gradio | Streamlit |
|---|---|---|
| **Execution model** | Function-dispatch graph; per-event preprocess→fn→postprocess | Whole-script rerun on every interaction; fragments scope partial reruns |
| **State** | `gr.State` per session; no global by convention | `st.session_state` per session; cache decorators for cross-session |
| **Layout** | `Row`/`Column`/`Tab`/`Group`/`Accordion` | `columns`/`container`/`expander`/`tabs`/`sidebar` |
| **Theming** | `gr.Theme` knobs + raw CSS escape | CSS-variable hacks + `st.markdown(unsafe_allow_html=True)` |
| **Deployment** | HF Spaces (canonical), self-host | Streamlit Community Cloud, Snowflake, self-host |
| **Extensibility** | Custom Components: Python + Svelte, build & publish | Custom Components: Python + sandboxed iframe (any JS framework) |
| **Type safety** | Positional input/output binding; untyped `gr.update` | Untyped session_state; `key=` strings unbound from declaration site |
| **Testability** | None first-class until 4.x debug tooling | `AppTest` framework added in 1.28 |
| **Rendering target** | Svelte SPA + SSR (5.0+) | React SPA, no SSR |
| **Wire protocol** | WebSocket + JSON, queue-based | WebSocket + Protobuf, delta stream |
| **Multipage** | Promised in 6.x roadmap | `pages/` since 1.10; `st.navigation` since 1.36 |
| **Chat UI primitive** | `gr.ChatInterface` (one-liner) | `st.chat_message` + `st.chat_input` + `st.write_stream` |

The big architectural divide: Gradio is a **graph**; Streamlit is a **script**. Everything else falls out of that.

---

## 4. What translates well to LLM-authored UI

Both libraries are heavily emitted by current LLMs. The shared properties that make them LLM-friendly:

- **Single-file executable.** `app.py` is the unit. No `package.json`, no separate frontend, no `tsconfig`, no build step. LLMs are good at single-file outputs. Both deployment models (HF Spaces, Streamlit Community Cloud) reinforce this: the file *is* the app.
- **Tiny semantic API surface.** Gradio's ~50 component classes and Streamlit's ~150 `st.*` functions both have semantic, stable names that map directly to natural-language requests ("build me a chatbot," "take an audio input"). Pattern density is high; the model has seen thousands of canonical examples.
- **Python-only.** No JS/HTML/CSS context-switching for the model until you go custom.
- **Stylized public corpus.** HF Spaces (~hundreds of thousands of canonical Gradio apps) and Streamlit's Galleries/GitHub provide enormous high-quality training data. Few-shot prompting works.
- **Vendor-shipped LLM codegen surfaces.** Gradio's [AI Playground](https://www.gradio.app/playground) is one of very few GUI libraries with a vendor-shipped codegen UI.

Properties more specific to each:

- **Gradio's Blocks are local.** A `with gr.Blocks() as demo: ... .click(...)` block is a self-contained graph; the model can see all the wiring in one screen of code.
- **Streamlit's imperative dataflow matches token order.** Each `st.*` call describes "next thing on the page" — output sequence ≈ visual sequence. No forward references, no callback graphs, no JSX nesting depth. Local coherence in the model maps cleanly to local coherence on the page.
- **Streamlit's magic + `st.write`** collapses the type system: the model can write `st.write(result)` without committing to whether `result` is a DataFrame, a string, or a Plotly figure.

---

## 5. What does *not* translate

Failure modes that bite particularly hard under LLM authorship:

- **Major-version churn poisons training data.** Both libraries have this. A model trained on Gradio 4.x emits `concurrency_count`, `Interface.load()`, `.style()`, `.update()`, `enable_queue=True`, `every=...` — all dead. Streamlit's `st.cache` → `cache_data`/`cache_resource`, `experimental_rerun` → `rerun`, `experimental_fragment` → `fragment`, `pages/` → `st.navigation`. Models trained pre-migration cheerfully emit deprecated forms; mid-migration models emit a mix.
- **Position-based binding (Gradio).** `inputs=[a, b, c]` matches function args by position. When a model rearranges code, it can silently swap inputs without a type error.
- **Untyped update dicts (Gradio).** `gr.update(visible=True, choices=[...])` is an untyped dict; the model can invent invalid keys with no signal.
- **Hidden cross-rerun state (Streamlit).** A model reading `app.py` cannot tell from the file whether `st.session_state["foo"]` was set on a previous rerun, by which widget, in which page. Bugs that look like "this variable is None" require reasoning across runs that the source text doesn't carry.
- **Magic is non-locally typed (Streamlit).** A bare `df` at top level renders if the file is `app.py`, is dead code if it's a helper module — context the model must track.
- **Key collisions across files (Streamlit).** Multipage apps with shared widget keys silently break (`DuplicateWidgetID` only fires within one rerun). An LLM editing `pages/2_settings.py` can't see keys defined in `pages/1_main.py` without reading both, and absent project convention, models routinely emit `key="filter"` from both pages.
- **Cache-invalidation reasoning is global (Streamlit).** Whether `@st.cache_data` is correct depends on whether the wrapped function captures any closure variable not in its argument list — local property, global consequences.
- **State-management ambiguity (Gradio).** `gr.State` vs module-level variable vs external store — docs are clear, wrong choice is silently wrong (per-session vs cross-session vs cross-process), LLMs frequently choose poorly.
- **Customization is the wall.** Both libraries' Custom Components stories collapse the LLM-friendly story: the model is suddenly authoring Svelte (Gradio) or sandboxed-iframe React with a postMessage handshake (Streamlit), with a build toolchain to learn.

---

## 6. Lessons for VUV — keep, adapt, reject

The following is the explicit mapping. "Source" is the library the lesson is drawn from. "Disposition" is what VUV should do with it. "Where it lands" points to the existing Vox surface where the decision is, or will be, expressed.

| Principle | Source | Disposition | Why | Where it lands |
|---|---|---|---|---|
| Single-file authorable view | both | **Keep** | LLM-friendliness; matches Vox's `component`-in-a-file model | Already true; component + view in one `.vox` file |
| Tiny semantic primitive vocabulary | both | **Keep** | LLM-friendliness; reduces K-complexity | VUV typed primitives ([gui-authoring-syntax-2026.md](gui-authoring-syntax-2026.md) Rule 1) |
| Function-call view syntax (no JSX, no template) | Streamlit imperative | **Adapt** | Removes hosted sub-languages (JSX brackets, attribute aliasing); Vox already chose this in VUV | VUV Rule 1 — view-as-expression |
| **Named** props instead of positional binding | reject Gradio | **Reject Gradio's positional model; require named** | Refactor-safe; LLM-rearrangement-safe; matches Vox's named-arg call convention everywhere | VUV Rule 1 — every prop is a named kwarg |
| Typed style kwargs (no class strings, no inline CSS) | reject both | **Reject Tailwind-string and `unsafe_allow_html` patterns; require typed kwargs from token registry** | Eliminates the largest single source of LLM mistakes (~1000 Tailwind tokens); keeps Tailwind/CSS as a *backend* not a *surface* | VUV Rule 2 — typed token kwargs; `web_ir/validate.rs` enforces |
| Typed event kwargs with single naming convention | reject both | **Reject the `on_click`/`on:click`/`onClick` decision burden** | One naming form everywhere | VUV Rule 3 — `on_click: fn() -> Action` |
| Layout primitives as first-class typed values | reject Gradio rigidity | **Adapt** — keep Row/Column-style primitives, but with typed style kwargs and a real composition model (responsive variants, nesting without ceremony) | Gradio's Row/Column is the right starting vocabulary; the failure was leaving it stuck rather than expanding it | VUV-1 token vocabulary expansion (justify, align, max-w, pad, gap, breakpoints) |
| High-level wrapper for the chat-shape | both, esp. `gr.ChatInterface` | **Adapt** | The shape is universal in 2023–2026 LLM apps; ship it as a built-in primitive in the dashboard or a stdlib component | Candidate for a `vox-stdlib`-shipped component or a primitive in the VUV vocabulary |
| Function-as-graph-node mental model (Blocks) | Gradio | **Adapt** | Genuinely good model for "a UI is interactions wired between components"; Vox's component + reactive lowering is already this | `codegen_ts/reactive.rs` already lowers reactive members to React hooks — same shape |
| Whole-script rerun-on-interaction | reject Streamlit | **Reject** | The hidden-state failure mode is intolerable for LLM maintenance; we already have a graph model | Reactive primitives (`HirReactiveComponent`); not a script |
| **Magic** (auto-render top-level expressions) | reject Streamlit | **Reject** | Non-locally typed; lints don't see it | VUV is explicit — `view: ...` is a named binding |
| `st.write`-style polymorphic catch-all | reject Streamlit | **Reject** | Collapses the type system; no static guarantees | Vox is statically typed; render functions are typed by primitive |
| Single global cache decorator | reject Streamlit's old `@st.cache` | **Reject; use intent-split caches if any** | The 2023 split into `cache_data`/`cache_resource` shows the failure mode | Use Vox's existing memoization story; do not unify cross-intent |
| Custom Components: same-language extension | reject both lock-ins | **Adapt** — Vox components and React components are first-class peers | Both libraries' Custom Components stories collapse the LLM-friendly story; we already chose bidirectional component interop | Phase 5 of [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md) |
| Stable major-version surface | both fail this | **Adopt as a discipline** | 12–18 month churn on component class names, parameter names, and decorator names taxes the entire training corpus | Wire format SSOT ([Phase 2](external-frontend-interop-plan-2026.md#phase-2--wire-format-ssot-and-standards-based-schema-emit)); stable component names; rename only with deprecation cycle |
| `share=True` magic deploy | Gradio | **Adapt later** | Genuinely the killer feature for time-to-share; not a VUV-2026 concern, but the design space ("git push and have a URL") should not be foreclosed | Future deploy-mode work; out of VUV scope |
| Per-session state primitive (`gr.State`) | Gradio | **Adapt** | Cleaner than Streamlit's session_state (typed, not a stringly-keyed dict) | Vox's reactive state already resembles this; keep typed |
| WebSocket + binary wire | both | **Adapt** | Both chose WebSocket; Streamlit chose protobuf, which is the right call for a typed system | Vox's wire-format SSOT ([Phase 2](external-frontend-interop-plan-2026.md#phase-2--wire-format-ssot-and-standards-based-schema-emit)) |
| First-class accessibility | both fail this | **Adopt as a quality bar** | Neither library publishes WCAG/ARIA commitments; Vox can do better with typed primitives carrying a11y attributes by default | VUV-6 a11y attribute typing (in-flight) |
| First-class testability | Streamlit `AppTest` (1.28) | **Adapt** | Renders should be unit-testable; we already have golden tests for VUV emit, extend to runtime | Existing `crates/vox-compiler/tests/golden_*` infrastructure |

The pattern, summarized in a sentence: **VUV should keep the parts of Gradio's graph and Streamlit's single-file imperative that make both LLM-friendly, replace every string-typed sub-language with typed kwargs from the token registry, and lean on Vox's existing component + reactive + Web IR substrate to do the work both libraries had to invent and re-invent across major versions.**

---

## 7. Open questions and follow-ups

These surface from the research but are deliberately not answered here. Each is a candidate for its own spec.

1. **Does VUV need a Streamlit-equivalent "magic" for prototyping?** A `vox demo` mode where bare expressions render to a default primitive. Tradeoff: ergonomics at the cost of explicitness. Likely no, but worth a deliberate decision rather than passive-rejection-by-omission.
2. **Should `vox-stdlib` ship a `ChatInterface`-equivalent?** The shape is universal enough in 2023–2026 to deserve a built-in. What does it cost in lock-in? Where does it live (stdlib component vs. dashboard app vs. external package)?
3. **What is Vox's equivalent of `share=True`?** Gradio's killer feature. Likely lives at the `vox deploy` layer, not at VUV. Doc somewhere in deploy roadmap.
4. **How does VUV handle the per-session state primitive?** Reactive state is per-session by default; do we need an explicit `@session` decorator for state that survives rerenders but not page loads? Compare with `gr.State` and `st.session_state` directly.
5. **Cache discipline.** Streamlit's `cache_data`/`cache_resource` split codifies a real distinction. Do we need a Vox equivalent? Where does it live (decorator on `fn`, decorator on `view`, neither)?
6. **Migration corpus discipline.** ✅ **Resolved by VUV-9 (landed 2026-05-09).** See [VUV-9 Implementation Plan](../../superpowers/plans/tooling/2026-05-08-vuv-improvement-roadmap.md) and the [Naming Policy](vuv-naming-policy-2026.md). Original question: Both libraries demonstrate that major-version churn poisons training data. What is Vox's policy for renaming a primitive, a kwarg, or a decorator? (Probably: never silently; deprecation alias for at least one major; explicit codemod.)

---

## Citations

### Gradio
- Original paper — [arxiv.org/abs/1906.02569](https://arxiv.org/abs/1906.02569)
- Acquisition announcement — [huggingface.co/blog/gradio-joins-hf](https://huggingface.co/blog/gradio-joins-hf)
- Acquisition retrospective — [x.com/abidlabs/status/1745533306492588303](https://x.com/abidlabs/status/1745533306492588303)
- Gradio 3.0 / Blocks — [huggingface.co/blog/gradio-blocks](https://huggingface.co/blog/gradio-blocks)
- Gradio 4.0 breaking changes — [github.com/gradio-app/gradio/issues/6339](https://github.com/gradio-app/gradio/issues/6339)
- Gradio 5 announcement — [huggingface.co/blog/gradio-5](https://huggingface.co/blog/gradio-5)
- Gradio 5 migration — [github.com/gradio-app/gradio/issues/9463](https://github.com/gradio-app/gradio/issues/9463)
- Trail of Bits security audit — [blog.trailofbits.com](https://blog.trailofbits.com/2024/10/10/auditing-gradio-5-hugging-faces-ml-gui-framework/)
- Gradio Lite launch — [huggingface.co/blog/gradio-lite](https://huggingface.co/blog/gradio-lite)
- Share-link / FRP guide — [gradio.app/guides/understanding-gradio-share-links](https://www.gradio.app/guides/understanding-gradio-share-links)
- FRP switch (PR #2509) — [github.com/gradio-app/gradio/pull/2509](https://github.com/gradio-app/gradio/pull/2509)
- HF FRP fork — [github.com/huggingface/frp](https://github.com/huggingface/frp)
- Queueing guide — [gradio.app/guides/queuing](https://www.gradio.app/guides/queuing)
- ChatInterface PR — [github.com/gradio-app/gradio/pull/3370](https://github.com/gradio-app/gradio/pull/3370)
- Gradio 6 migration guide — [gradio.app/main/guides/gradio-6-migration-guide](https://www.gradio.app/main/guides/gradio-6-migration-guide)
- Streamlit/Gradio comparisons — [uibakery.io/blog/streamlit-vs-gradio](https://uibakery.io/blog/streamlit-vs-gradio), [evidence.dev/learn/gradio-vs-streamlit](https://evidence.dev/learn/gradio-vs-streamlit)
- Concurrency issue — [github.com/gradio-app/gradio/issues/4399](https://github.com/gradio-app/gradio/issues/4399)
- Custom Components revisit — [github.com/gradio-app/gradio/issues/12074](https://github.com/gradio-app/gradio/issues/12074)

### Streamlit
- Snowflake acquisition press release — [snowflake.com/.../snowflake-announces-intent-to-acquire-streamlit](https://www.snowflake.com/en/news/press-releases/snowflake-announces-intent-to-acquire-streamlit-to-empower-developers-and-data-scientists-to-mobilize-the-worlds-data/)
- TechCrunch acquisition coverage — [techcrunch.com](https://techcrunch.com/2022/03/02/snowflake-acquires-streamlit-for-800m-to-help-customers-build-data-based-apps/)
- GGV Founder Real Talk Ep. 43 — [founderrealtalk.ggvc.com/2020/10/15/episode-43-streamlit/](https://founderrealtalk.ggvc.com/2020/10/15/episode-43-streamlit/)
- 0.84.0 announcement (session_state) — [discuss.streamlit.io/t/version-0-84-0/14542](https://discuss.streamlit.io/t/version-0-84-0/14542)
- Custom Components launch — [blog.streamlit.io/introducing-streamlit-components-d73f2092ae30](https://blog.streamlit.io/introducing-streamlit-components-d73f2092ae30)
- Release notes index — [docs.streamlit.io/develop/quick-reference/release-notes](https://docs.streamlit.io/develop/quick-reference/release-notes)
- Architecture docs — [docs.streamlit.io/develop/concepts/architecture/architecture](https://docs.streamlit.io/develop/concepts/architecture/architecture)
- Performance pain-points thread — [discuss.streamlit.io/t/...performance-pain-points/8218](https://discuss.streamlit.io/t/what-are-your-performance-pain-points-with-streamlit/8218)
- "Pushing the boundaries of Streamlit" — [twitchard.github.io/posts/2024-11-27-streamlit.html](https://twitchard.github.io/posts/2024-11-27-streamlit.html)
- Issue #12980 — real-time updates without full rerun — [github.com/streamlit/streamlit/issues/12980](https://github.com/streamlit/streamlit/issues/12980)
- Issue #3930 — postpone rerun of whole script — [github.com/streamlit/streamlit/issues/3930](https://github.com/streamlit/streamlit/issues/3930)
