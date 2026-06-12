/**
 * Shared search state for CommandPalette and SearchView.
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

/** Map user-facing scope chips to backend scope strings. */
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
      return ['settings'];
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
}

export type SearchAction =
  | { type: 'setQuery'; query: string }
  | { type: 'setScopes'; scopes: UserScope[] }
  | { type: 'setHits'; hits: unknown[]; token: number }
  | { type: 'setLoading'; loading: boolean; token: number };

export function searchReducer(
  state: SearchControllerState,
  action: SearchAction,
): SearchControllerState {
  switch (action.type) {
    case 'setQuery':
      return { ...state, query: action.query, requestToken: state.requestToken + 1 };
    case 'setScopes':
      return { ...state, scopes: action.scopes };
    case 'setHits':
      if (action.token !== state.requestToken) return state;
      return { ...state, hits: action.hits, loading: false };
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

export interface CommandCatalogHitInput {
  command: string;
  about: string;
}

/** Client-side command catalog matches (backend has no commands corpus yet). */
export function filterCommandCatalogHits(
  entries: CommandCatalogHitInput[],
  query: string,
): Array<{
  source: string;
  kind: string;
  path: string;
  title: string;
  snippet: string;
  score: number;
  provenance: string[];
  locator: { kind: string; value: string };
}> {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return entries
    .filter(
      (e) =>
        e.command.toLowerCase().includes(q) ||
        e.about.toLowerCase().includes(q),
    )
    .slice(0, 30)
    .map((e) => ({
      source: 'commands',
      kind: 'command',
      path: e.command,
      title: e.command,
      snippet: e.about,
      score: 0.85,
      provenance: ['commands:catalog'],
      locator: { kind: 'command', value: e.command },
    }));
}
