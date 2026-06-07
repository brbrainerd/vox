// Pure helpers for translating Loquela context attachments into the
// orchestrator task's file manifest. Kept framework-free so the mapping logic
// is unit-testable without a DOM/Tauri runtime.

/** A context attachment as Loquela emits it on submit. */
export interface ContextRef {
  kind: string;
  ref: string;
}

/** A chip-shaped context item ready to pin into the shared Loquela set. */
export interface AttachItem {
  kind: 'file' | 'url' | 'image';
  label: string;
}

/** Minimal shape of a search/recall hit needed to derive an attachment. */
export interface AttachableHit {
  locator?: { kind?: string; value?: string };
  path?: string | null;
  source?: string;
}

/**
 * Convert recall/search hits into pinnable context items. Only hits with a
 * concrete `file` or `web` locator are attachable — `memory`/`none` locators
 * have no file the orchestrator can read, so they are dropped (an honest
 * filter rather than pretending everything pins). `web` maps to a `url` chip.
 */
export function attachItemsFromHits(hits: AttachableHit[]): AttachItem[] {
  const out: AttachItem[] = [];
  for (const h of hits) {
    const lk = h.locator?.kind;
    const value = h.locator?.value || h.path || h.source;
    if (!value) continue;
    if (lk === 'file') out.push({ kind: 'file', label: value });
    else if (lk === 'web') out.push({ kind: 'url', label: value });
    // 'memory' / 'none' locators are intentionally skipped.
  }
  return out;
}

/** Chip kinds whose `ref` is a concrete locator the backend can pin as a file. */
export const ATTACHABLE_KINDS = ['file', 'image', 'url'] as const;

/**
 * Resolve the file manifest for a Loquela submit payload.
 *
 * Precedence: an explicit, already-resolved `files: string[]` wins. Otherwise
 * file/image/url context chips contribute their `ref` as the manifest. Skill /
 * agent / branch chips are ignored here — they are not file affinities.
 */
export function contextRefsFromPayload(payload: {
  files?: unknown;
  context?: unknown;
}): string[] {
  if (Array.isArray(payload.files) && payload.files.length > 0) {
    return payload.files.map((f) => String(f)).filter(Boolean);
  }
  const ctx = Array.isArray(payload.context) ? (payload.context as ContextRef[]) : [];
  return ctx
    .filter((c) => c && ATTACHABLE_KINDS.includes(c.kind as (typeof ATTACHABLE_KINDS)[number]))
    .map((c) => String(c.ref))
    .filter(Boolean);
}
