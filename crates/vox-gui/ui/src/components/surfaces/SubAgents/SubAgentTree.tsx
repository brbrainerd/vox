import React from 'react';
import { ChevronRight, ChevronDown } from 'lucide-react';
import { useSubAgentStore } from './subAgentStore';
import { flattenTree, tokenFate } from './types';

// Hoisted stable selectors satisfy React 19 useSyncExternalStore getSnapshot
// stability requirement: same function reference = no false positive tearing check.
type S = ReturnType<typeof useSubAgentStore.getState>;
const sel = {
  tree: (s: S) => s.tree,
  expanded: (s: S) => s.expanded,
  selected: (s: S) => s.selectedWindowId,
  toggleExpand: (s: S) => s.toggleExpand,
  select: (s: S) => s.select,
};

export function SubAgentTree() {
  const tree = useSubAgentStore(sel.tree);
  const expanded = useSubAgentStore(sel.expanded);
  const selected = useSubAgentStore(sel.selected);
  const toggleExpand = useSubAgentStore(sel.toggleExpand);
  const select = useSubAgentStore(sel.select);
  const rows = flattenTree(tree, expanded);

  return (
    <div role="tree" aria-label="Sub-agent activity">
      {rows.map((r) => {
        const fate = tokenFate(r.node.usedTokens, r.node.model.maxTokens);
        return (
          <div role="treeitem" key={r.windowId} aria-selected={selected === r.windowId}
               style={{ paddingLeft: 8 + r.depth * 16, display: 'flex', gap: 6, alignItems: 'center' }}
               onClick={() => select(r.windowId)}>
            {r.hasChildren ? (
              <button aria-label={`expand ${r.node.title}`} onClick={(e) => { e.stopPropagation(); toggleExpand(r.windowId); }}>
                {expanded.has(r.windowId) ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              </button>
            ) : <span style={{ width: 14 }} />}
            <span>{r.node.title}</span>
            {r.node.skill && <span className="pill">{r.node.skill}</span>}
            <span style={{ opacity: 0.6 }}>{r.node.model.id}</span>
            <span data-testid={`budget-${r.windowId}`} data-fate={fate}>
              {r.node.usedTokens}/{r.node.model.maxTokens}
            </span>
            <span data-status={r.node.status} />
          </div>
        );
      })}
    </div>
  );
}
