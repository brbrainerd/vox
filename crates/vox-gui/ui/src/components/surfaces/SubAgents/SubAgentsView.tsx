import React, { useEffect } from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useSubAgentStore } from './subAgentStore';
import { fetchTree, listenActivity } from './subAgentClient';
import { SubAgentTree } from './SubAgentTree';
import { SubAgentContextEditor } from './SubAgentContextEditor';
import { SubAgentControls } from './SubAgentControls';
import { SubAgentActivityStream } from './SubAgentActivityStream';
import type { SubAgentNode } from './types';

/** Find a node by windowId anywhere in the tree (ignores expand state). */
function findNode(nodes: SubAgentNode[], id: string): SubAgentNode | null {
  for (const n of nodes) {
    if (n.windowId === id) return n;
    const found = findNode(n.children, id);
    if (found) return found;
  }
  return null;
}

type S = ReturnType<typeof useSubAgentStore.getState>;
const selTree = (s: S) => s.tree;
const selSelected = (s: S) => s.selectedWindowId;
const selSetTree = (s: S) => s.setTree;
const selPushEvent = (s: S) => s.pushEvent;

export function SubAgentsView(_props: SurfaceDecoratorProps) {
  const tree = useSubAgentStore(selTree);
  const selected = useSubAgentStore(selSelected);
  const setTree = useSubAgentStore(selSetTree);
  const pushEvent = useSubAgentStore(selPushEvent);

  useEffect(() => { let live = true; fetchTree().then((t) => { if (live) setTree(t); }).catch(() => {}); return () => { live = false; }; }, [setTree]);
  useEffect(() => {
    let un: (() => void) | undefined;
    listenActivity((e) => {
      // Backend does not yet stamp window_id on agent events (audit correction #1):
      // route by window_id when present, else attribute to the selected window.
      const w = (e.kind as { window_id?: string }).window_id
        ?? useSubAgentStore.getState().selectedWindowId;
      if (w) pushEvent(w, e);
    }).then((u) => { un = u; }).catch(() => {});
    return () => un?.();
  }, [pushEvent]);

  const node = selected ? findNode(tree, selected) : null;

  return (
    <div style={{ display: 'flex', gap: 8, height: '100%' }}>
      <div style={{ flex: 1.1, overflow: 'auto' }}><SubAgentTree /></div>
      <div style={{ flex: 1.4, overflow: 'auto' }}>
        {node ? (
          <>
            <SubAgentControls windowId={node.windowId} status={node.status} />
            <SubAgentContextEditor windowId={node.windowId} maxTokens={node.model.maxTokens} />
            <SubAgentActivityStream windowId={node.windowId} />
          </>
        ) : <p>Select a sub-agent</p>}
      </div>
    </div>
  );
}
