# vox-gui Wave 3 — Chat and Approvals Implementation Plan

**Goal:** Apply 24-item checklist to Chat + Loquela + Approvals surfaces.

**Paths:** `surfaces/Chat/`, `surfaces/Loquela/`, `surfaces/Approvals/`.

---

## Task 1: Chat composer a11y

- [ ] Slash router keyboard hints (`aria-keyshortcuts`)
- [ ] Message list `aria-live="polite"` for streaming tokens
- [ ] All icon buttons `type="button"` + `aria-label`

---

## Task 2: Approvals IPC

- [ ] Wrap pending approvals fetch in `useVoxQuery`
- [ ] `<Async>` for empty/pending/error on ApprovalsView

---

## Exit criteria

- `ChatSurface.test.tsx`, `ApprovalsView.test.tsx`, `Loquela.test.tsx` green
- Playwright: open Chat from sidebar
