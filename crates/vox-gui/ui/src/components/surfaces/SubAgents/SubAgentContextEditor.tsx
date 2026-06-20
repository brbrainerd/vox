import React, { useEffect, useState } from 'react';
import { X, Pin } from 'lucide-react';
import { getContext, setContext } from './subAgentClient';
import type { ProjectionItem } from './types';

export function SubAgentContextEditor({ windowId, maxTokens }: { windowId: string; maxTokens: number }) {
  const [items, setItems] = useState<ProjectionItem[]>([]);
  useEffect(() => { let live = true; getContext(windowId).then((r) => { if (live) setItems(r); }).catch(() => {}); return () => { live = false; }; }, [windowId]);

  const used = items.filter((i) => i.fate === 'included' || i.pinned).reduce((a, i) => a + i.tokenEstimate, 0);

  async function persist(next: ProjectionItem[]) {
    setItems(next);
    await setContext(windowId, next.map((i) => i.itemId));
  }
  const remove = (id: string) => persist(items.filter((i) => i.itemId !== id));
  const togglePin = (id: string) => persist(items.map((i) => i.itemId === id ? { ...i, pinned: !i.pinned } : i));

  return (
    <div aria-label={`committed set for ${windowId}`}>
      <div data-testid="budget-bar">{used}/{maxTokens} tok</div>
      <ul>
        {items.map((i) => (
          <li key={i.itemId} data-fate={i.fate} style={{ display: 'flex', gap: 6 }}>
            <button aria-label={`pin ${i.itemId}`} onClick={() => togglePin(i.itemId)}><Pin size={12} /></button>
            <span style={{ opacity: 0.6 }}>{i.role}</span>
            <span>{i.preview}</span>
            <span style={{ opacity: 0.5 }}>{i.tokenEstimate}t</span>
            <button aria-label={`remove ${i.itemId}`} onClick={() => remove(i.itemId)}><X size={12} /></button>
          </li>
        ))}
      </ul>
    </div>
  );
}
