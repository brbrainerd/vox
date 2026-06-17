import { describe, expect, it } from 'vitest';
import {
  ALL_USER_SCOPES,
  backendScopesFromUserScopes,
  filterCommandCatalogHits,
  filterSettingsIndexHits,
  searchReducer,
  initialSearchState,
  userScopeToBackend,
} from './searchController';

describe('searchController', () => {
  it('maps code scope to repo and symbol', () => {
    expect(userScopeToBackend('code')).toEqual(['repo', 'symbol']);
  });

  it('fans out user scopes without duplicates', () => {
    const scopes = backendScopesFromUserScopes(['code', 'docs', 'chats']);
    expect(scopes).toContain('repo');
    expect(scopes).toContain('symbol');
    expect(scopes).toContain('chunk');
    expect(scopes).toContain('chats');
  });

  it('lists every user-facing scope chip', () => {
    expect(ALL_USER_SCOPES).toContain('commands');
    expect(ALL_USER_SCOPES).toContain('settings');
  });

  it('maps settings scope to no backend corpora (client-federated only)', () => {
    expect(userScopeToBackend('settings')).toEqual([]);
    expect(backendScopesFromUserScopes(['settings'])).toEqual([]);
  });

  it('filters command catalog entries by command or about text', () => {
    const hits = filterCommandCatalogHits(
      [
        { command: 'vox ci pre-push', about: 'Run local CI gates' },
        { command: 'vox doctor', about: 'Health checks' },
      ],
      'pre-push',
    );
    expect(hits).toHaveLength(1);
    expect(hits[0].path).toBe('vox ci pre-push');
    expect(hits[0].source).toBe('commands');
  });

  it('maps SETTINGS_INDEX rows to unified hits for settings scope merge', () => {
    const hits = filterSettingsIndexHits('openrouter');
    expect(hits.length).toBeGreaterThan(0);
    expect(hits.every(h => h.source === 'settings' && h.kind === 'setting')).toBe(true);
    expect(hits[0].locator).toEqual({
      kind: 'setting',
      value: expect.stringContaining('settingId'),
    });
    expect(hits.some(h => h.title === 'OpenRouter override')).toBe(true);
  });

  it('setScopes bumps requestToken to discard stale hits', () => {
    const withHits = searchReducer(
      { ...initialSearchState, requestToken: 2, hits: [{ id: 1 }] },
      { type: 'setScopes', scopes: ['memory'] },
    );
    expect(withHits.requestToken).toBe(3);
    const stale = searchReducer(withHits, { type: 'setHits', hits: [{ id: 99 }], repoTruncated: false, token: 2 });
    expect(stale.hits).toEqual([{ id: 1 }]);
  });
});
