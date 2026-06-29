// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { buildPaletteItems, DocEntryLike, parsePaletteQuery, SurfaceEntryLike } from './paletteSources';
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
  beforeEach(() => window.localStorage.clear());

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

  it('> prefix yields commands mode with stripped query', () => {
    expect(parsePaletteQuery('> ci')).toEqual({ mode: 'commands', query: 'ci' });
  });

  it('@ prefix yields agents mode', () => {
    expect(parsePaletteQuery('@ scout')).toEqual({ mode: 'agents', query: 'scout' });
  });

  it('/ prefix yields skills mode for docs and catalog', () => {
    expect(parsePaletteQuery('/ mesh')).toEqual({ mode: 'skills', query: 'mesh' });
  });
});

describe('palette dual-language search', () => {
  const mercatusSources = {
    surfaces: [
      { viewKey: 'mercatus', navLabel: 'Mercatus', navGroup: 'operate', navIcon: null, cliGroup: null, tier: 'live_backend' } as SurfaceEntryLike,
    ],
    settings: [],
    docs: [],
  };

  beforeEach(() => window.localStorage.clear());

  it('matches the English label even when navLabel is the Latin form', () => {
    const hits = buildPaletteItems('market', mercatusSources);
    expect(hits.some(h => h.kind === 'surface' && h.viewKey === 'mercatus')).toBe(true);
  });

  it('matches the Latin label too', () => {
    const hits = buildPaletteItems('mercatus', mercatusSources);
    expect(hits.some(h => h.kind === 'surface' && h.viewKey === 'mercatus')).toBe(true);
  });
});
