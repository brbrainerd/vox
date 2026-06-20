import React from 'react';
import { ReactFlow, Background, type Node, type Edge } from '@xyflow/react';
import type { SubAgentNode } from './types';

export function toFlow(tree: SubAgentNode[]): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  const walk = (list: SubAgentNode[]) => {
    for (const n of list) {
      nodes.push({ id: n.windowId, position: { x: n.depth * 200, y: nodes.length * 70 }, data: { label: `${n.title}${n.skill ? ` · ${n.skill}` : ''}` } });
      if (n.parentWindowId) edges.push({ id: `${n.parentWindowId}-${n.windowId}`, source: n.parentWindowId, target: n.windowId });
      walk(n.children);
    }
  };
  walk(tree);
  return { nodes, edges };
}

export function SubAgentGraph({ tree }: { tree: SubAgentNode[] }) {
  const { nodes, edges } = toFlow(tree);
  return (
    <div style={{ height: '100%', minHeight: 240 }}>
      <ReactFlow nodes={nodes} edges={edges} fitView><Background /></ReactFlow>
    </div>
  );
}
