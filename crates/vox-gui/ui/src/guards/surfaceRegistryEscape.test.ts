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

  it('DocReader is the only registry-exempt special tab render in App.tsx', () => {
    const app = readFileSync(join(SRC_ROOT, 'App.tsx'), 'utf8');
    const specialRenders = [...app.matchAll(/isDocTab\([^)]*\)\s*\?\s*<(\w+)/g)].map(
      (m) => m[1],
    );
    // doc:* tabs are keyed by document path, not a surface viewKey — a new
    // special tab TYPE must either register a surface or extend this guard
    // with its own justification.
    expect(specialRenders).toEqual(['DocReader']);
  });

  it('allowlist entries stay minimal and still routed (no stale entries)', () => {
    const cases = new Set(childRendererCaseKeys());
    const stale = [...ALLOWLIST].filter((k) => !cases.has(k));
    expect(stale, `Allowlisted keys no longer routed — delete them: ${stale}`).toEqual([]);
  });
});
