/** Convert a raw [0,1] relevance score to a percentage string, clamped. */
export function scoreToPct(score: number): string {
  const clamped = Math.max(0, Math.min(1, score));
  return (clamped * 100).toFixed(2) + '%';
}

export interface OpenLocator {
  kind: 'file' | 'web' | 'memory' | 'chat' | 'command' | 'none';
  value: string;
}

export interface UnifiedHit {
  source: string;
  kind: string;
  path: string | null;
  title: string | null;
  snippet: string;
  score: number;
  provenance: string[];
  locator: OpenLocator;
}

export interface FacetCount {
  value: string;
  count: number;
}

export interface SearchResponse {
  hits: UnifiedHit[];
  facets_by_source: FacetCount[];
  facets_by_kind: FacetCount[];
  total: number;
  next_cursor: number | null;
  corpora: string[];
}

/** Group hits by their source field, preserving insertion order of first occurrence. */
export function groupBySource(hits: UnifiedHit[]): Map<string, UnifiedHit[]> {
  const map = new Map<string, UnifiedHit[]>();
  for (const hit of hits) {
    const bucket = map.get(hit.source);
    if (bucket) {
      bucket.push(hit);
    } else {
      map.set(hit.source, [hit]);
    }
  }
  return map;
}

/** Return the last path segment (basename) of a path string, or the full string if no separator. */
export function pathBasename(p: string): string {
  const last = p.split(/[/\\]/).filter(Boolean).pop();
  return last ?? p;
}

/**
 * Split `snippet` into segments where each whitespace-delimited token of `query`
 * is marked. Case-insensitive, non-overlapping. Operates on JS string characters
 * (not byte offsets).
 */
export function renderHighlights(
  snippet: string,
  query: string,
): { text: string; mark: boolean }[] {
  const tokens = query
    .split(/\s+/)
    .map(t => t.trim())
    .filter(t => t.length > 0);

  if (tokens.length === 0) {
    return [{ text: snippet, mark: false }];
  }

  // Build a regex that matches any token (escaped for special chars).
  const escaped = tokens.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const pattern = new RegExp(`(${escaped.join('|')})`, 'gi');

  const segments: { text: string; mark: boolean }[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(snippet)) !== null) {
    const start = match.index;
    const end = start + match[0].length;

    if (start > lastIndex) {
      segments.push({ text: snippet.slice(lastIndex, start), mark: false });
    }
    segments.push({ text: snippet.slice(start, end), mark: true });
    lastIndex = end;
  }

  if (lastIndex < snippet.length) {
    segments.push({ text: snippet.slice(lastIndex), mark: false });
  }

  return segments.length > 0 ? segments : [{ text: snippet, mark: false }];
}
