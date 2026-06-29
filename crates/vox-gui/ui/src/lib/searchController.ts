/**
 * Shared search state for the Omnibar backend-search lane (useSearchController).
 */

export type UserScope =
  | 'code'
  | 'docs'
  | 'chats'
  | 'commands'
  | 'memory'
  | 'web'
  | 'settings';

export const USER_SCOPE_LABELS: Record<UserScope, string> = {
  code: 'Code',
  docs: 'Docs',
  chats: 'Chats',
  commands: 'Commands',
  memory: 'Memory',
  web: 'Web',
  settings: 'Settings',
};

/**
 * Map user-facing scope chips to `vox_search_query` scope strings.
 * Client-federated lanes (settings, and commands until a corpus exists) return `[]`
 * — see `contracts/gui/omnisearch-index.v1.yaml` and docs/src/reference/gui-navigation.md.
 */
export function userScopeToBackend(scope: UserScope): string[] {
  switch (scope) {
    case 'code':
      return ['repo', 'symbol'];
    case 'docs':
      return ['chunk', 'knowledge'];
    case 'chats':
      return ['chats'];
    case 'commands':
      return ['commands'];
    case 'memory':
      return ['memory'];
    case 'web':
      return ['web'];
    case 'settings':
      // Client-federated only (SETTINGS_INDEX); no search.rs corpus in v1.
      return [];
  }
}

export function backendScopesFromUserScopes(scopes: UserScope[]): string[] {
  const out = new Set<string>();
  for (const s of scopes) {
    for (const b of userScopeToBackend(s)) out.add(b);
  }
  return [...out];
}

export interface SearchControllerState {
  query: string;
  scopes: UserScope[];
  hits: unknown[];
  loading: boolean;
  requestToken: number;
  repoTruncated: boolean;
}

export type SearchAction =
  | { type: 'setQuery'; query: string }
  | { type: 'setScopes'; scopes: UserScope[] }
  | { type: 'setHits'; hits: unknown[]; repoTruncated: boolean; token: number }
  | { type: 'setLoading'; loading: boolean; token: number };

export function searchReducer(
  state: SearchControllerState,
  action: SearchAction,
): SearchControllerState {
  switch (action.type) {
    case 'setQuery':
      return { ...state, query: action.query, requestToken: state.requestToken + 1 };
    case 'setScopes':
      return { ...state, scopes: action.scopes, requestToken: state.requestToken + 1 };
    case 'setHits':
      if (action.token !== state.requestToken) return state;
      return { ...state, hits: action.hits, repoTruncated: action.repoTruncated, loading: false };
    case 'setLoading':
      if (action.token !== state.requestToken) return state;
      return { ...state, loading: action.loading };
    default:
      return state;
  }
}

export const initialSearchState: SearchControllerState = {
  query: '',
  scopes: ['code', 'docs', 'chats', 'commands', 'memory'],
  hits: [],
  loading: false,
  requestToken: 0,
  repoTruncated: false,
};

export const ALL_USER_SCOPES: UserScope[] = [
  'code',
  'docs',
  'chats',
  'commands',
  'memory',
  'web',
  'settings',
];

