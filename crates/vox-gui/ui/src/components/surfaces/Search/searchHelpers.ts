/** Convert a raw [0,1] relevance score to a percentage string, clamped. */
export function scoreToPct(score: number): string {
  const clamped = Math.max(0, Math.min(1, score));
  return (clamped * 100).toFixed(2) + '%';
}

export interface UnifiedHit {
  source: string;
  kind: string;
  path: string | null;
  title: string | null;
  snippet: string;
  score: number;
  provenance: string[];
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
