/**
 * Subsequence fuzzy filtering logic for history entries.
 * Case-insensitive subsequence match: all characters in query must appear in text
 * in the same order, but not necessarily contiguously.
 * Matches are scored by first occurrence index (lower is better) and match span (tighter is better).
 */
export function filterEntries<T extends { text: string }>(query: string, entries: T[]): T[] {
  const q = query.trim().toLowerCase();
  if (!q) {
    return entries;
  }

  interface Scored<T> {
    entry: T;
    firstIndex: number;
    span: number;
    originalIndex: number;
  }

  const results: Scored<T>[] = [];

  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i];
    const text = entry.text.toLowerCase();
    let qIdx = 0;
    let firstIndex = -1;
    let lastIndex = -1;

    for (let tIdx = 0; tIdx < text.length; tIdx++) {
      if (text[tIdx] === q[qIdx]) {
        if (qIdx === 0) {
          firstIndex = tIdx;
        }
        lastIndex = tIdx;
        qIdx++;
        if (qIdx === q.length) {
          break;
        }
      }
    }

    if (qIdx === q.length) {
      const span = lastIndex - firstIndex + 1;
      results.push({
        entry,
        firstIndex,
        span,
        originalIndex: i,
      });
    }
  }

  return results
    .sort((a, b) => {
      if (a.firstIndex !== b.firstIndex) {
        return a.firstIndex - b.firstIndex;
      }
      if (a.span !== b.span) {
        return a.span - b.span;
      }
      return a.originalIndex - b.originalIndex;
    })
    .map(r => r.entry);
}
