import { describe, expect, it } from 'vitest';
import {
  ALL_USER_SCOPES,
  backendScopesFromUserScopes,
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
