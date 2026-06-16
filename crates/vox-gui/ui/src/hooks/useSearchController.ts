import { useCallback, useEffect, useReducer, useRef } from 'react';
import {
  backendScopesFromUserScopes,
  initialSearchState,
  searchReducer,
  type UserScope,
} from '../lib/searchController';
import { voxTransport } from '../transport';

const DEFAULT_DEBOUNCE_MS = 200;

export interface UseSearchControllerOptions {
  debounceMs?: number;
  enabled?: boolean;
}

export function useSearchController(options: UseSearchControllerOptions = {}) {
  const { debounceMs = DEFAULT_DEBOUNCE_MS, enabled = true } = options;
  const [state, dispatch] = useReducer(searchReducer, initialSearchState);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const setQuery = useCallback((query: string) => {
    dispatch({ type: 'setQuery', query });
  }, []);

  const setScopes = useCallback((scopes: UserScope[]) => {
    dispatch({ type: 'setScopes', scopes });
  }, []);

  const runSearch = useCallback(async (query: string, scopes: UserScope[], token: number) => {
    const q = query.trim();
    if (!q) {
      dispatch({ type: 'setHits', hits: [], token });
      return;
    }
    dispatch({ type: 'setLoading', loading: true, token });
    try {
      const backendScopes = backendScopesFromUserScopes(scopes);
      const res = await voxTransport.voxSearchQuery(q, 30, backendScopes);
      dispatch({ type: 'setHits', hits: res.hits ?? [], token });
    } catch {
      dispatch({ type: 'setHits', hits: [], token });
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    const token = state.requestToken;
    debounceRef.current = setTimeout(() => {
      runSearch(state.query, state.scopes, token);
    }, debounceMs);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [state.query, state.scopes, state.requestToken, debounceMs, enabled, runSearch]);

  return { state, setQuery, setScopes, runSearch };
}
