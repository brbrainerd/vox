import React from 'react';
import type { DashboardData } from '../../types/dashboard';
import { CostWidget } from './widgets/CostWidget';
import { MeshWidget } from './widgets/MeshWidget';
import { ApprovalsWidget } from './widgets/ApprovalsWidget';
import { CoverageWidget } from './widgets/CoverageWidget';
import { AgentsStreamWidget } from './widgets/AgentsStreamWidget';

/** Data a purpose-built widget may consume. Extend as widgets grow. */
export interface PurposeBuiltProps {
  data: DashboardData;
}

export type PurposeBuiltComponent = (props: PurposeBuiltProps) => React.ReactElement;

/**
 * The shortlist that earns a purpose-built widget (spec §4.2). Keyed by the
 * surface key it represents. Everything else falls back to a mini-render.
 * `cost` maps to the spend surface concept (no `cost` viewKey exists — it is a
 * synthetic monitorable backed by useLlmSpend), so it is keyed `cost` here and
 * offered explicitly in the picker (Task 6); the other four match real viewKeys.
 */
const PURPOSE_BUILT: Record<string, PurposeBuiltComponent> = {
  agents: ({ data }) => <AgentsStreamWidget data={data} />,
  cost: () => <CostWidget />,
  mesh: ({ data }) => <MeshWidget data={data} />,
  approvals: ({ data }) => <ApprovalsWidget data={data} />,
  coverage: ({ data }) => <CoverageWidget data={data} />,
};

export const PURPOSE_BUILT_SURFACE_KEYS = new Set(Object.keys(PURPOSE_BUILT));

export type ResolvedWidget =
  | { kind: 'purpose-built'; Component: PurposeBuiltComponent }
  | { kind: 'fallback' };

/**
 * Resolve a slot's surface key to a render path. A registered surface gets its
 * purpose-built widget (overriding the fallback); ANY other key — including a
 * brand-new surface never seen before — falls back to a mini-render.
 */
export function resolveWidget(surfaceKey: string): ResolvedWidget {
  const Component = PURPOSE_BUILT[surfaceKey];
  return Component ? { kind: 'purpose-built', Component } : { kind: 'fallback' };
}
