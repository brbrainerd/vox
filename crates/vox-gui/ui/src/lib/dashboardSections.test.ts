import { describe, expect, it } from 'vitest';
import {
  DASHBOARD_SECTIONS,
  sectionForNavGroup,
  surfacesForSection,
  type SurfaceRow,
} from './dashboardSections';

describe('dashboardSections', () => {
  it('exposes the four ordered sections', () => {
    expect(DASHBOARD_SECTIONS).toEqual(['operations', 'cost', 'knowledge', 'surfaces']);
  });

  it('maps operate→operations, knowledge→knowledge, and other groups→surfaces', () => {
    expect(sectionForNavGroup('operate')).toBe('operations');
    expect(sectionForNavGroup('knowledge')).toBe('knowledge');
    expect(sectionForNavGroup('develop')).toBe('surfaces');
    expect(sectionForNavGroup('compute')).toBe('surfaces');
    expect(sectionForNavGroup('system')).toBe('surfaces');
    expect(sectionForNavGroup(null)).toBe('surfaces');
  });

  it('AUTO-EXPANSION: a brand-new registry row lands in a section with no edit', () => {
    const baseline: SurfaceRow[] = [
      { viewKey: 'mesh', navLabel: 'Mesh', navGroup: 'compute' },
    ];
    const withNew: SurfaceRow[] = [
      ...baseline,
      { viewKey: 'brand-new-surface', navLabel: 'Brand New', navGroup: 'operate' },
    ];
    const opsBefore = surfacesForSection('operations', baseline).map((r) => r.viewKey);
    const opsAfter = surfacesForSection('operations', withNew).map((r) => r.viewKey);
    expect(opsBefore).not.toContain('brand-new-surface');
    expect(opsAfter).toContain('brand-new-surface'); // auto-appeared, zero dashboard edits
  });
});
