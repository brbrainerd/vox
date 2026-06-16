# vox-gui Wave 5 — Config and Ops Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement task-by-task.

**Goal:** Complete IPC migration and `<Async>` patterns for Policies, Memory, Mesh, Models, Repository.

**Prerequisites:** Navigation layout Task 4 (Policies two-rail); Wave 1 query adoption recommended.

---

## Task 1: Memory transport migration

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts`
- Modify: `crates/vox-gui/ui/src/transport.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.tsx`
- Modify: `crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts`

- [ ] **Step 1: Add transport methods**

```typescript
getMemoryStatus(): Promise<MemoryStatusPayload> {
  return invoke('get_memory_status');
}
mnemosyneReindex(): Promise<void> {
  return invoke('mnemosyne_reindex');
}
```

- [ ] **Step 2: Failing test** — MemoryView mock transport, not invoke

- [ ] **Step 3: Replace invokes in MemoryView**; recall already can use `voxTransport.voxSearchQuery`

- [ ] **Step 4: Remove `Memory/MemoryView.tsx` from ipc allowlist**

- [ ] **Step 5: `pnpm test src/components/surfaces/Memory/` — PASS**

---

## Task 2: `useMemoryStatus` query hook

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useMemoryStatus.ts`
- Create: `crates/vox-gui/ui/src/hooks/useMemoryStatus.test.ts`

- [ ] **Step 1: Failing test** with QueryClientProvider wrapper
- [ ] **Step 2: `useVoxQuery(['memory','status'], () => voxTransport.getMemoryStatus())`**
- [ ] **Step 3: MemoryView uses hook; wrap shard panel in `<Async>`**
- [ ] **Step 4: Commit**

---

## Task 3: Mesh / Models / Repository `<Async>`

For each surface (`MeshView.tsx`, `ModelsView.tsx`, `RepositoryView.tsx`):

- [ ] Add `loading` + `error` props or internal `useVoxQuery`
- [ ] Wrap primary data panel in `<Async>` + `EmptyState`
- [ ] Extend existing `*.test.tsx` with error path assertion

---

## Exit criteria

- [ ] MemoryView off ipc allowlist
- [ ] Policies two-rail (navigation plan Task 4)
- [ ] All five surfaces use `<Async>` for IPC-backed panels
