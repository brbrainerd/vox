import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { surfaceDecorators } from '../components/surfaces/decoratorRegistry';

const SRC_ROOT = join(import.meta.dirname, '..');

const REGISTERED = new Set(
  SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string),
);

/**
 * Routed render paths that intentionally have NO SURFACE_REGISTRY entry.
 * Every entry needs a justification — an unexplained addition here defeats
 * the guard. Registry escapes silently skip ALL screenshot/visual coverage
 * (the sweep derives its view list from SURFACE_REGISTRY), so the default
 * answer is: register the surface, don't allowlist it.
 */
const ALLOWLIST = new Set<string>([
  // One-release deep-link alias for 'vox-search' (surfaceComponents.tsx
  // fall-through case). The canonical key 'vox-search' IS registered and
  // screenshot-covered; the alias renders the identical panel.
  'graphify',
]);

function childRendererCaseKeys(): string[] {
  const src = readFileSync(
    join(SRC_ROOT, 'components/layout/surfaceComponents.tsx'),
    'utf8',
  );
  return [...src.matchAll(/^\s*case '([^']+)':/gm)].map((m) => m[1]);
}

describe('surface registry escape guard (B8)', () => {
  it('every childRenderer case key has a SURFACE_REGISTRY entry or a justified allowlist entry', () => {
    const escaped = childRendererCaseKeys().filter(
      (k) => !REGISTERED.has(k) && !ALLOWLIST.has(k),
    );
    expect(
      escaped,
      `Routed in surfaceComponents.tsx but missing from SURFACE_REGISTRY ` +
        `(register via contracts/gui/surface-registry.v1.yaml + ` +
        `\`vox ci gui-surface-registry --write\`, or remove the route): ` +
        JSON.stringify(escaped),
    ).toEqual([]);
  });

  it('every surfaceDecorators key has a SURFACE_REGISTRY entry', () => {
    const escaped = Object.keys(surfaceDecorators).filter((k) => !REGISTERED.has(k));
    expect(escaped, `Decorator surfaces missing from SURFACE_REGISTRY: ${escaped}`).toEqual([]);
  });

  it('App.tsx has no isDocTab-gated special tab render (DocReader moved to DocViewerDrawer)', () => {
    // Historically DocReader was rendered as a `isDocTab(activeTab) ? <DocReader
    // tabId={...} /> : renderSurfaceContent(...)` special case in App.tsx's main
    // surface, exempted from the registry by name in this guard. The nav-shell
    // redesign (useActiveView/useDocViewer) removed workbench tabs entirely:
    // DocReader now renders exclusively inside DocViewerDrawer, always layered
    // on top of whatever registry-driven surface is active — it never
    // special-cases the main-surface render path at all, so there is nothing
    // left for this guard to exempt. Assert the pattern is gone rather than
    // deleting the test outright, so a regression (someone reintroducing an
    // isDocTab ternary in App.tsx) is still caught.
    const app = readFileSync(join(SRC_ROOT, 'App.tsx'), 'utf8');
    const specialRenders = [...app.matchAll(/isDocTab\([^)]*\)\s*\?\s*<(\w+)/g)].map(
      (m) => m[1],
    );
    expect(specialRenders).toEqual([]);
  });

  it('allowlist entries stay minimal and still routed (no stale entries)', () => {
    const cases = new Set(childRendererCaseKeys());
    const stale = [...ALLOWLIST].filter((k) => !cases.has(k));
    expect(stale, `Allowlisted keys no longer routed — delete them: ${stale}`).toEqual([]);
  });
});
