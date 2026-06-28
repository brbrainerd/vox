// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { resolveWidget, PURPOSE_BUILT_SURFACE_KEYS } from './dashboardWidgetRegistry';

describe('dashboardWidgetRegistry', () => {
  it('registers exactly the five purpose-built surfaces', () => {
    expect([...PURPOSE_BUILT_SURFACE_KEYS].sort()).toEqual(
      ['agents', 'approvals', 'coverage', 'cost', 'mesh'].sort(),
    );
  });

  it('returns a purpose-built descriptor for a registered surface (overrides fallback)', () => {
    const r = resolveWidget('mesh');
    expect(r.kind).toBe('purpose-built');
    if (r.kind === 'purpose-built') {
      expect(typeof r.Component).toBe('function');
    }
  });

  it('falls back for an unregistered surface', () => {
    const r = resolveWidget('repository');
    expect(r.kind).toBe('fallback');
  });

  it('falls back for a brand-new surface key never seen before (auto-expansion)', () => {
    const r = resolveWidget('totally-new-surface-xyz');
    expect(r.kind).toBe('fallback');
  });
});
