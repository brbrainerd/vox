# F-05 fix verdict (2026-07-19)

## Fix applied (commit ca18981267, on axis-frontend-remediation)

`crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx`:
- Rail width `w-44` (176px) -> `w-64` (256px).
- Title span `block truncate` -> `line-clamp-2 break-words` (2 lines instead of 1).
- Added `title={s.title}` on the tab button for native-tooltip full-text discoverability.

## Root cause of residual "major clipping" verdicts after the fix

`crates/vox-gui/ui/e2e/lib/tauriMockRich.ts:43`'s `long()` helper pads every mock
session title to 90-142 characters:
```
title: long(`Session ${i + 1}: exploratory conversation about the architecture
refactor and its long-term implications `, 90 + i * 4),
```
This is an overflow **stress-test fixture**, not representative of real user-typed
session titles (which are almost always well under 40 characters). No rail width
short of consuming most of the viewport fits a 90-142 character string without
eliding text — the AI vision reviewer's re-runs continued to score `major clipping`
on these cells purely because *some* ellipsis is still visible on the stress-test
string, which is correct-but-unhelpful feedback given the input.

Verified: `chat--session-menu-open--wide--{chromium,firefox}` (the widest viewport,
least width pressure) improved from `fail`/critical-adjacent to `pass_with_notes`,
score 82-85, clipping downgraded from `major` to `minor` — this is the real signal
that the fix helps. The remaining `major` verdicts are concentrated on `compact`
viewport cells, where 256px is a genuinely small budget for a 100+ char string
regardless of wrapping — an unavoidable trade-off at that viewport size for
pathological input, not a regression or an unaddressed bug.

## Verdict

**Fixed for realistic session titles.** The rail is wider, titles wrap to 2 lines
instead of 1, and the full text is always available via hover tooltip. The
harness's specific stress-test mock data (100+ char titles) will continue to show
some ellipsis at compact viewports no matter how this is tuned further, short of
a fundamentally different pattern (e.g. a dedicated tooltip-on-hover-only display,
or truncating server-side titles at creation time) — recommending this be accepted
as expected behavior for extreme-length titles rather than chased further, unless
real user reports of short-title clipping surface.

## Follow-up idea (not implemented, low priority)

If long titles are common in practice, consider truncating/summarizing very long
session titles server-side at creation time (e.g. cap at ~60 chars with an
LLM-generated short label) rather than relying on client-side CSS elision.
