// Theme accent palette switching.
//
// The accent color (`brass`) is a CSS variable (`--brass`, a space-separated RGB
// triple) whose value is selected by the `data-theme` attribute on the root
// <html> element. See index.css for the per-theme definitions and
// tailwind.config.js where `brass` is wired to `rgb(var(--brass) / <alpha-value>)`.
//
// `arcane` is the default look and must stay byte-identical to the historical
// fixed gold (#d4af37).

export type ThemeId = 'arcane' | 'void' | 'glacier';

const KNOWN: ReadonlySet<string> = new Set(['arcane', 'void', 'glacier']);

/** Normalize an arbitrary preference value to a known theme id, defaulting to 'arcane'. */
export function normalizeTheme(theme: string | null | undefined): ThemeId {
  return theme && KNOWN.has(theme) ? (theme as ThemeId) : 'arcane';
}

/**
 * Apply the accent palette for `theme` by setting `data-theme` on <html>.
 * Unknown/empty values fall back to 'arcane'. Safe to call outside a DOM
 * (degrades to a no-op).
 */
export function applyTheme(theme: string | null | undefined): ThemeId {
  const id = normalizeTheme(theme);
  if (typeof document !== 'undefined' && document.documentElement) {
    document.documentElement.dataset.theme = id;
  }
  return id;
}
