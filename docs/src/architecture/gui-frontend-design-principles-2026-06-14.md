---
title: GUI & Front-End Design Principles for Tauri + TypeScript (vox-gui)
description: A directly-applicable catalog of 300+ GUI/front-end design principles spanning UX heuristics, visual design, interaction, accessibility, performance, design systems, desktop-native UX, and Tauri-specific architecture, anchored to primary sources where verified.
category: "Architecture SSOTs"
---

# GUI & Front-End Design Principles for Tauri + TypeScript (vox-gui)

> **Provenance.** This catalog has two tiers. **[V]** marks a principle whose core claim was verified 3-0 against an authoritative primary source by a fan-out research pass (NN/g, W3C/WAI, Google web.dev, official Tauri v2 docs) — see [Sources](#sources). Unmarked principles are established industry best practice (HIG/Material, Refactoring UI, design-systems literature, Tauri community) included to give complete coverage; treat them as strong defaults, not normative law. None of these were checked against the actual `vox-gui` source — applicability is framework-level.
>
> **Scope note (RAIL).** Google's RAIL per-interaction budgets (100/50/10/50 ms) remain valid *design targets* but at the goal-setting level are now framed through Core Web Vitals / INP (FID→INP, March 2024; "good" INP = 200 ms at p75). Frame/idle budgets assume 60 Hz; on 120/144 Hz the per-frame budget shrinks.

---

## 1. Foundational UX Heuristics (Nielsen's 10, expanded)

The ten heuristics are the stable, authoritative framework for evaluating any GUI, unchanged since 1994 with 2020 language refinements. **[V]**

### 1.1 Visibility of system status
1. Always keep the user informed about what is happening through appropriate feedback within a reasonable time. **[V]**
2. Every asynchronous / IPC operation must surface a visible state: pending, in-progress, success, or failure. Silent waits are a defect.
3. Show progress for any operation likely to exceed ~1 second; show determinate progress when total work is known, indeterminate only when it is not.
4. Reflect the *current* mode/state of the app (which view, which document, connected/disconnected) so users never guess.
5. Acknowledge user actions immediately (≤100 ms perceived) even if the result takes longer — a button press should visibly "take".
6. Communicate the result of an action, not just its initiation (e.g., "Saved", not just a spinner that vanishes).
7. Make system state machine-honest: never show a success state for an operation that actually failed or is still pending.
8. Persist status across navigation where relevant (a background job should remain observable when the user moves to another screen).

### 1.2 Match between system and the real world
9. Speak the user's language — words, phrases, concepts familiar to them, not internal jargon or crate/module names.
10. Follow real-world conventions; present information in a natural and logical order.
11. Map controls to outcomes the way the domain expects (a "stop" affordance should look stoppable).
12. Use domain vocabulary consistently; one concept = one word across the entire UI.
13. Prefer recognizable metaphors over novel ones unless the novel one is demonstrably clearer.

### 1.3 User control and freedom
14. Provide a clearly marked "emergency exit" to leave unwanted states without an extended dialogue.
15. Support Undo and Redo for destructive or consequential actions.
16. Let users cancel long-running operations; a cancel that does nothing is worse than no cancel.
17. Never trap the user in a modal with no escape (always honor Esc / a visible close).
18. Make navigation reversible — back should return to the prior state, not a fresh one.
19. Allow users to abandon multi-step flows without penalty and resume later where feasible.

### 1.4 Consistency and standards
20. Follow platform and product conventions — users should not wonder whether different words/situations/actions mean the same thing (Jakob's Law: users spend most time on *other* apps).
21. Internal consistency: the same action looks and behaves the same everywhere in vox-gui.
22. External consistency: match OS conventions for shortcuts, menus, window controls (see §7).
23. Reuse components; do not reinvent a control that already exists in the design system.
24. Keep terminology, iconography, color meaning, and spacing rhythm consistent across all surfaces.

### 1.5 Error prevention
25. The best designs prevent problems before they occur, rather than relying on good error messages. **[V]**
26. Either eliminate error-prone conditions, or check for them and present a confirmation before the user commits to a consequential action. **[V]**
27. Prefer constraints (disable invalid options, mask inputs, use pickers) over post-hoc validation.
28. Use confirmation only for genuinely destructive/irreversible actions; over-confirming trains users to click through.
29. Provide sensible defaults so the most common path requires the fewest decisions.
30. Distinguish *slips* (accidental) from *mistakes* (wrong intent) and design guards for each — undo for slips, clarity for mistakes.
31. Validate inline and early, near the field, before submission where possible.

### 1.6 Recognition rather than recall
32. Minimize memory load by making objects, actions, and options visible.
33. The user should not have to remember information from one part of the interface to another.
34. Make instructions and field requirements visible or easily retrievable at the point of need.
35. Show recently used / suggested items rather than requiring the user to recall identifiers.
36. Keep selected state, filters, and context visible rather than hidden behind menus.

### 1.7 Flexibility and efficiency of use
37. Provide accelerators (keyboard shortcuts, command palette) for experts, invisible to novices.
38. Let frequent users tailor frequent actions (customizable toolbars, saved views, defaults).
39. Support both pointer and keyboard as first-class paths to every action.
40. Offer batch operations for repetitive tasks.
41. Remember user preferences and the last-used state across sessions.

### 1.8 Aesthetic and minimalist design
42. Interfaces should not contain information that is irrelevant or rarely needed — every extra unit competes with the relevant units.
43. Keep content and controls focused on supporting the user's primary goals.
44. Progressive disclosure: show the essential first, reveal advanced options on demand.
45. Reduce visual noise — fewer borders, fewer competing colors, more whitespace.
46. Remove decorative elements that do not aid comprehension or delight with purpose.

### 1.9 Help users recognize, diagnose, and recover from errors
47. Error messages should be in plain language (no codes), precisely indicate the problem, and constructively suggest a solution.
48. Place errors next to their cause; never make the user hunt for what went wrong.
49. Preserve user input when an error occurs — never clear a form on a validation failure.
50. Use color *plus* icon *plus* text for errors (never color alone — see §4).
51. Offer a concrete next action ("Retry", "Reconnect", "Edit field") rather than a dead-end message.
52. Log the technical detail for developers; show the human-meaningful summary to users.

### 1.10 Help and documentation
53. It is best if the system needs no documentation, but provide help that is easy to search, focused on the user's task, and lists concrete steps.
54. Make help contextual — accessible from where the question arises, not only in a separate manual.
55. Keep help concise and scannable; long prose is rarely read.

### 1.11 Supporting heuristics & laws
56. **Hick's Law** — decision time grows with the number/complexity of choices; reduce or group options.
57. **Fitts's Law** — time to hit a target depends on its size and distance; make important targets big and close (edges/corners are infinitely deep).
58. **Miller's Law** — chunk information; ~7±2 is folklore but chunking itself is real and useful.
59. **Doherty Threshold** — keep system response under ~400 ms to keep users engaged and productive.
60. **Peak-End Rule** — users judge an experience by its most intense point and its end; polish completion and error moments.
61. **Postel's Law / Robustness** — be liberal in what input you accept, conservative in what you produce (accept messy input, normalize it).
62. **Tesler's Law** — complexity is conserved; decide whether the system or the user absorbs irreducible complexity (prefer the system).
63. **Goal-Gradient Effect** — users accelerate as they near a goal; show progress to motivate completion.
64. **Zeigarnik Effect** — incomplete tasks are remembered; surface unfinished work to aid resumption.

---

## 2. Visual Design & Layout

A layout missing a clear visual hierarchy makes it hard to know where to look — the interface must guide the eye through elements in order of importance. **[V]**

### 2.1 Visual hierarchy
65. Establish a deliberate hierarchy: every screen should answer "where do I look first?" within a glance. **[V]**
66. Make the most important elements bigger than less important ones (scale signals importance). **[V]**
67. Use weight, color, and contrast — not just size — to rank elements; reserve the strongest contrast for the primary action.
68. There should be exactly one primary action per view; everything else is secondary or tertiary.
69. De-emphasize secondary content with muted color and lighter weight rather than shrinking it into illegibility.
70. Don't rely on font size alone for hierarchy — pair a smaller bold dark label with larger light gray supporting text.
71. Use a limited number of hierarchy levels (typically 3–4: primary, secondary, tertiary, supplementary).
72. Group related controls so the eye reads them as one unit (proximity creates hierarchy).
73. Establish a clear reading path (Z-pattern for sparse layouts, F-pattern for text-dense ones).

### 2.2 Typography
74. Choose a small, consistent type scale (e.g., a modular scale) rather than arbitrary sizes.
75. Limit the number of font sizes and weights in active use; a handful covers nearly all needs.
76. Set body text to a comfortable size (≈14–16 px desktop) and never below the minimum legible for the platform.
77. Line length (measure) ≈ 45–75 characters for body text; too wide tires the eye, too narrow fragments it.
78. Line height ≈ 1.4–1.6 for body; tighter for large headings, looser for dense small text.
79. Use weight, not italics or all-caps, for primary emphasis; reserve all-caps for short labels with added letter-spacing.
80. Align text to a baseline rhythm; avoid mixing many leading values arbitrarily.
81. Prefer left-aligned body text in LTR locales; avoid justified text (rivers of whitespace).
82. Don't use pure-black text on pure-white — slightly soften (e.g., near-black on off-white) while still meeting contrast.
83. Use tabular/lining figures for numeric tables so digits align.
84. Pick a font stack that includes robust system fallbacks for each OS so the app reads natively even before web fonts load.
85. Avoid more than two type families; pair a neutral UI sans with at most one accent/mono family.
86. Use a monospace font for code, IDs, hashes, and aligned technical data.
87. Truncate gracefully (ellipsis + tooltip/title) rather than letting long strings break layout.

### 2.3 Spacing, rhythm, and grid
88. Use a single spacing scale (e.g., 4 px base: 4/8/12/16/24/32/48…) and snap all gaps to it.
89. Whitespace is a feature — generous spacing increases comprehension and perceived quality.
90. Space related items close together and unrelated items farther apart (proximity = relationship).
91. Give related elements more space *outside* the group than *inside* it.
92. Establish a consistent grid (columns + gutters) and align elements to it.
93. Use consistent padding inside containers; mismatched internal padding reads as sloppiness.
94. Maintain a vertical rhythm so successive blocks feel evenly paced.
95. Prefer defining spacing on containers/parents over ad-hoc margins on children to avoid margin collapse surprises.
96. Don't center long-form text blocks; center only short, symmetric content.
97. Align edges — establish strong alignment lines that the eye can follow down/across the layout.
98. Use density deliberately: comfortable density for primary flows, compact density for power-user data views (offer a toggle).

### 2.4 Color
99. Define color by role (primary, surface, background, border, success/warn/error/info, text-primary/secondary/disabled), not by raw hex used ad hoc.
100. Build a tonal palette (e.g., 50→900 steps per hue) so you always have a correct shade for backgrounds, borders, and text.
101. Reserve saturated/accent color for interactive and high-priority elements; keep large surfaces neutral.
102. Don't use too many accent hues; one or two plus a neutral ramp is usually enough.
103. Convey state with semantic colors consistently (the same red always means error/destructive).
104. Never encode meaning by color alone — pair with icon, text, or pattern (colorblind users, see §4).
105. Define elevation/depth with subtle shadows and surface tints, not heavy borders.
106. Test the entire palette for contrast against its intended text/background pairings (see §4 thresholds).
107. Design light *and* dark themes from semantic tokens, not by inverting hex values.
108. In dark themes, avoid pure black surfaces and pure white text; use dark gray surfaces and slightly dimmed text to reduce halation.
109. Desaturate colors slightly in dark mode; fully saturated hues vibrate on dark backgrounds.
110. Reducing text contrast reduces legibility and may make content inaccessible — keep it high. **[V]**

### 2.5 Iconography & imagery
111. Use a single icon set with consistent stroke weight, corner radius, and optical size.
112. Pair icons with text labels for primary navigation/actions; icon-only is for compact, well-learned controls.
113. Give icon-only controls accessible names and tooltips.
114. Align icon optical centers, not bounding boxes, with adjacent text.
115. Use icons that match the platform's visual language where it improves nativeness.
116. Don't overload one icon with multiple meanings across contexts.

### 2.6 Gestalt & composition
117. **Proximity** — items placed close together are perceived as related.
118. **Similarity** — items sharing color/shape/size are perceived as a group; use it to imply structure.
119. **Common region** — a shared background/border groups items (cards, panels).
120. **Continuity** — the eye follows lines and alignment; use it to lead through content.
121. **Closure** — the mind completes shapes; you can imply containers without full borders.
122. **Figure/ground** — ensure the foreground clearly separates from the background (contrast, elevation).
123. **Symmetry & balance** — distribute visual weight; an unbalanced layout feels unstable.
124. Use borders sparingly — prefer spacing, background color, and shadow to separate regions.

---

## 3. Interaction Patterns

### 3.1 Affordances & signifiers
125. Make interactive elements *look* interactive (affordance) via signifiers — shape, color, cursor, hover state.
126. Make non-interactive elements *not* look clickable; false affordances erode trust.
127. Use the correct cursor for the context (pointer on links/buttons, text on editable, grab on draggable, not-allowed on disabled).
128. Hover, focus, active, and disabled states must be visually distinct for every interactive control.
129. Disabled controls should look disabled *and* (ideally) explain why they're disabled on hover/focus.
130. Buttons look like buttons, links look like links; don't style a navigation link as a destructive button.
131. Primary buttons are visually dominant; destructive buttons are clearly marked (and often require confirmation).

### 3.2 Feedback & responsiveness
132. Provide immediate visual feedback on press/click (state change within ~100 ms perceived).
133. For 0.1–1 s waits no special indicator is needed; for 1–10 s show a determinate/indeterminate indicator; beyond ~10 s show progress + allow other work.
134. Optimistic UI: reflect the expected result immediately, then reconcile when the backend responds — and roll back visibly on failure.
135. Debounce/throttle high-frequency inputs (search-as-you-type, resize) to keep feedback smooth.
136. Use skeleton screens for content-shaped loads to reduce perceived wait and layout shift; spinners for short, indeterminate waits.
137. Avoid layout shift when content arrives — reserve space for async content.
138. Confirm completion of background tasks via unobtrusive notifications (toast), not blocking modals.
139. Animate state transitions to preserve context (where did this come from, where did it go), but keep them fast (see §5).

### 3.3 Forms & input
140. Ask only for what you need; every field has a cost.
141. One column layouts complete faster than multi-column for most forms.
142. Place labels above fields (fastest scanning) and keep them always visible (avoid placeholder-as-label).
143. Don't use placeholder text as the only label — it disappears on input and fails accessibility.
144. Group related fields with headings/sections; show progress for long/multi-step forms.
145. Mark required vs optional explicitly and consistently (prefer marking the rarer of the two).
146. Use the right input control for the data (date picker, stepper, select, toggle) to prevent invalid entry.
147. Validate inline as the user leaves a field; show success affirmatively where helpful.
148. Keep error messages specific and adjacent; never blank the form on error (see §1.9).
149. Set input constraints (type, min/max, pattern, autocomplete attributes) so the platform helps the user.
150. Disable the submit button only with a clear reason, or allow submit and explain what's missing — avoid silently-dead buttons.
151. Preserve and restore in-progress form state across accidental navigation where data loss would hurt.
152. Make the primary submit action keyboard-reachable and Enter-submittable in single-field/simple forms.
153. Provide autofocus on the first meaningful field when a form is the screen's purpose.

### 3.4 Navigation & information architecture
154. Make the current location obvious (active nav state, breadcrumb, title).
155. Keep primary navigation stable and consistent across screens.
156. Match navigation depth to content; avoid deep nesting that hides functionality.
157. Provide a global search / command palette for direct access in feature-rich apps.
158. Use breadcrumbs for hierarchical structures; use tabs for sibling views of one object.
159. Don't mix navigation paradigms arbitrarily; pick patterns and apply them consistently.
160. Ensure back/forward and history behave predictably (especially with a webview router).
161. Surface the most frequent destinations first; bury the rare ones.
162. Keep destructive or irreversible navigation (close, discard) clearly separated from routine navigation.

### 3.5 Empty, loading, and error states
163. Design the empty state deliberately — it's the user's first impression and a chance to guide the next action.
164. Empty states should explain what goes here and offer a primary action to fill it.
165. Distinguish "empty because new" from "empty because filtered" from "empty because error".
166. Loading states should match the shape of the eventual content (skeletons) to minimize jolt.
167. Error states should be actionable (retry/reconnect/contact) and preserve as much context as possible.
168. Partial-failure states (some data loaded, some failed) should show what succeeded and isolate the failure.

### 3.6 Microinteractions
169. Every microinteraction has a trigger, rules, feedback, and loops/modes — design all four.
170. Keep microinteractions fast, subtle, and purposeful; they should clarify, not distract.
171. Respect `prefers-reduced-motion` — disable or reduce nonessential animation when requested.
172. Use motion to show causality and continuity (an item flying to a destination explains where it went).

---

## 4. Accessibility (WCAG 2.2)

Accessibility is structured by WCAG's four POUR principles (Perceivable, Operable, Understandable, Robust) and three conformance levels A/AA/AAA; **AA is the common target** (and the bar of the 2024 US DOJ rule). **[V]** ("POUR" is an industry mnemonic, not W3C's literal term.)

### 4.1 Conformance model
173. Target WCAG 2.2 Level AA as the baseline for vox-gui. **[V]**
174. Conformance is determined *solely* by meeting success criteria — W3C's published *techniques* are informative and not required. **[V]**
175. Failing a specific technique test does not necessarily mean failing WCAG; evaluate actual success-criteria conformance. **[V]**
176. Automated technique-level checks (axe, Lighthouse) are necessary but **not sufficient** — manual testing is required. **[V]**
177. Each success criterion has a level (A/AA/AAA); meeting AA means meeting all A and AA criteria. **[V]**

### 4.2 Contrast (Perceivable)
178. Text and images of text need a contrast ratio of at least **4.5:1** against background (SC 1.4.3, Level AA). **[V]**
179. Large-scale text (≥18 pt, or ≥14 pt bold) needs at least **3:1** (SC 1.4.3). **[V]**
180. UI components and graphical objects (button/input borders, icons, focus indicators, slider thumbs, chart elements) need at least **3:1** against adjacent colors (SC 1.4.11, Level AA). **[V]**
181. Inactive/disabled components are exempt from the contrast minimum — but don't abuse the exemption for content users must read. **[V]**
182. Low contrast measurably harms low-vision and colorblind users — test every theme variant (light/dark/high-contrast). **[V]**
183. Verify focus indicators specifically meet the 3:1 non-text contrast and remain visible on every background.

### 4.3 Color independence (Perceivable)
184. Never use color as the only means of conveying information, indicating an action, or distinguishing a visual element (SC 1.4.1) — add text, icon, or pattern.
185. Ensure charts/graphs are distinguishable in grayscale and for common color-vision deficiencies.
186. Don't rely on red/green alone for valid/invalid (most common color-blindness type).

### 4.4 Keyboard & focus (Operable)
187. All functionality must be operable via keyboard (SC 2.1.1, Level A). *(Note: the strong "no feature may be mouse-only" phrasing was the one research claim that did NOT survive verification — implement full keyboard operability as best practice, but cite the SC text itself rather than the overstatement.)*
188. Maintain a logical, predictable tab order that follows visual/reading order.
189. Provide a visible focus indicator on every focusable element (do not remove outlines without a compliant replacement).
190. No keyboard traps — focus must be able to move away from any component via keyboard (SC 2.1.2).
191. Implement standard keyboard interaction patterns for composite widgets (arrow keys in menus/lists/tabs, Esc to close, Enter/Space to activate).
192. Provide "skip to content" affordances for repetitive navigation where applicable.
193. Manage focus on route/view changes and when opening/closing dialogs (move focus in, restore it out).
194. Make custom controls focusable with `tabindex` only when they genuinely need it; don't add positive tabindex values.

### 4.5 ARIA discipline (Robust)
195. Prefer native HTML controls — they provide keyboard behavior and semantics for free. **[V]**
196. Applying an ARIA role gives an element **no** keyboard behavior and **no** styling; you must script the expected keyboard interactions yourself. **[V]**
197. Incorrectly applied ARIA can override an element's native accessibility semantics and harm assistive-tech users. **[V]**
198. "No ARIA is better than bad ARIA" — omit ARIA rather than use it wrong. **[V]** (Documented on MDN / the WAI-ARIA spec.)
199. Follow the WAI-ARIA Authoring Practices patterns when you must build a custom widget; don't improvise roles.
200. Use `aria-live` regions to announce dynamic changes (status, errors, async results) to screen readers.
201. Keep the accessibility tree in sync with the visual state (`aria-expanded`, `aria-selected`, `aria-checked`, `aria-disabled`).
202. Give every input an associated `<label>`; give every actionable icon an accessible name.
203. Use landmark regions/roles (`header`, `nav`, `main`, `aside`, `footer`) so screen-reader users can navigate structure.

### 4.6 Understandable & robust
204. Set the document `lang`; mark language changes inline where they occur.
205. Use a logical heading hierarchy (one h1; no skipped levels) to convey structure.
206. Make instructions and errors programmatically associated with their fields (`aria-describedby`).
207. Respect OS-level preferences: reduced motion, high contrast, increased font size, and color scheme.
208. Ensure the UI is usable at 200% zoom and at increased OS font scaling without loss of content/function (SC 1.4.4 / 1.4.10 reflow).
209. Don't disable user zoom or fix font sizes in viewport units that ignore user preferences.
210. Test with an actual screen reader (NVDA on Windows, VoiceOver on macOS, Orca on Linux) — not just automated tools.
211. Provide sufficiently large click/tap targets (the WCAG 2.2 target-size minimum is 24×24 CSS px for SC 2.5.8 AA; larger is better).

---

## 5. Performance & Perceived Performance

RAIL budgets give the webview's single main thread precise targets. **[V]**

### 5.1 RAIL budgets (design targets)
212. **Response:** complete a transition initiated by user input within **100 ms** to feel instantaneous. **[V]**
213. The actual processing budget for an input handler is only **~50 ms** (input may be queued behind other work). **[V]**
214. **Animation:** produce each frame in **≤10 ms** (the 60 FPS ~16 ms budget minus ~6 ms the browser needs to render). **[V]**
215. **Idle:** break background/idle work into **≤50 ms** chunks. **[V]**
216. User interaction must always take highest priority and interrupt idle-time work. **[V]**
217. On 120/144 Hz displays the per-frame budget is *smaller* than 16/10 ms — don't assume 60 Hz.
218. Track the modern field metric INP (target ≤200 ms at p75) as the goal-level successor to the per-interaction RAIL response budget.

### 5.2 Keeping the main thread free
219. The webview has a single main thread shared by JS, layout, paint, and event handling — never block it.
220. Move heavy computation to Web Workers, or better, to the Rust core via Tauri commands (off the JS thread entirely).
221. Chunk long tasks and yield to the event loop (e.g., `scheduler.yield()` / `setTimeout` / `requestIdleCallback`) so input stays responsive.
222. Defer non-critical work to idle time; prioritize what the user is waiting on.
223. Avoid synchronous layout thrashing — batch DOM reads then writes; don't interleave them in loops.
224. Use `requestAnimationFrame` for visual updates; never animate in a tight `setInterval`.
225. Animate only compositor-friendly properties (`transform`, `opacity`); avoid animating layout-triggering properties (width/height/top/left).
226. Use `will-change` sparingly and remove it after the animation; it consumes memory.
227. Virtualize long lists/tables (render only visible rows) — vox-gui's data views must not render thousands of DOM nodes.

### 5.3 Perceived performance
228. Acknowledge input instantly even when the result is slow (perceived speed > actual speed).
229. Use skeleton screens for content-shaped loads; they reduce perceived wait vs. spinners and prevent layout shift.
230. Use spinners only for short, genuinely indeterminate waits; a spinner shown too long feels slower than a skeleton.
231. Show progress for long operations; a determinate bar feels faster than an indeterminate one.
232. Preload/prefetch likely-next data during idle time so navigation feels instant.
233. Render above-the-fold/critical UI first; lazy-load the rest.
234. Avoid content layout shift (reserve space, set dimensions) — jank reads as slowness and breaks trust.
235. Cache results of expensive queries and reuse them; invalidate deliberately.

### 5.4 Bundle, startup & assets
236. Code-split by route/feature; load only what the current view needs.
237. Lazy-load heavy components, charts, editors, and rarely-used panels.
238. Tree-shake and drop dead code; audit bundle composition regularly.
239. Prefer a few small, focused dependencies over large monolithic libraries; question every dependency's weight.
240. Self-host fonts and assets (Tauri ships them locally) — no runtime CDN dependency for a desktop app.
241. Subset fonts to the glyphs you use; avoid shipping entire families.
242. Compress and right-size images; use modern formats (WebP/AVIF) where the webview supports them.
243. Inline only truly critical CSS; defer the rest.
244. Minify and compress all production assets.
245. Measure cold-start to first meaningful paint and treat startup time as a tracked budget.
246. Keep the initial JS execution small — heavy work at startup delays the first interaction.

---

## 6. Component Architecture & Design Systems

> The research pass found **no surviving primary-sourced claims** for tokens/state/component architecture (see [Open questions](#open-questions)); the principles below are established design-systems and React/TS best practice, not verified claims.

### 6.1 Design tokens
247. Define design decisions as tokens (color, spacing, type, radius, shadow, motion, z-index) — a single source of truth.
248. Design tokens are *not* just CSS variables — they're a platform-agnostic, named decision layer that *compiles to* CSS variables, TS constants, Rust, etc.
249. Layer tokens: **global/primitive** (raw scale values) → **semantic/alias** (role-based, e.g., `color.text.primary`) → **component** (e.g., `button.bg`).
250. Reference semantic tokens in components, never raw primitives, so theming and re-skinning work without touching components.
251. Keep one canonical token source and generate per-platform outputs from it (avoid hand-maintained duplicates).
252. Name tokens by role/intent, not by appearance (`color.danger`, not `color.red`) so values can change without renaming.
253. Express theming (light/dark/high-contrast) by swapping semantic token values, not by conditional logic in components.
254. Include motion and elevation tokens, not just color/space, so animation and depth stay consistent.
255. Version and document tokens; treat token changes as API changes with migration notes.

### 6.2 Component design
256. Build a small set of well-tested primitives (Button, Input, Select, Dialog, Menu, Tooltip, Table) and compose features from them.
257. Prefer composition over configuration — many small composable parts beat one component with dozens of boolean props.
258. Use compound components (e.g., `<Menu>`, `<Menu.Item>`) to model parent/child relationships with shared implicit state.
259. Keep components single-responsibility; a component that does everything is hard to reuse and test.
260. Separate presentational components (how it looks) from container/logic components (what it does) where it clarifies.
261. Make components controllable *and* uncontrolled-capable where it makes sense (accept value+onChange, but provide defaults).
262. Expose a minimal, well-typed prop API; avoid leaking internal implementation details.
263. Provide sensible defaults so the common case needs few props.
264. Forward refs and spread remaining props to the underlying DOM node for composability.
265. Co-locate component styles, tests, stories, and types with the component.
266. Document each component's intended use, variants, and accessibility behavior (a living catalog / Storybook).
267. Build accessibility into primitives once (focus management, ARIA, keyboard) so every consumer inherits it.
268. Make variants explicit and bounded (a `variant` union type), not open-ended style overrides.

### 6.3 TypeScript discipline
269. Type the IPC/command boundary end-to-end — share types between the Rust core and the TS frontend (generate from a single source where possible, e.g., ts-rs/specta).
270. Prefer discriminated unions for state and message shapes; let the compiler enforce exhaustive handling.
271. Avoid `any`; prefer `unknown` at boundaries and narrow explicitly.
272. Make illegal states unrepresentable — model loading/success/error as a union, not three independent booleans.
273. Use `readonly`/immutability for state to avoid accidental mutation bugs.
274. Validate untrusted/external data at runtime (e.g., zod/valibot) and derive types from the schema.
275. Keep prop and event types exported so consumers get full inference.
276. Use strict compiler settings (`strict`, `noUncheckedIndexedAccess`) to catch errors early.

### 6.4 State management
277. Keep state as local as possible; lift it only when genuinely shared (avoid premature global stores).
278. Separate server/async state (data from the Rust core) from client UI state (open panels, selections) — they have different lifecycles.
279. Use a query/cache layer for async data (caching, deduping, background refresh, stale-while-revalidate) rather than hand-rolled effects.
280. Make async state explicit (idle/loading/success/error) and render each state deliberately.
281. Derive state rather than duplicating it; a single source of truth prevents desync bugs.
282. Avoid storing derived/computable values in state; compute them from the source.
283. Co-locate state with the components that use it; global state is for genuinely cross-cutting concerns.
284. Keep reducers/updates pure and predictable; isolate side effects.
285. Persist only the state worth persisting (preferences, last view) and version the persisted shape.

---

## 7. Desktop-App-Specific & Cross-Platform UX

> No primary-sourced claims survived for native/platform UX (see [Open questions](#open-questions)); these are established HIG/platform conventions and Tauri community practice.

### 7.1 Native feel
286. A Tauri app *can* feel native — invest in matching platform conventions, not just shipping a web page in a window.
287. Respect the OS theme (light/dark) and accent color where the platform exposes it; follow system changes live.
288. Match platform typography expectations (system UI font stack per OS) so text reads native.
289. Honor OS-level settings: reduced motion, high contrast, font scaling, and reduced transparency.
290. Use native dialogs (file open/save, message boxes) via Tauri APIs rather than re-implementing them in HTML.
291. Use native notifications via the platform, not in-webview toasts, for OS-level alerts.
292. Provide a real application menu / system tray where the platform expects one.
293. Avoid web idioms that feel wrong on desktop (e.g., a "hamburger menu" where a menu bar belongs; browser-style page reload).

### 7.2 Window management
294. Restore window size, position, and maximized/fullscreen state across launches.
295. Set sensible minimum window dimensions so the layout never collapses.
296. Handle multi-monitor and DPI changes gracefully; test on mixed-DPI setups.
297. Support standard window controls and their platform-correct placement (see §7.4).
298. If using a custom/decorationless title bar, re-implement drag regions, double-click-to-maximize, and the control buttons per platform.
299. Persist and restore per-window state for multi-window apps; don't lose the user's workspace.
300. Make the app resilient to being hidden/minimized/restored and to display configuration changes.

### 7.3 Menus & keyboard shortcuts
301. Provide a conventional menu structure (File, Edit, View, Window, Help) on platforms that expect a menu bar.
302. Follow platform shortcut conventions: **Cmd** on macOS vs **Ctrl** on Windows/Linux for the same actions (copy/paste/save/undo).
303. Standard shortcuts must do standard things: Ctrl/Cmd+C/V/X/Z/Y/S/A/F/N/W/Q as users expect.
304. Show shortcut hints next to menu items so users learn accelerators.
305. Don't override OS-reserved shortcuts; don't shadow platform conventions with custom behavior.
306. Provide a discoverable command palette as a shortcut-agnostic path to every action.
307. Make destructive shortcuts safe (confirm or undo) — an accidental Cmd/Ctrl shortcut shouldn't lose data.
308. Localize/adapt the modifier symbols shown in UI per platform (⌘⌥⇧ on macOS, "Ctrl/Alt/Shift" on Windows/Linux).

### 7.4 Platform conventions (Windows / macOS / Linux)
309. Window control buttons: **right** on Windows/most Linux, **left** on macOS — respect placement if you customize the title bar.
310. Primary action button order in dialogs differs by platform (e.g., affirmative-on-right on Windows vs macOS conventions) — follow each platform's HIG.
311. Follow each platform's Human Interface Guidelines: Apple HIG (macOS), Microsoft Fluent/Windows app guidelines, GNOME/KDE HIG (Linux).
312. Match platform conventions for menu placement (global menu bar on macOS vs in-window on Windows/Linux).
313. Use platform-correct terminology ("Preferences" on macOS vs "Settings"/"Options" on Windows; "Quit" vs "Exit").
314. Respect platform file-system conventions and standard directories (config/data/cache locations) via Tauri path APIs.
315. Test on all three target platforms — webview engines differ (WebKitGTK on Linux, WebView2 on Windows, WKWebView on macOS) and so do behaviors.
316. Account for WebView engine differences: a feature/CSS that works in WebView2 may render or behave differently in WebKitGTK; test, don't assume Chromium.

---

## 8. Tauri-Specific Architecture, Security & Performance

All claims in this section verified 3-0 against official Tauri v2 docs. **[V]**

### 8.1 The IPC trust boundary
317. Code in the WebView/frontend can access system resources **only** through the well-defined IPC layer — never directly. **[V]**
318. The security model differentiates Rust *core* code from *frontend* code; the IPC layer is the bridge and the chokepoint that keeps the boundary intact. **[V]**
319. Design the command API as the trust boundary: the Rust core never hands raw OS access to the frontend — treat every IPC input as untrusted. **[V]**
320. Validate and sanitize all arguments arriving from the frontend inside the Rust command; the frontend is attacker-controllable if XSS occurs.
321. Keep commands coarse and intention-revealing (`save_project`) rather than exposing low-level primitives (`write_file(path)`) that widen the attack surface.
322. Return typed, structured results and typed errors from commands; don't leak raw internal errors to the UI.

### 8.2 Capabilities & permissions (default-deny)
323. Tauri commands are **default-deny** — frontend access to core commands must be explicitly granted via capabilities; without a grant a command silently does nothing / returns "not allowed". **[V]**
324. Capabilities are JSON or TOML files in **`src-tauri/capabilities/`** that define which permissions are granted or denied for which windows/webviews. **[V]**
325. A single capability can affect multiple windows and webviews simultaneously — scope grants narrowly to the windows that need them. **[V]**
326. Individual command implementations enforce optional **fine-grained access levels** (scopes/permissions) defined in the capabilities config — the runtime passes the scope to the command, which must enforce it. **[V]**
327. Command developers must ensure there are **no scope bypasses** — the runtime hands you the scope, but enforcement is your responsibility. **[V]**
328. Grant the minimum permission set required (least privilege); audit the capabilities files as security-critical config.
329. Prefer per-window capabilities for least privilege when different windows have different trust needs.

### 8.3 Content Security Policy (CSP)
330. Tauri's CSP-based XSS protection is **opt-in** — it is only active if a CSP value is set in the Tauri config; omitting it leaves the webview without this XSS mitigation. **[V]**
331. The Isolation pattern and IPC layer are **not substitutes** for CSP — set a CSP. **[V]**
332. When CSP is configured, Tauri auto-injects nonces and hashes at **compile time** for bundled code/assets, so you only configure what's unique to your app (non-bundled sources). **[V]**
333. Local scripts are hashed; styles and external scripts are referenced via a cryptographic nonce, blocking content not explicitly allowed. **[V]**
334. Set a strict CSP and add only the application-specific sources you actually need; avoid `unsafe-inline`/`unsafe-eval`.
335. Treat any need to relax CSP as a security review trigger.

### 8.4 Isolation, updates & supply chain
336. Consider the Isolation pattern to intercept and validate IPC messages between frontend and core as a defense-in-depth layer.
337. Keep Tauri and its dependencies updated — published IPC-bypass advisories were implementation bugs fixed by hardening the same IPC chokepoint; staying current is part of the security posture.
338. Sign and verify application updates; use Tauri's updater with signature verification.
339. Audit both the npm/TS and Cargo/Rust dependency trees; a desktop app's supply chain spans both ecosystems.
340. Don't ship secrets in the frontend bundle — the webview is inspectable; keep secrets in the Rust core.

### 8.5 Tauri performance & bundle
341. Push heavy/CPU-bound work into the Rust core via commands; it runs off the webview's single JS thread and is faster.
342. Use async commands for I/O-bound work so the UI thread and the Rust runtime aren't blocked.
343. Stream or paginate large datasets across IPC rather than serializing huge payloads in one message.
344. Minimize IPC chatter — batch related calls; each IPC round-trip has serialization overhead.
345. Choose an efficient serialization strategy for large/binary payloads (e.g., raw bytes over JSON) where appropriate.
346. Leverage Tauri's small binary footprint — it uses the OS webview rather than bundling a browser, so keep the frontend bundle lean to preserve the startup/size advantage.
347. Measure and budget cold-start (process spawn → webview ready → first meaningful paint); regressions here hurt the desktop-native feel most.
348. Lazy-initialize expensive Rust subsystems (DB, models) so the window appears fast and heavy init happens after first paint.
349. Free webview/Rust resources on window close; long-lived multi-window apps must avoid leaks.
350. Profile on the *slowest* target platform/engine (often WebKitGTK on Linux), not just your dev machine.

---

## 9. Applying This to vox-gui (synthesis)

351. **Tauri-first data flow:** push compute to the Rust core, keep the webview thin — this satisfies both the IPC security boundary **[V]** and the single-thread performance budget **[V]** at once.
352. **Type the seam:** generate shared TS types from Rust for every command so the trust boundary is also a compile-time contract.
353. **Default-deny by habit:** treat each new command as needing an explicit, minimal capability grant **[V]**; never broaden capabilities to "make it work".
354. **Theme from tokens:** build vox-gui's light/dark themes from semantic tokens that are validated against the WCAG 4.5:1 / 3:1 thresholds **[V]** before they ship.
355. **Status for every async op:** every command invocation gets a visible pending/success/error state (Visibility of System Status **[V]**), with input acknowledged in ≤100 ms **[V]**.
356. **Native where it counts:** native dialogs, menus, window-state persistence, and platform-correct shortcuts so vox-gui feels like an app, not a page.
357. **Accessibility as a primitive:** bake keyboard + ARIA + focus management into the base component set once **[V]**, so every screen inherits AA conformance.
358. **Set the CSP now:** because it's opt-in **[V]**, an unconfigured vox-gui has *no* CSP XSS mitigation — make it a launch-blocking checklist item.
359. **Budget the cold start:** track process-spawn-to-first-paint as a first-class metric; it's where the Tauri size/speed advantage is won or lost.
360. **Measure, don't assume:** verify contrast with a tool, performance with a profiler, accessibility with a real screen reader, and behavior on all three webview engines.

---

## Open questions

The research pass produced **no surviving primary-sourced claims** for several requested sub-topics; §6 and §7 above draw on established secondary best practice rather than verified claims, and these remain worth a dedicated, citable follow-up:

1. **Design-token architecture & state management** specifically for Tauri + TypeScript — authoritative, citable guidance (W3C Design Tokens Community Group format; canonical state-management patterns).
2. **Platform HIG specifics** — the most load-bearing Apple HIG / Windows Fluent / GNOME HIG points for a cross-platform Tauri app (menu/dialog/button-order conventions per OS).
3. **Tauri startup/bundle targets** — concrete, sourced cold-start and binary-size numbers and techniques (lazy-loading, webview constraints) rather than directional advice.
4. **Forms & navigation** — primary-sourced NN/g guidance on form design and navigation patterns to supplement the error-prevention/feedback heuristics.

One claim was **refuted** (1-2) and excluded: the overstated phrasing that WCAG SC 2.1.1 "prohibits any mouse-only feature." Full keyboard operability remains best practice; cite the SC text directly rather than the overstatement.

## Sources

Verified-tier primary sources (each backed a 3-0 claim):

- NN/g — [10 Usability Heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/)
- NN/g — [Principles of Visual Design](https://www.nngroup.com/articles/principles-visual-design/)
- W3C — [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- W3C/WAI — [WCAG overview](https://www.w3.org/WAI/standards-guidelines/wcag/)
- W3C/WAI — [ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/)
- W3C/WAI — [Understanding Techniques for WCAG](https://www.w3.org/WAI/WCAG22/Understanding/understanding-techniques)
- Google web.dev — [Measure performance with the RAIL model](https://web.dev/articles/rail)
- Tauri v2 — [Security](https://v2.tauri.app/security/), [Capabilities](https://v2.tauri.app/security/capabilities/), [Scope](https://v2.tauri.app/security/scope/), [CSP](https://v2.tauri.app/security/csp/)

Secondary/best-practice sources informing the unmarked principles (§2 craft, §3 forms/nav, §6 tokens/components/state, §7 platform UX): Refactoring UI (Wathan/Schoger), Interaction Design Foundation, EightShapes (design tokens), WebAIM, Tauri community guides, and platform HIGs (Apple/Microsoft/GNOME). These are not verified claims; treat as strong defaults.

> Research method: 6-angle fan-out, 28 sources fetched, 139 claims extracted, top 25 adversarially verified (3-vote, 2/3 to kill), 24 confirmed / 1 refuted, synthesized 2026-06-14.
