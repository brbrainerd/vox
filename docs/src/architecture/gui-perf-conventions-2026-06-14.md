---
title: vox-gui Performance Conventions
description: RAIL budgets, animation rules, and heavy-work patterns for the vox-gui frontend.
category: "Architecture SSOTs"
---

# vox-gui Performance Conventions

> Adopted: 2026-06-14. Covers the vox-gui Tauri frontend (`crates/vox-gui/ui/`).

## 1. RAIL Budget

RAIL is Google's model for user-centric performance. The following budgets apply to all vox-gui surfaces.

| Phase | Budget | Notes |
|-------|--------|-------|
| **Response** (user action → visual feedback) | < 100 ms total; < 50 ms JS processing | Button clicks, keyboard shortcuts, UI state toggles |
| **Animation** (each frame) | < 10 ms JS work per frame | Remaining 6 ms budget is for the browser's compositor |
| **Idle** (deferred work) | ≤ 50 ms chunks | Use `requestIdleCallback` or `setTimeout(fn, 0)` for background parsing |
| **Load** (shell to interactive) | < 1 000 ms on target hardware | Tauri webview startup + JS parse + first render |

### Why 50 ms?

50 ms is the "Long Task" threshold (Chrome DevTools / LoAF). Any JS that blocks the main thread for > 50 ms is flagged as a long task and produces jank at 60 fps.

---

## 2. Animation Rule: `transform` and `opacity` Only

**Rule:** Animate only `transform` and `opacity` (and `filter` when unavoidable). Never animate `width`, `height`, `top`, `left`, `margin`, `padding`, or `background-color` via JS.

**Why:** `transform` and `opacity` are composited by the GPU on a separate layer. Animating them skips layout and paint — the two most expensive browser pipeline stages. Animating geometry properties triggers layout on every frame, burning through the 10 ms frame budget instantly.

**How to apply in Tailwind:**

```css
/* Good — compositor only */
.slide-in  { animation: slideIn 200ms ease-out; }
@keyframes slideIn { from { transform: translateY(8px); opacity: 0; } to { transform: none; opacity: 1; } }

/* Bad — triggers layout every frame */
.expand { animation: expand 300ms ease; }
@keyframes expand { from { height: 0; } to { height: 200px; } }
```

**Existing keyframes in `src/index.css`:** `vox-toast-in`, `vox-ping`, `vox-pulse-slow` all use `transform`/`opacity`/`scale` — compliant. Add new keyframes only with `transform`/`opacity`.

**`prefers-reduced-motion`:** Handled in Phase 0C. Do not duplicate here.

---

## 3. Heavy Work → Rust `#[command]`

**Rule:** Any computation that is not trivially O(1) or O(n) with n < 50 must be moved to a Tauri `#[command]` in Rust.

| Category | Threshold | Action |
|----------|-----------|--------|
| Data filtering / sorting | > 500 items | Rust command |
| String search / regex over corpus | any corpus size | Rust command (via `vox_search_query`) |
| Cryptographic operations | any | Rust command |
| File I/O or network | any | Rust command (Tauri already enforces this) |
| JSON parsing of large payloads | > 100 KB | Consider streaming from Rust |
| Embedding / ML inference | any | Rust command (via MENS pipeline) |

```typescript
// BAD — blocks the JS thread for large data sets
const filtered = hugeArray.filter(item => item.score > threshold);

// GOOD — offload to Rust, return the processed result
const filtered = await invoke<Item[]>('filter_items_by_score', { threshold });
```

---

## 4. List Virtualization

**Rule:** Any list that can exceed 50 items must be virtualized using `useVirtualList` (`src/hooks/useVirtualList.ts`).

| Surface | List | Status |
|---------|------|--------|
| `TasksView` | `inProgress`, `queued` | Virtualized (Phase 0D) |
| `MemoryView` | `recent_recalls` | Virtualized (Phase 0D) |
| `MemoryView` | `shards` | Scroll-capped at 700 px (Phase 0D) |
| `RunsView` | runs | Hard-bounded at 40 — no virtualization needed |
| `SearchView` | search hits | Hard-bounded at 30 — no virtualization needed |

---

## 5. Inline Styles: When They Are Correct

The project intentionally uses `style={{}}` for dynamic values that cannot be expressed as static Tailwind classes. Examples:

- `style={{ width: '${score * 100}%' }}` — progress bars
- `style={{ height: totalSize, position: 'relative' }}` — virtualizer inner div
- `style={{ transform: \`translateY(\${vItem.start}px)\` }}` — virtual item position

These are **not** style violations. The CSP already includes `style-src 'unsafe-inline'` to allow them.

---

## 6. References

- [RAIL model](https://web.dev/rail/)
- [Stick to compositor-only properties](https://web.dev/stick-to-compositor-only-properties-and-manage-layer-count/)
- [`@tanstack/react-virtual` docs](https://tanstack.com/virtual/latest)
- Phase 0C (a11y, `prefers-reduced-motion`): `docs/superpowers/plans/2026-06-14-vox-gui-phase0c-a11y-primitives.md`
