---
title: "FFScript Mutation API Spec (T-041)"
description: "Full TypeScript interface for the FFScriptDoc mutation API: 15 public methods, branded types, JSON-Patch undo/redo, error types, and optimistic-concurrency contract."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation spec for a separate codebase (FableForge)."
last_updated: "2026-04-23"
---

# FFScript Mutation API Spec — T-041 through T-054

Addresses roadmap tasks **T-041, T-042, T-043, T-044, T-045, T-051, T-052, T-054**.

## Effort note (from audit §7)

The roadmap labels T-041 as **M** (½–2 days). The actual scope — 15 public methods, Immer
internals, post-mutation revalidation, undo/redo ring buffer, conflict detection, and type exports
— is realistically **L** (3–7 days). Plan accordingly.

---

## Package structure

```
packages/ffscript/src/
  mutations/
    index.ts          ← public API (this spec)
    doc.ts            ← FFScriptDoc class (Immer wrapper)
    patch.ts          ← JSON Patch helpers + ring buffer
    errors.ts         ← typed error hierarchy
    conflict.ts       ← optimistic concurrency helpers
  index.ts            ← re-exports Panel, Scene, branded types, FFScriptDoc
```

---

## Branded types (`packages/ffscript/src/index.ts`)

```typescript
// Scalar brands — imported everywhere; never use raw `string` for IDs
export type PanelId     = string & { readonly _brand: "PanelId" };
export type NodeId      = string & { readonly _brand: "NodeId" };
export type LineId      = string & { readonly _brand: "LineId" };
export type SceneId     = string & { readonly _brand: "SceneId" };
export type CharacterId = string & { readonly _brand: "CharacterId" };
export type BubbleId    = string & { readonly _brand: "BubbleId" };
export type PlacementId = string & { readonly _brand: "PlacementId" };
export type EdgeId      = string & { readonly _brand: "EdgeId" };

/** Monotonically increasing integer. Each successful mutation increments this. */
export type DocVersion = number & { readonly _brand: "DocVersion" };
```

---

## Error hierarchy (`packages/ffscript/src/mutations/errors.ts`)

```typescript
/** Base for all mutation errors. Always typed; never use raw Error. */
export class FFScriptMutationError extends Error {
  constructor(
    public readonly code: MutationErrorCode,
    message: string,
    public readonly context?: Record<string, unknown>,
  ) {
    super(message);
    this.name = "FFScriptMutationError";
  }
}

export type MutationErrorCode =
  | "VALIDATION_FAILED"      // post-mutation Zod revalidation failed
  | "NOT_FOUND"              // referenced ID does not exist in the doc
  | "DUPLICATE_ID"           // attempted to insert a panel/node with a colliding ID
  | "INVARIANT_VIOLATION"    // e.g. panel order gap, orphaned speaker
  | "VERSION_CONFLICT"       // optimistic concurrency mismatch (T-045)
  | "UNDO_STACK_EMPTY"       // undo() called with nothing to undo
  | "REDO_STACK_EMPTY"       // redo() called with nothing to redo
  | "READ_ONLY"              // doc opened in read-only mode (e.g. during playtest)
  ;
```

---

## JSON Patch types (`packages/ffscript/src/mutations/patch.ts`)

```typescript
import type { Operation } from "fast-json-patch"; // or rfc6902

/** A single reversible mutation record. */
export interface MutationRecord {
  /** Human-readable name for display in command palette history (T-100). */
  label:     string;
  /** Forward patch: transforms doc from before → after. */
  forward:   Operation[];
  /** Reverse patch: transforms doc from after → before. */
  reverse:   Operation[];
  /** Doc version BEFORE this mutation was applied. */
  fromVersion: DocVersion;
  /** Doc version AFTER this mutation was applied. */
  toVersion:   DocVersion;
  timestamp:   number; // Date.now()
}

/** Ring buffer capped at UNDO_LIMIT (default 100). */
export interface UndoRedoStack {
  past:   MutationRecord[]; // index 0 = oldest
  future: MutationRecord[]; // index 0 = most-recently-undone
}
```

---

## Return type for every mutating method

```typescript
/**
 * Every mutation returns this. Callers use the patch to:
 * - update optimistic local state immediately
 * - ship to Convex via ctx.runMutation(internal.panels.applyPatch, { patch: diff.forward })
 * - drive the undo/redo stack
 */
export interface MutationResult<T = void> {
  /** Typed return value specific to each method (e.g. the new PanelId). */
  value:      T;
  /** The JSON Patch operations applied. */
  diff:       MutationRecord;
  /** Doc version after this mutation. */
  newVersion: DocVersion;
}
```

---

## `FFScriptDoc` class — public API (`packages/ffscript/src/mutations/doc.ts`)

```typescript
import type { FFScriptV02 } from "../schema";
import type {
  PanelId, NodeId, LineId, SceneId, CharacterId, BubbleId, PlacementId, DocVersion
} from "../index";
import type { MutationResult } from "./patch";
import type { AspectRatio, PanelMode, BubbleStyle, Transition } from "../schema";

export class FFScriptDoc {

  // ── Construction ──────────────────────────────────────────────────────────

  /** Open a doc for editing. Pass `readOnly: true` during playtest (T-222). */
  static open(raw: FFScriptV02, opts?: { readOnly?: boolean }): FFScriptDoc;

  /** Serialise to a canonical byte string (deterministic key order, no whitespace). */
  serialize(): string;

  /** SHA-256 hash of serialize(). Used as the R2 content-addressable key (T-201). */
  hash(): Promise<string>;

  /** Current doc version. */
  get version(): DocVersion;

  // ── Panel mutations ───────────────────────────────────────────────────────

  /**
   * Insert a new blank panel immediately after `afterId`.
   * If afterId is null, inserts at position 0.
   * Repairs order values of all subsequent panels (no gaps).
   * Returns the new panel's ID.
   */
  insertPanel(params: {
    afterId:     PanelId | null;
    mode?:       PanelMode;       // default "vn"
    aspectRatio?: AspectRatio;    // default "16:9"
    sceneId?:    SceneId;         // if omitted, panel inherits the scene of `afterId`
  }): MutationResult<PanelId>;

  /**
   * Delete a panel and all its child bubbles.
   * Removes the panel from its scene's panelIds list.
   * Removes FlowGraph edges that targeted this panel's dialogue nodes.
   * Returns the IDs of removed nodes and edges for the caller to clean up
   * in the FlowGraph if needed.
   */
  deletePanel(id: PanelId): MutationResult<{
    removedNodeIds: NodeId[];
    removedEdgeIds: EdgeId[];
    removedBubbleIds: BubbleId[];
    removedPlacementIds: PlacementId[];
  }>;

  /**
   * Move a panel to a new order position.
   * All other panels' order values are shifted to maintain contiguity.
   */
  movePanel(id: PanelId, toOrder: number): MutationResult<void>;

  /**
   * Deep-clone a panel, assigning new UUIDs to the clone and all its children.
   * Inserts the clone immediately after the source panel.
   * Background assetKey is shared (not duplicated in R2) until the clone is regenerated.
   */
  duplicatePanel(id: PanelId): MutationResult<PanelId>;

  /**
   * Update scalar fields on a panel (background, mode, aspectRatio, transition,
   * contentRating, metadata, caption, bounds).
   * Uses a partial update — only supplied keys are changed.
   */
  updatePanel(
    id: PanelId,
    patch: Partial<Pick<Panel,
      | "background" | "foreground" | "mode" | "aspectRatio" | "customDims"
      | "transition" | "contentRating" | "caption" | "bounds" | "metadata"
    >>
  ): MutationResult<void>;

  // ── Dialogue mutations ────────────────────────────────────────────────────

  /**
   * Add a dialogue line (DialogueNode) to a panel.
   * If afterLineId is supplied, inserts after that line; otherwise appends.
   * Creates a BubbleSchema entry in panel.bubbles with auto-positioned placement.
   * Returns the new NodeId and BubbleId.
   */
  insertDialogueLine(params: {
    panelId:     PanelId;
    speakerId:   CharacterId | null; // null = narrator / caption
    text:        string;
    afterLineId?: LineId;
  }): MutationResult<{ nodeId: NodeId; bubbleId: BubbleId }>;

  /**
   * Remove a dialogue line and its associated bubble.
   * Repairs readingOrder of remaining bubbles in the panel.
   */
  deleteDialogueLine(id: LineId): MutationResult<void>;

  /**
   * Swap the speaker on an existing dialogue line.
   * speakerId: null → convert to narrator caption (bubble style becomes "caption").
   */
  reassignSpeaker(params: {
    lineId:    LineId;
    speakerId: CharacterId | null;
  }): MutationResult<void>;

  // ── Choice / branch mutations ─────────────────────────────────────────────

  /**
   * Insert a choice node in a panel with 2–6 options.
   * Each option has display text and an optional target panel ID.
   */
  insertChoice(params: {
    panelId: PanelId;
    options:  Array<{ text: string; targetPanelId?: PanelId }>;
  }): MutationResult<NodeId>;

  /**
   * Wire a choice option to a target panel.
   * Creates a FlowGraph edge from choiceNode → targetPanel's first dialogueNode.
   */
  setBranchTarget(params: {
    choiceId:     NodeId;
    optionIndex:  number;
    targetPanelId: PanelId;
  }): MutationResult<EdgeId>;

  // ── Scene mutations ───────────────────────────────────────────────────────

  /**
   * Create a new scene with an optional title.
   * Inserts after `afterSceneId` (or at the end if null).
   */
  insertScene(params: {
    title?:      string;
    afterSceneId?: SceneId;
  }): MutationResult<SceneId>;

  /**
   * Move a panel from its current scene to `targetSceneId`.
   * Respects order within the target scene (appends if toOrder is omitted).
   */
  movePanelToScene(params: {
    panelId:      PanelId;
    targetSceneId: SceneId;
    toOrder?:     number;
  }): MutationResult<void>;

  // ── Character placement mutations ─────────────────────────────────────────

  /**
   * Place a character on the panel canvas.
   * Auto-assigns z-index (one above the current highest in the panel).
   * Returns the new PlacementId.
   */
  placeCharacter(params: {
    panelId:     PanelId;
    characterId: CharacterId;
    pose?:       string;   // default "neutral"
    x:           number;   // fraction of panel width
    y:           number;   // fraction of panel height
    scale?:      number;   // default 1.0
    flipX?:      boolean;  // default false
  }): MutationResult<PlacementId>;

  /**
   * Update position/scale/pose of an existing character placement.
   */
  updatePlacement(
    id: PlacementId,
    patch: Partial<Pick<CharacterPlacement, "pose" | "x" | "y" | "scale" | "flipX" | "zIndex">>
  ): MutationResult<void>;

  /** Remove a character placement from a panel. */
  removePlacement(id: PlacementId): MutationResult<void>;

  // ── Bubble mutations ──────────────────────────────────────────────────────

  /**
   * Reposition a speech bubble and/or its tail.
   */
  moveBubble(params: {
    panelId: PanelId;
    bubbleId: BubbleId;
    x?: number; y?: number;
    w?: number; h?: number;
    tailX?: number; tailY?: number;
  }): MutationResult<void>;

  /**
   * Change bubble style (e.g. "round" → "thought").
   */
  setBubbleStyle(params: {
    bubbleId: BubbleId;
    style:    BubbleStyle;
  }): MutationResult<void>;

  /**
   * Set the reading order of a bubble within its panel.
   * Other bubbles in the panel are shifted to maintain contiguity.
   */
  setBubbleReadingOrder(params: {
    bubbleId:   BubbleId;
    newOrder:   number; // 1-indexed
  }): MutationResult<void>;

  // ── Undo / redo (T-044) ───────────────────────────────────────────────────

  /**
   * Undo the last mutation. Returns the reverse patch.
   * Throws FFScriptMutationError("UNDO_STACK_EMPTY") if nothing to undo.
   */
  undo(): MutationResult<void>;

  /**
   * Redo the last undone mutation. Returns the forward patch.
   * Throws FFScriptMutationError("REDO_STACK_EMPTY") if nothing to redo.
   */
  redo(): MutationResult<void>;

  /** True if undo() would succeed. */
  get canUndo(): boolean;

  /** True if redo() would succeed. */
  get canRedo(): boolean;

  /** The label of the next undo operation (for menu display). */
  get undoLabel(): string | null;

  /** The label of the next redo operation (for menu display). */
  get redoLabel(): string | null;

  // ── Optimistic concurrency (T-045) ───────────────────────────────────────

  /**
   * Assert that the doc is at the expected version before mutating.
   * Call this before any mutation that must win a concurrent-edit race.
   *
   * Throws FFScriptMutationError("VERSION_CONFLICT") if version !== expectedVersion.
   *
   * Usage:
   *   doc.assertVersion(lastKnownVersion);
   *   doc.insertPanel({ afterId: null });
   */
  assertVersion(expectedVersion: DocVersion): void;

  // ── Batch mutations ───────────────────────────────────────────────────────

  /**
   * Run multiple mutations in a single atomic block.
   * If any mutation throws, the entire block is rolled back.
   * Returns a single combined MutationRecord spanning all operations.
   */
  batch(
    label:   string,
    fn:      (doc: FFScriptDoc) => void
  ): MutationResult<void>;
}
```

---

## Internal implementation notes (for the team, not the API consumer)

**Immer integration (T-042)**

```typescript
import { produce, enableMapSet } from "immer";
enableMapSet();

// Inside each mutating method:
const nextState = produce(this._state, (draft) => {
  // ... apply changes to draft ...
});
const diff = computeJsonPatch(this._state, nextState); // rfc6902 diff
this._undoStack.push({ label, forward: diff, reverse: inversePatch(diff), ... });
this._state = nextState;
```

**Post-mutation revalidation (T-043)**

Every mutation calls `FFScriptSchema.safeParse(nextState)` before committing. On failure:
- the Immer draft is discarded
- a `FFScriptMutationError("VALIDATION_FAILED")` is thrown with the Zod error details

The `forgivingValidator` (T-051) pre-processes LLM-generated blobs before the strict validator
runs: it auto-repairs duplicate panel IDs (UUID re-generation), fills missing required defaults,
and removes null values from non-nullable arrays. This runs only at document *open* time, never
after a user-initiated mutation.

**Ring buffer limit**

Default `UNDO_LIMIT = 100`. Exceeding this drops the oldest `past` entry. Configurable per doc.

**Convex integration pattern**

```typescript
// In a React component:
const doc = useFFScriptDoc(gameId);       // reactive Convex subscription → FFScriptDoc
const result = doc.insertPanel({ ... });  // optimistic local update
await convex.mutation(api.panels.applyPatch, {
  gameId,
  patch:       result.diff.forward,
  fromVersion: result.diff.fromVersion,
}); // server applies same patch or returns VERSION_CONFLICT
```

---

## Test coverage requirements (T-052)

`tests/unit/ffscript/mutations/` must include:

| Test file | Coverage requirement |
|-----------|---------------------|
| `panel-crud.test.ts` | insertPanel, deletePanel, movePanel, duplicatePanel — all edge cases |
| `panel-order.test.ts` | 0-indexed contiguity maintained after every operation; no gaps |
| `dialogue.test.ts` | insertDialogueLine, deleteDialogueLine, reassignSpeaker |
| `choice.test.ts` | insertChoice, setBranchTarget; invalid option index throws |
| `scene.test.ts` | insertScene, movePanelToScene |
| `placement.test.ts` | placeCharacter, updatePlacement, removePlacement |
| `bubble.test.ts` | moveBubble, setBubbleStyle, setBubbleReadingOrder |
| `undo-redo.test.ts` | undo/redo round-trip; UNDO_STACK_EMPTY; redo clears future on new mutation |
| `batch.test.ts` | batch rollback on inner failure; nested batch rejected |
| `concurrency.test.ts` | assertVersion throws on mismatch; succeeds on match |
| `revalidation.test.ts` | any mutation that would violate an invariant throws VALIDATION_FAILED |

---

## Acceptance criteria (T-041 — corrected effort: L)

- `FFScriptDoc.open(validV02Doc)` returns a doc at version N.
- `doc.insertPanel({ afterId: null })` returns a `MutationResult<PanelId>` where the new panel
  is at order 0 and all existing panels have been shifted by +1.
- `doc.deletePanel(id)` on a panel with 2 bubbles returns `removedBubbleIds.length === 2`.
- `doc.undo()` after `insertPanel` brings the doc back to its prior state and `doc.canUndo` returns
  false if the stack was empty before the insert.
- `doc.batch("my op", (d) => { d.insertPanel(...); d.insertPanel(...); })` produces a single
  MutationRecord spanning both inserts.
- A mutation that would orphan a speaker throws `VALIDATION_FAILED`.
- `doc.assertVersion(wrongVersion)` throws `VERSION_CONFLICT`.
- `doc.serialize()` is deterministic: calling it twice on the same doc produces identical strings.

---

*Spec produced 2026-04-23. Addresses T-041–T-045, T-051–T-052, T-054.*
