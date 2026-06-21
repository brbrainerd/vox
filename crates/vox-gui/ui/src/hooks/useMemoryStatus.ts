import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

export interface UseMemoryStatus {
  /** search_documents (embedding/vector corpus) count, or null when unavailable. */
  vectorCount: number | null;
  loading: boolean;
  error: string | null;
}

export function useMemoryStatus(): UseMemoryStatus {
  const [state, setState] = useState<UseMemoryStatus>({ vectorCount: null, loading: true, error: null });
  useEffect(() => {
    let live = true;
    voxTransport.getMemoryStatus().then(
      // 'proj' = search_documents corpus (the vector/embedding index) per commands/memory.rs::get_memory_status
      (s) => { if (live) setState({ vectorCount: s.corpus_counts?.proj ?? 0, loading: false, error: null }); },
      (e) => { if (live) setState({ vectorCount: null, loading: false, error: String(e instanceof Error ? e.message : e) }); },
    );
    return () => { live = false; };
  }, []);
  return state;
}
