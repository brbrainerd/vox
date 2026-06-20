---
title: "Frontend coverage ledger — .vox-expressibility of GUI surfaces"
category: "Architecture SSOTs"
status: living
date: 2026-06-20
---

# Frontend Coverage Ledger

Measures Sub-project A's baseline for the Vox-Native Frontend SSOT spec
(`docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md`). One row
per top-level surface directory under `crates/vox-gui/ui/src/components/surfaces/`.
The `crates/vox-gui/tests/frontend_coverage_ledger.rs` currency test fails if a
surface is added or removed without updating this table.

**Status legend** (exactly one per row):
- `expressible` — renderable from `.vox` with today's authoring surface.
- `blocked:reactive-streams` — depends on `vox://*` stream subscription / effect
  deps / cleanup (spec §3.2 critical gap).
- `blocked:interop` — depends on an unfinished ecosystem-import slice (spec §3.3).
- `blocked:mobile` — depends on the mobile-first rule / PWA scaffold (spec §3).
- `blocked:other` — blocked on something else; note it in the Notes column.

> These statuses are the **initial audited estimate**. They are refined as
> Sub-projects B–G land. Counts here are the denominator for the 95–99% target.

| Surface | Status | Notes |
|---|---|---|
| Approvals | blocked:reactive-streams | live approval queue via agent-events stream |
| Browser | blocked:other | CDP frame mirror + native session commands |
| Catalog | expressible | mostly static command catalog rendering |
| Chat | blocked:reactive-streams | streamed tokens + secretary-proposed events |
| Console | blocked:other | PTY streams + xterm.js terminal emulation |
| Coverage | expressible | tabular report rendering |
| Dashboard | blocked:reactive-streams | live orch-status / widget streams |
| Flow | blocked:reactive-streams | live pipeline timeline events |
| Gamify | blocked:reactive-streams | ludus notifications stream |
| Harness | expressible | diff + repo file listing (request/response) |
| Loquela | blocked:reactive-streams | live agent conversation stream |
| Matrix | expressible | static matrix/grid rendering |
| Memory | expressible | recall/reindex request-response |
| Mesh | expressible | trusted-node list CRUD |
| Models | expressible | model cards + routing request-response |
| Policies | expressible | policy list/show request-response |
| Publications | blocked:reactive-streams | scientia-queue / discovery-surfaced events |
| Repository | expressible | repo file/branch listing |
| Research | blocked:reactive-streams | async research start + live progress |
| Runs | expressible | run list/detail request-response |
| Scientia | blocked:reactive-streams | review queue change pings |
| Search | expressible | query/result request-response |
| Settings | expressible | config get/set forms |
| SkillsPlugins | expressible | skill/plugin list rendering |
| Tasks | blocked:reactive-streams | tasks-changed live mutations |

## Summary

- Total surfaces: 25
- `expressible` today: 13
- `blocked:reactive-streams`: 10
- `blocked:other`: 2 (Browser, Console)

The dominant blocker is `reactive-streams` (Sub-project B), confirming the spec's
§8 risk call: closing the `vox://*` authoring gap is the make-or-break for 99%.
