---
title: "FFScript Panel Schema Spec (v0.2.0)"
description: "Authoritative Zod schema for the Panel type introduced in FFScript v0.2.0, resolving the version-naming conflict identified in the 2026-04-23 audit."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation spec for a separate codebase (FableForge)."
last_updated: "2026-04-23"
---

# FFScript Panel Schema Spec — v0.2.0

Addresses roadmap tasks **T-021**, **T-022**, **T-025**, **T-031**, **T-035**.

## Version naming decision (resolves audit §1.1)

The roadmap's T-021 acceptance criteria mixed two incompatible version strings.
**Decision:** use semver `0.x` throughout until the Panel contract is proven in production.

| Event | Version |
|-------|---------|
| Current (pre-Panel) | `0.1.0` |
| This spec (Panel added) | `0.2.0` |
| After battle-testing + linter stable | `1.0.0` (future task, replaces the missing T-268b reference) |

The schema version regex in `src/lib/ffscript/schema.ts` therefore stays as
`/^\d+\.\d+\.\d+$/` (full semver) rather than the current `/^0\.\d+\.\d+$/` so that `1.0.0`
is a valid future version without another regex change.

---

## Zod schema — drop-in for `src/lib/ffscript/schema.ts`

```typescript
import { z } from "zod";

// ─── Branded scalars ──────────────────────────────────────────────────────────

/** All IDs are UUID v4 strings. Using branded types prevents accidental mix-ups. */
const UUIDSchema = z.string().uuid();
export type PanelId    = z.infer<typeof UUIDSchema> & { readonly _brand: "PanelId" };
export type NodeId     = z.infer<typeof UUIDSchema> & { readonly _brand: "NodeId" };
export type LineId     = z.infer<typeof UUIDSchema> & { readonly _brand: "LineId" };
export type SceneId    = z.infer<typeof UUIDSchema> & { readonly _brand: "SceneId" };
export type CharacterId = z.infer<typeof UUIDSchema> & { readonly _brand: "CharacterId" };
export type BubbleId   = z.infer<typeof UUIDSchema> & { readonly _brand: "BubbleId" };
export type PlacementId = z.infer<typeof UUIDSchema> & { readonly _brand: "PlacementId" };

// ─── Aspect ratio (T-031) — replaces hardcoded workflow dims ─────────────────

export const AspectRatioSchema = z.enum([
  "16:9",   // → 1344×768
  "9:16",   // → 768×1344
  "1:1",    // → 1024×1024
  "4:3",    // → 1152×896
  "3:4",    // → 896×1152
  "21:9",   // → 1536×640
  "2:3",    // → 832×1248
  "custom", // → bounds.w × bounds.h (clamped 512–2048 each axis)
]);
export type AspectRatio = z.infer<typeof AspectRatioSchema>;

/** Maps an AspectRatio to a canonical {w,h} for workflow dispatch. */
export function aspectRatioDims(r: AspectRatio, customW?: number, customH?: number) {
  const map: Record<Exclude<AspectRatio, "custom">, { w: number; h: number }> = {
    "16:9":  { w: 1344, h: 768  },
    "9:16":  { w: 768,  h: 1344 },
    "1:1":   { w: 1024, h: 1024 },
    "4:3":   { w: 1152, h: 896  },
    "3:4":   { w: 896,  h: 1152 },
    "21:9":  { w: 1536, h: 640  },
    "2:3":   { w: 832,  h: 1248 },
  };
  if (r === "custom") {
    const clamp = (v: number) => Math.min(2048, Math.max(512, v));
    return { w: clamp(customW ?? 1024), h: clamp(customH ?? 1024) };
  }
  return map[r];
}

// ─── Background layer ─────────────────────────────────────────────────────────

export const BackgroundSchema = z.object({
  /** R2 key for the generated (or user-uploaded) background image. */
  assetKey:    z.string().min(1),
  /** The text prompt used to generate this background. */
  prompt:      z.string(),
  /** Model ID from the model catalog (never a raw string — from LLM_MODELS or IMAGE_MODELS). */
  modelId:     z.string(),
  /** Generation seed. null = unseeded / random. */
  seed:        z.number().int().nullable(),
  /** If true, seed is preserved across re-generations of this panel (T-030). */
  seedLocked:  z.boolean().default(false),
  /** SAM-2 or rembg foreground mask key in R2 (null until auto-masking has run). */
  maskCacheKey: z.string().nullable().default(null),
});
export type Background = z.infer<typeof BackgroundSchema>;

// ─── Character placement ──────────────────────────────────────────────────────

export const CharacterPlacementSchema = z.object({
  id:          UUIDSchema,
  characterId: UUIDSchema,
  /** Named pose from this character's pose palette (e.g. "neutral", "surprised"). */
  pose:        z.string().default("neutral"),
  /** Horizontal position as a fraction of panel width [0.0 = left edge, 1.0 = right edge].
   *  Linter warns when outside [-0.2, 1.2] (T-037). */
  x:           z.number(),
  /** Vertical position as a fraction of panel height [0.0 = top, 1.0 = bottom]. */
  y:           z.number(),
  /** Scale relative to panel height. 1.0 = character fills the panel height. */
  scale:       z.number().positive().default(1.0),
  /** Mirror the character sprite horizontally (T-177). */
  flipX:       z.boolean().default(false),
  /** Z-index within the foreground layer (higher = in front). */
  zIndex:      z.number().int().default(0),
});
export type CharacterPlacement = z.infer<typeof CharacterPlacementSchema>;

export const ForegroundSchema = z.object({
  characterPlacements: z.array(CharacterPlacementSchema),
});

// ─── Speech bubbles ───────────────────────────────────────────────────────────

export const BubbleStyleSchema = z.enum([
  "round",    // standard speech bubble
  "square",   // rectangular / comic-book style
  "thought",  // cloud outline (T-166)
  "shout",    // spiky / jagged
  "whisper",  // dashed outline
  "caption",  // narrator strip — no tail (T-165)
]);

export const BubbleSchema = z.object({
  id:     UUIDSchema,
  /** The dialogue LineId this bubble is bound to. Linter errors if missing (T-035). */
  lineId: UUIDSchema,
  /** Position of the bubble body top-left corner as fractions of panel dims. */
  x: z.number(), y: z.number(),
  /** Size as fractions of panel dims. */
  w: z.number().positive(), h: z.number().positive(),
  /** Bubble tail tip as fractions of panel dims.
   *  Required for all non-caption styles; null only when style = "caption".
   *  Linter warns when tail does not land near the speaker's placement (T-039). */
  tailX: z.number().nullable(),
  tailY: z.number().nullable(),
  style: BubbleStyleSchema.default("round"),
  /** Reading order within this panel (1-indexed). Determines TTS playback sequence. */
  readingOrder: z.number().int().positive(),
  /** Typography preset key (e.g. "manga-bold", "handwritten"). */
  fontPreset: z.string().nullable().default(null),
}).refine(
  (b) => b.style === "caption" || (b.tailX !== null && b.tailY !== null),
  { message: "Non-caption bubbles must have tailX and tailY set." }
);
export type Bubble = z.infer<typeof BubbleSchema>;

// ─── Caption (narrator bar) ───────────────────────────────────────────────────

export const CaptionPositionSchema = z.enum(["top", "bottom", "floating"]);

export const CaptionSchema = z.object({
  text:       z.string(),
  position:   CaptionPositionSchema.default("top"),
  fontPreset: z.string().nullable().default(null),
});

// ─── Panel mode ───────────────────────────────────────────────────────────────

export const PanelModeSchema = z.enum([
  "vn",         // Visual novel: dialogue displayed below the image
  "comic",      // Comic: speech bubbles composited on the image
  "cinematic",  // Letterboxed auto-scroll with optional VACE video
]);
export type PanelMode = z.infer<typeof PanelModeSchema>;

// ─── Transition ───────────────────────────────────────────────────────────────

export const TransitionSchema = z.enum(["cut", "fade", "wipe", "iris"]);

// ─── Content rating (per-panel, overrides game default — T-149) ──────────────

export const ContentRatingSchema = z.enum([
  "sfw",   // safe for all audiences
  "pg13",  // mild suggestive content
  "r18",   // adult content; requires server-side age verification (T-226)
]);

// ─── The Panel ────────────────────────────────────────────────────────────────

export const PanelSchema = z.object({
  /** Stable UUID — never reused even if the panel is deleted and a new one created. */
  id: UUIDSchema,

  /** 0-indexed position in reading order. The mutation API enforces uniqueness and
   *  contiguity; gaps are not allowed (T-034). */
  order: z.number().int().nonneg(),

  /** Optional back-link to a FlowGraph node cluster (DialogueNode group).
   *  null = panel is a pure visual interlude with no scripted dialogue. */
  beatId: UUIDSchema.nullable().default(null),

  /** FlowGraph node IDs whose dialogue is rendered during this panel. */
  dialogueNodeIds: z.array(UUIDSchema).default([]),

  /** SceneGraph element IDs that belong to this panel. */
  sceneElementIds: z.array(UUIDSchema).default([]),

  /** Layout in a comic grid (fractions of canvas; used by PanelGrid layout engine). */
  bounds: z.object({
    x: z.number().nonneg(),
    y: z.number().nonneg(),
    w: z.number().positive(),
    h: z.number().positive(),
  }).nullable().default(null),

  aspectRatio: AspectRatioSchema.default("16:9"),
  /** Only meaningful when aspectRatio = "custom". */
  customDims: z.object({ w: z.number().int(), h: z.number().int() }).nullable().default(null),

  background: BackgroundSchema.nullable().default(null),
  foreground:  ForegroundSchema.default({ characterPlacements: [] }),
  bubbles:     z.array(BubbleSchema).default([]),
  caption:     CaptionSchema.nullable().default(null),

  transition: TransitionSchema.default("cut"),
  mode:       PanelModeSchema.default("vn"),
  contentRating: ContentRatingSchema.nullable().default(null), // null = inherit game default

  metadata: z.object({
    /** Locked seed for stylistic consistency across regenerations (T-030). */
    lockedSeed:        z.number().int().nullable().default(null),
    /** Curator tags for batch-regen art direction (T-091). */
    artDirectionTags:  z.array(z.string()).default([]),
    /** When true, this panel is excluded from bulk-regen operations (T-088). */
    locked:            z.boolean().default(false),
    /** Last 10 generation seeds for the "restore" dice button (T-143). */
    seedHistory:       z.array(z.number().int()).max(10).default([]),
  }).default({}),
});

export type Panel = z.infer<typeof PanelSchema>;

// ─── Scene grouping (T-033) ───────────────────────────────────────────────────

export const SceneSchema = z.object({
  id:    UUIDSchema,
  title: z.string(),
  /** Ordered list of panel IDs in this scene. A panel belongs to exactly one scene. */
  panelIds: z.array(UUIDSchema),
  /** Music theme ID active for this scene (from the game's music.themes array). */
  backgroundMusicThemeId: z.string().nullable().default(null),
});
export type Scene = z.infer<typeof SceneSchema>;

// ─── FFScript v0.2 document extension ────────────────────────────────────────

/**
 * Extend the existing FFScriptV1Schema with these two new top-level arrays.
 * The existing schema fields (characters, flowGraph, sceneGraph, etc.) are unchanged.
 *
 * In src/lib/ffscript/schema.ts:
 *
 *   export const FFScriptSchema = FFScriptV1Schema.extend({
 *     version: z.literal("0.2.0"),
 *     panels:  z.array(PanelSchema).default([]),
 *     scenes:  z.array(SceneSchema).default([]),
 *   });
 */

// ─── Invariants documented as runtime assertions ──────────────────────────────

/**
 * PANEL_INVARIANTS — must hold after every mutation (enforced by T-043 post-mutation revalidation):
 *
 * 1. panel.order values form a contiguous 0-indexed sequence with no gaps or duplicates.
 * 2. Every panel.bubbles[i].lineId points to a node ID present in panel.dialogueNodeIds
 *    OR to a node in the game's flowGraph (for cross-panel narrative lines).
 * 3. Every bubble's speaker (derived from its lineId's node.speakerId) must be in
 *    the game's characters[] array.  (T-035)
 * 4. If panel.background is non-null, panel.background.assetKey must resolve to a
 *    generatedAssets row with status "ready".  (Linter warning T-036, not hard error)
 * 5. panel.contentRating = "r18" requires the game's contentRating to be "r18".
 *    A panel cannot be more permissive than its parent game.  (T-149)
 * 6. Bubble readingOrder values within a single panel form a contiguous 1-indexed sequence.
 */
```

---

## Migration: v0.1.0 → v0.2.0

File: `src/lib/ffscript/migrations/v0_1_to_v0_2.ts`

```typescript
import type { FFScriptV1 } from "../schema";         // old type
import type { Panel, Scene } from "./panel-schema";  // new types (this file)
import { v4 as uuid } from "uuid";

/**
 * Auto-constructs one Panel per DialogueNode (or per contiguous dialogue run)
 * from the existing flowGraph. Panels start with no background (linter warning
 * fires, driving users to generate images). All panels land in a single default Scene.
 */
export function migrateV01toV02(doc: FFScriptV1): FFScriptV02 {
  const dialogueNodes = doc.flowGraph.nodes.filter(
    (n) => n.type === "dialogue" || n.type === "choice"
  );

  const panels: Panel[] = dialogueNodes.map((node, idx) => ({
    id:              uuid() as PanelId,
    order:           idx,
    beatId:          null,
    dialogueNodeIds: [node.id as NodeId],
    sceneElementIds: [],
    bounds:          null,
    aspectRatio:     "16:9",
    customDims:      null,
    background:      null,   // triggers linter T-036 → user must generate
    foreground:      { characterPlacements: [] },
    bubbles:         [],
    caption:         null,
    transition:      "cut",
    mode:            "vn",
    contentRating:   null,
    metadata: {
      lockedSeed:       null,
      artDirectionTags: [],
      locked:           false,
      seedHistory:      [],
    },
  }));

  const defaultScene: Scene = {
    id:                     uuid() as SceneId,
    title:                  "Act 1",
    panelIds:               panels.map((p) => p.id),
    backgroundMusicThemeId: null,
  };

  return {
    ...doc,
    version: "0.2.0",
    panels,
    scenes: [defaultScene],
  };
}
```

---

## Acceptance criteria (T-021 / T-022 — corrected)

- `PanelSchema.parse(validPanel)` succeeds for a fully-populated panel.
- `PanelSchema.parse(minimalPanel)` succeeds where only `id` and `order` are set (all
  other fields default correctly).
- A panel with `style ≠ "caption"` and `tailX: null` throws a Zod error.
- `aspectRatioDims("16:9")` returns `{ w: 1344, h: 768 }`.
- `aspectRatioDims("custom", 800, 600)` returns `{ w: 800, h: 600 }`.
- Running `migrateV01toV02` on any existing FFScript v0.1 document produces a valid v0.2
  document that passes `FFScriptSchema.parse`.
- Running migration twice on the same document is idempotent (panels list is not doubled).
- All 15 missing-context file claims in audit §6 that touch schema must be verified before
  this migration runner is deployed to production.

---

*Spec produced 2026-04-23. Addresses T-021, T-022, T-025, T-031, T-035. FableForge codebase
not directly verified; adapt field names to match real schema before merging.*
