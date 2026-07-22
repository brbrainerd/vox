# Task 4 truncate-vs-wrap verdict (2026-07-21)

## Concern under test

Task 4 (commit `490d22a32f`) redesigned `ChatSessionRail.tsx` session rows from
the F-05 2-line-wrap layout (`line-clamp-2 break-words`, see
`.remediation-notes/task-f05-fix-verdict.md`) to a compact single-line
`truncate` layout with a native `title` tooltip. Review flagged that F-05's
own verdict says realistic session titles are "almost always well under 40
characters" and that 2-line wrapping was deliberately chosen to avoid needing
a hover tooltip for those realistic titles — so Task 4's single-line
`truncate` risked re-clipping realistic 30-40 char titles that fit at 2 lines
but not 1, in the 256px-wide (`w-64`) rail.

## Method

Real-browser (Chromium, Playwright) measurement, not jsdom (jsdom performs no
real layout/font metrics, so `scrollWidth`/`clientWidth` there are always 0 —
not trustworthy). Steps:

1. Started the real Vite dev server (`pnpm run dev`, port 1420).
2. Loaded the real app shell via `installOperatorShellMock` (chat view),
   with the mocked `chat_list_sessions` response overridden in-page to
   return exactly the 3 test titles below, so the rail renders the real
   component through its real render path (no synthetic markup).
3. Rendered at 1400x900 viewport (rail width unaffected — `w-64` is fixed).
4. For each session row, read `scrollWidth` vs `clientWidth` of the title
   `<span>` via `getComputedStyle`/DOM APIs in the live page (real font:
   `500 12px/16px Inter, system-ui, "Segoe UI", sans-serif`). `clipped =
   scrollWidth > clientWidth`.
5. Captured a full-page and rail-only screenshot as visual evidence.

Row geometry: `w-64` (256px) rail, `Glass` padding `p-3` (12px each side),
row `pl-2 pr-1.5` (8px/6px) inside a `border-l-2` (2px) button, plus the
`⋯` actions button (~22px) and its `gap-2` — leaves ~178-192px of available
width for the title text depending on whether the message-count badge is
present.

## Results — BEFORE fix (single-line `truncate`, Task 4 as committed)

| Title (chars) | scrollWidth | clientWidth | clipped |
|---|---|---|---|
| "Investigate flaky dockview drag test" (37) | 217px | 192px | **yes** |
| "Vox Terminal ratatui TUI phase 2" (33) | 196px | 178px | **yes** |
| "Build broker L1 fair-FIFO shim" (31) | 178px | 178px | no |

2 of 3 realistic test titles clipped at 1 line — confirming the review's
concern: the original F-05 title-clipping bug was reintroduced by Task 4 for
realistic titles.

## Fix applied

Restored `line-clamp-2 break-words` on the title span (reverting that one
class from Task 4's `truncate`), changed the row from a fixed `h-8`/
`items-center` single line to `min-h-8`/`items-start` so it can grow to fit
2 lines, and nudged the message-count badge down (`pt-px`) to align with the
first line. All of Task 4's other compact styling — the `border-l-2` left
gutter, accent-on-active coloring, tight `py-1 pl-2 pr-1.5` padding, and the
inline single-digit badge — is unchanged.

## Results — AFTER fix

| Title (chars) | scrollWidth | clientWidth | clipped |
|---|---|---|---|
| "Investigate flaky dockview drag test" (37) | 192px | 192px | no |
| "Vox Terminal ratatui TUI phase 2" (33) | 178px | 178px | no |
| "Build broker L1 fair-FIFO shim" (31) | 178px | 178px | no |

All 3 realistic titles now fit without clipping (`scrollWidth ===
clientWidth`), wrapped to 2 lines. Row height grew from a fixed 32px to
36px for 2-line rows (measured `rowBox.height`), matching F-05's original
compact-but-wrapped intent.

## Evidence artifacts

- Rail-only screenshot (post-fix, 3 test titles, no clipping):
  `C:\Users\Owner\AppData\Local\Temp\claude\C--Users-Owner-vox\96f082b4-2578-495a-b574-93143ad4b2f9\scratchpad\task4-title-measure-rail.png`
- Full-page screenshot (same run):
  `C:\Users\Owner\AppData\Local\Temp\claude\C--Users-Owner-vox\96f082b4-2578-495a-b574-93143ad4b2f9\scratchpad\task4-title-measure-full.png`
- Raw scrollWidth/clientWidth JSON for both runs is reproduced verbatim in
  this note (captured via `console.log` in the Playwright test run; the
  measurement spec itself was a temporary, throwaway file — not committed —
  since it only existed to drive this one measurement against a mocked
  session list, per the "no test-only hardcoded data in the committed diff"
  requirement).

## Verdict

**Fix applied: restored 2-line wrap for realistic titles, kept Task 4's
compact gutter/badge/button styling otherwise.** See
`crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx` and the
updated assertions in `ChatSessionRail.test.tsx` (F-05 regression test now
asserts `line-clamp-2`, not `truncate`).
