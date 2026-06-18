import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

const DEFAULT_TITLE = 'Operator';

export function useWorkspaceIdentity(): { workspaceTitle: string } {
  const [workspaceTitle, setWorkspaceTitle] = useState(DEFAULT_TITLE);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const summary = await voxTransport.getIdentitySummary();
        if (cancelled) return;
        const name = summary.display_name?.trim();
        setWorkspaceTitle(name && name.length > 0 ? name : DEFAULT_TITLE);
      } catch {
        if (!cancelled) setWorkspaceTitle(DEFAULT_TITLE);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  return { workspaceTitle };
}
