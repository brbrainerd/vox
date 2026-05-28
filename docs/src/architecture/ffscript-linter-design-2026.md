---
title: "FFScript Linter Engine Design (T-046)"
description: "TypeScript interface for the pluggable FFScript linter: LintRule shape, severity levels, default rule catalogue with per-rule acceptance criteria, and integration points."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation spec for a separate codebase (FableForge)."
last_updated: "2026-04-23"
---

# FFScript Linter Engine Design — T-046

Addresses roadmap tasks **T-046, T-047, T-048, T-049, T-050, T-035, T-036, T-037, T-038, T-039**.

## Effort note (from audit §7)

The roadmap labels T-046 as **M**. A pluggable rule engine with 10+ default rules, a CLI,
autofix, and publish-gate integration is realistically **L** (3–7 days). Budget accordingly.

---

## Package structure

```
packages/ffscript/src/
  linter/
    index.ts          ← public API
    engine.ts         ← LintEngine class
    rules/
      index.ts        ← exports all default rules
      orphaned-speaker.ts
      missing-background.ts
      off-canvas-character.ts
      dangling-beat.ts
      bubble-tail-off-target.ts
      unreachable-node.ts
      infinite-loop.ts
      undeclared-variable.ts
      duplicate-panel-id.ts
      panel-content-rating.ts
      bubble-reading-order.ts
```

---

## Core types (`packages/ffscript/src/linter/index.ts`)

```typescript
import type { FFScriptV02 } from "../schema";
import type { PanelId, NodeId, BubbleId, PlacementId } from "../index";

// ─── Severity ─────────────────────────────────────────────────────────────────

export type LintSeverity =
  | "error"    // blocks publish (T-048); must be fixed before submittal
  | "warning"  // does not block publish; surfaced in wizard health bar (T-047)
  | "info"     // advisory only; shown in editor gutter
  ;

// ─── Location — pinpoints the offending element ───────────────────────────────

export interface LintLocation {
  panelId?:     PanelId;
  nodeId?:      NodeId;
  bubbleId?:    BubbleId;
  placementId?: PlacementId;
  /** Human-readable path for display (e.g. "panels[3].bubbles[1].lineId"). */
  path?:        string;
}

// ─── Serializable fix descriptor (mirrors Vox DiagnosticFix) ─────────────────
//
// Fixes are JSON-serializable so they can be:
//   • stored on the gameDrafts.lintReport Convex row
//   • shipped to the CLI `--fix` mode over the wire
//   • applied in a replay loop without closures
//
// The engine resolves a fix descriptor → FFScriptDoc mutations at apply-time,
// not at check-time. Rules emit descriptors; the engine knows how to apply them.

export type FixKind =
  | { type: "regenerate_panel_id"; panelId: string }
  | { type: "repair_panel_order" }                    // reassign 0..N-1 preserving sort
  | { type: "repair_reading_order"; panelId: string } // reassign 1..N preserving sort
  | { type: "remove_placement"; placementId: string }
  | { type: "set_speaker_null"; bubbleId: string }    // demote orphaned speaker to narrator
  ;

export interface LintFix {
  /** Human-readable label shown in the UI "Apply fix" button. */
  label:   string;
  /** Machine-applicable repair descriptor. */
  kind:    FixKind;
}

// ─── A single lint finding ────────────────────────────────────────────────────

export interface LintResult {
  /** Unique rule identifier (snake_case, stable across versions). */
  ruleId:    string;
  severity:  LintSeverity;
  /** Short human-readable message (≤120 chars). */
  message:   string;
  /** Optional extended explanation with a docs link. */
  detail?:   string;
  location:  LintLocation;
  /**
   * If the rule supports autofix (T-050), supply a serializable fix descriptor.
   * The engine's fix() method resolves this to FFScriptDoc mutations.
   * Must be undefined when LintRule.fixable = false.
   */
  fix?:      LintFix;
}

// ─── A lint rule ──────────────────────────────────────────────────────────────

export interface LintRule {
  /** Unique stable ID (snake_case). Convention: "ff/<category>/<name>". */
  id:          string;
  /** One-line description shown in rule catalogues and --help. */
  description: string;
  /** Default severity. Consumers can override per-game. */
  severity:    LintSeverity;
  /** If true, the rule supports --fix (T-050). */
  fixable:     boolean;
  /**
   * Run the rule against the document.
   * Return an array of findings (empty = no issues).
   * Must be a pure function — no side effects.
   */
  check: (doc: FFScriptV02) => LintResult[];
}

// ─── Engine options ───────────────────────────────────────────────────────────

export interface LintEngineOptions {
  /** Override severity for specific rules. */
  severityOverrides?: Partial<Record<string, LintSeverity>>;
  /** Disable specific rules entirely. */
  disabledRules?:     string[];
  /** Add project-specific custom rules. */
  customRules?:       LintRule[];
  /** When true, only "error" findings are returned (faster; used in publish gate). */
  errorsOnly?:        boolean;
}

// ─── Engine output ────────────────────────────────────────────────────────────

export interface LintReport {
  findings:     LintResult[];
  errorCount:   number;
  warningCount: number;
  infoCount:    number;
  /** True if any finding has severity "error". Publish gate checks this. */
  hasErrors:    boolean;
  /** Ms elapsed. */
  durationMs:   number;
}
```

---

## `LintEngine` class (`packages/ffscript/src/linter/engine.ts`)

```typescript
export class LintEngine {
  /** Instantiate with the default rule set + optional config. */
  constructor(opts?: LintEngineOptions);

  /** Run all enabled rules against a document. Returns a complete report. */
  lint(doc: FFScriptV02): LintReport;

  /**
   * Apply all fixable LintFix descriptors to the doc (T-050).
   * Resolves each FixKind → FFScriptDoc mutation call.
   * Runs fixes in dependency order (panel ID repairs before order repairs).
   * Re-lints after applying; returns fixes applied count and updated report.
   */
  fix(doc: import("../mutations").FFScriptDoc): { fixesApplied: number; report: LintReport };

  /**
   * Apply a single fix descriptor to the doc (for "Apply this fix" button in the UI).
   */
  applyFix(doc: import("../mutations").FFScriptDoc, fix: LintFix): void;

  /** Retrieve the full list of enabled rules (useful for tooling / --help). */
  get rules(): LintRule[];
}
```

---

## Default rule catalogue

### `ff/structure/duplicate-panel-id`

```
severity: error | fixable: true
```

**Check:** `panels.map(p => p.id)` — any duplicate triggers one error per collision.

**Fix:** regenerate UUID on the duplicate panel (and cascade to any references in scenes,
bubbles, and FlowGraph edges).

**Acceptance criteria:**
- A doc with two panels sharing the same UUID produces exactly one error per duplicate.
- Running `--fix` resolves it and the doc validates cleanly.

---

### `ff/structure/panel-order-gap`

```
severity: error | fixable: true
```

**Check:** panel order values must form a contiguous 0-indexed sequence. `[0, 1, 3]` is invalid.

**Fix:** reassign order values to `[0, 1, 2, ...]` preserving relative sort.

---

### `ff/speaker/orphaned-speaker` (T-035)

```
severity: error | fixable: false
```

**Check:** for every `bubble.lineId`, resolve the FlowGraph node's `speakerId`. If the
`speakerId` is not null and is not present in `characters[]`, emit an error.

**Message:** `"Bubble in panel '{panelTitle}' references speaker '{speakerId}' who is not in the character list."`

**Acceptance criteria:**
- Deleting a character without cleaning up their bubbles produces one error per orphaned bubble.
- A null speakerId (narrator) does not trigger this rule.

---

### `ff/background/missing-background` (T-036)

```
severity: warning | fixable: false
```

**Check:** any panel where `panel.background === null` or `panel.background.assetKey` is empty.

**Exceptions:** panels tagged `locked: false` AND in placeholder builds (detected by a
`buildType: "placeholder"` flag on the doc root — add this field) emit `info` not `warning`.

**Acceptance criteria:**
- A panel with no background produces a warning.
- A panel with `background.assetKey` set but the asset row has `status: "pending"` also
  produces a warning (asset not ready).

---

### `ff/placement/off-canvas-character` (T-037)

```
severity: warning | fixable: false
```

**Check:** for each `CharacterPlacement`, `x` must be in `[-0.2, 1.2]` and `y` must be
in `[-0.1, 1.1]`. Outside these bounds the character is fully invisible.

**Message:** `"Character '{name}' in panel '{panelTitle}' is placed outside the canvas bounds (x={x}, y={y})."`

---

### `ff/flow/dangling-beat` (T-038)

```
severity: warning | fixable: false
```

**Check:** for every `flowGraph.node` of type `dialogue` or `choice`, verify that at least one
`panel.dialogueNodeIds` array references it. Exempt node types: `start`, `end`, `condition`,
`macro`, `stage_op`.

**Message:** `"DialogueNode '{nodeId}' is not referenced by any panel."`

---

### `ff/bubble/tail-off-target` (T-039)

```
severity: info | fixable: false
```

**Check:** for each non-caption bubble with a non-null `(tailX, tailY)`, find the speaker's
`CharacterPlacement` in the panel. Compute the placement's bounding box (approx. as
`[x, x + scale*0.25] × [y - scale, y]`). If `(tailX, tailY)` does not intersect this box,
emit an info finding.

**Message:** `"Bubble tail in panel '{panelTitle}' may not point at the speaker's face."`

**Note:** This is `info` severity; tail positioning is imprecise and the rule should not block
anything. It serves as an authoring hint only.

---

### `ff/flow/unreachable-node`

```
severity: warning | fixable: false
```

**Check:** breadth-first traversal from the `start` node. Any node not visited is unreachable.
Excludes nodes that are `macro` type (macros may be called indirectly).

**Acceptance criteria (T-059 symbolic executor):**
- Every panel reachable from the start is visited at least once.
- The publish gate (T-117) fails if any `choice` option has a `targetPanelId` that is unreachable.

---

### `ff/flow/infinite-loop`

```
severity: error | fixable: false
```

**Check:** DFS with a cycle-detection stack. If a cycle is found that does not pass through
a `condition` node (which could break the loop), emit an error.

**Message:** `"Infinite loop detected: {nodeId1} → {nodeId2} → ... → {nodeId1}"`

---

### `ff/flow/undeclared-variable`

```
severity: error | fixable: false
```

**Check:** every `condition` node's expression is parsed for variable references.
Any variable not declared in `flowGraph.variables[]` is an error.

---

### `ff/content/panel-content-rating`

```
severity: error | fixable: false
```

**Check:** `panel.contentRating === "r18"` but `game.contentRating !== "r18"`. A panel
cannot carry a higher-tier rating than its parent game.

**Message:** `"Panel '{panelTitle}' is rated r18 but the game's content rating is '{gameRating}'."`

---

### `ff/bubble/reading-order-gap`

```
severity: error | fixable: true
```

**Check:** bubble `readingOrder` values within a panel must form a contiguous 1-indexed sequence.

**Fix:** reassign reading orders in current sort order.

---

## Integration points

### Wizard health bar (T-047)

```typescript
// In updateDraft Convex mutation:
const engine = new LintEngine();
const report = engine.lint(doc);

// Surface in gameDrafts row:
await ctx.db.patch(draftId, {
  lintReport: {
    errorCount:   report.errorCount,
    warningCount: report.warningCount,
    // Store first 20 findings for display:
    findings:     report.findings.slice(0, 20).map(f => ({
      ruleId:   f.ruleId,
      severity: f.severity,
      message:  f.message,
      panelId:  f.location.panelId ?? null,
    })),
  },
});
```

### Publish gate (T-048)

```typescript
// In publishDraft Convex action, before setting status = "published":
const engine = new LintEngine({ errorsOnly: true });
const report = engine.lint(doc);
if (report.hasErrors) {
  throw new ConvexError({
    code: "LINT_ERRORS_BLOCK_PUBLISH",
    findings: report.findings,
  });
}
```

### CLI (T-049)

```bash
# Human-readable:
pnpm ffscript:lint <game-id>

# JSON output (for CI):
pnpm ffscript:lint <game-id> --format json

# Autofix:
pnpm ffscript:lint <game-id> --fix
```

Implementation: `scripts/ffscript/lint.ts` — fetches FFScript blob from R2, runs LintEngine,
prints results, exits with code 1 if `report.hasErrors`.

---

## Autofix design (T-050)

Only rules marked `fixable: true` support autofix. The engine runs them in this order:

1. `ff/structure/duplicate-panel-id` (UUID repairs must run before anything referencing panel IDs)
2. `ff/structure/panel-order-gap` (order repairs must run before anything relying on order)
3. `ff/bubble/reading-order-gap`

After applying fixes, the engine re-lints and returns the new report. If any fixable rule
still fires after fixing (indicating a fix loop), the engine emits a `warning` and stops.

---

## Acceptance criteria (T-046 — corrected effort: L)

- `new LintEngine().lint(cleanDoc).hasErrors` returns `false` for a well-formed v0.2 document.
- A doc with an orphaned speaker produces exactly one `ff/speaker/orphaned-speaker` error per
  orphaned bubble.
- A doc with a cycle through two dialogue nodes (no condition) produces one
  `ff/flow/infinite-loop` error.
- `engine.lint(doc)` is a pure function: calling it twice on the same doc produces identical
  reports.
- `engine.fix(ffScriptDoc)` on a doc with two panels sharing a UUID returns
  `fixesApplied: 1` and a follow-up report with `errorCount: 0` for that rule.
- `publishDraft` with `hasErrors: true` returns a typed Convex error containing the findings.
- `pnpm ffscript:lint <id>` exits with code 0 for a clean doc; code 1 for an errored doc.

---

*Spec produced 2026-04-23. Addresses T-035–T-039, T-046–T-050, T-059 (partial).*
