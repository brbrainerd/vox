import { describe, it, expect } from 'vitest';
import { buildPaletteItems, SurfaceEntryLike, DocEntryLike } from './paletteSources';
import { SettingEntry } from '../surfaces/Settings/settingsIndex';

const surfaces: SurfaceEntryLike[] = [
  { viewKey: 'tasks', cliGroup: null, tier: 'live_backend', navLabel: 'Tasks', navIcon: 'list', navGroup: 'operate' },
  { viewKey: null, cliGroup: 'mens', tier: 'none', navLabel: null, navIcon: null, navGroup: null },
];
const settings: SettingEntry[] = [
  { id: 'llm-max-concurrency', section: 'llm', label: 'Max parallel LLM requests', hint: 'Global ceiling', keywords: ['openrouter'] },
];
const docs: DocEntryLike[] = [{ title: 'Mesh SSOT', description: 'phases', path: 'C:/x/mesh.md' }];

describe('buildPaletteItems', () => {
  it('matches surfaces by navLabel and excludes non-navigable entries', () => {
    const items = buildPaletteItems('task', { surfaces, settings, docs });
    const surfaceHits = items.filter(i => i.kind === 'surface');
    expect(surfaceHits).toHaveLength(1);
    expect(surfaceHits[0].kind === 'surface' && surfaceHits[0].label).toBe('Tasks');
  });

  it('matches settings by keyword', () => {
    const items = buildPaletteItems('openrouter', { surfaces, settings, docs });
    expect(items.some(i => i.kind === 'setting' && i.targetSection === 'llm')).toBe(true);
  });

  it('matches docs by title and carries the path', () => {
    const items = buildPaletteItems('mesh', { surfaces, settings, docs });
    const doc = items.find(i => i.kind === 'doc');
    expect(doc && doc.kind === 'doc' && doc.path).toBe('C:/x/mesh.md');
  });

  it('empty query returns no federation items', () => {
    expect(buildPaletteItems('', { surfaces, settings, docs })).toHaveLength(0);
  });
});
