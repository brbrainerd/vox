import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';

export const DASHBOARD_SECTIONS = ['operations', 'cost', 'knowledge', 'surfaces'] as const;
export type DashboardSection = (typeof DASHBOARD_SECTIONS)[number];

/** Minimal shape of a registry row this module reads (test-injectable). */
export interface SurfaceRow {
  viewKey: string | null;
  navLabel: string | null;
  navGroup: string | null;
}

/**
 * Fold a surface registry navGroup into a dashboard section. Operations and
 * Knowledge map directly; everything else (develop/compute/system/null) lands
 * in the catch-all "Surfaces" section. "Cost" is synthetic (the spend
 * monitorable) and is not produced by any navGroup.
 */
export function sectionForNavGroup(navGroup: string | null): DashboardSection {
  switch (navGroup) {
    case 'operate':
      return 'operations';
    case 'knowledge':
      return 'knowledge';
    default:
      return 'surfaces';
  }
}

/** Surface rows (real viewKeys with labels) that belong to a section. */
export function surfacesForSection(
  section: DashboardSection,
  rows: SurfaceRow[] = SURFACE_REGISTRY as unknown as SurfaceRow[],
): SurfaceRow[] {
  if (section === 'cost') return []; // synthetic; offered explicitly by the picker
  return rows.filter(
    (r) => r.viewKey && r.navLabel && sectionForNavGroup(r.navGroup) === section,
  );
}
