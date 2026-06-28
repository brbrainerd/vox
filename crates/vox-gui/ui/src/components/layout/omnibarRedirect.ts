/**
 * Migration-ledger redirect: the dedicated Search surface is retired (VG-2).
 * Any deep link to `#view=search` opens the Omnibar instead of dead-ending,
 * and parks navigation on a real child surface.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §3.2.
 */
export interface RedirectDeps {
  openOmnibar: () => void;
  navigateTo: (viewKey: string) => void;
  fallbackChild: string;
}

export function redirectSearchViewToOmnibar(viewKey: string, deps: RedirectDeps): boolean {
  if (viewKey !== 'search') return false;
  deps.navigateTo(deps.fallbackChild);
  deps.openOmnibar();
  return true;
}
