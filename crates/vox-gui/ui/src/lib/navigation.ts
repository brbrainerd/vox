/**
 * Resolve a view key to its top-level nav parent and optional child tab.
 */
export const PARENT_CHILD_MAP: Record<string, { parent: string; child?: string }> = {
  dashboard: { parent: 'agents', child: 'dashboard' },
  flow: { parent: 'agents', child: 'flow' },
  matrix: { parent: 'agents', child: 'matrix' },
  tasks: { parent: 'agents', child: 'tasks' },
  approvals: { parent: 'runs', child: 'approvals' },
  policies: { parent: 'runs', child: 'policies' },
  repository: { parent: 'workspace', child: 'repository' },
  browser: { parent: 'workspace', child: 'browser' },
  harness: { parent: 'workspace', child: 'harness' },
  console: { parent: 'workspace', child: 'console' },
  catalog: { parent: 'commands', child: 'catalog' },
  skills: { parent: 'commands', child: 'skills' },
  memory: { parent: 'search', child: 'memory' },
  research: { parent: 'knowledge', child: 'research' },
  scientia: { parent: 'knowledge', child: 'scientia' },
  'vox-search': { parent: 'knowledge', child: 'vox-search' },
  'discovery-review': { parent: 'knowledge', child: 'discovery-review' },
  claims: { parent: 'knowledge', child: 'claims' },
  publications: { parent: 'knowledge', child: 'publications' },
  models: { parent: 'compute', child: 'models' },
  mens: { parent: 'compute', child: 'mens' },
  populi: { parent: 'compute', child: 'populi' },
  oratio: { parent: 'compute', child: 'oratio' },
  mesh: { parent: 'compute', child: 'mesh' },
  coverage: { parent: 'settings', child: 'coverage' },
  gamify: { parent: 'agents', child: 'gamify' },
  'discovery-inbox': { parent: 'knowledge', child: 'discovery-inbox' },
  'archive-panel': { parent: 'knowledge', child: 'archive-panel' },
};

/** Stable default child when navigating to a top-level parent (breadcrumb / sidebar). */
export const DEFAULT_CHILD_BY_PARENT: Record<string, string> = {
  chat: 'chat',
  agents: 'dashboard',
  runs: 'approvals',
  workspace: 'console',
  commands: 'catalog',
  search: 'memory',
  knowledge: 'scientia',
  compute: 'models',
  mercatus: 'mercatus',
  settings: 'settings',
};

export const TOP_LEVEL_VIEWS = [
  'chat',
  'agents',
  'runs',
  'workspace',
  'commands',
  'search',
  'knowledge',
  'compute',
  'mercatus',
  'settings',
] as const;

export type TopLevelView = typeof TOP_LEVEL_VIEWS[number];

/** Human-readable labels for breadcrumb segments. */
export const NAV_LABELS: Record<string, string> = {
  chat: 'Chat',
  agents: 'Agents',
  runs: 'Runs & Approvals',
  workspace: 'Workspace',
  commands: 'Commands',
  search: 'Search',
  knowledge: 'Knowledge',
  compute: 'Compute',
  mercatus: 'Mercatus',
  settings: 'Settings',
  dashboard: 'Dashboard',
  flow: 'Flow',
  matrix: 'Matrix',
  tasks: 'Tasks',
  approvals: 'Approvals',
  policies: 'Policies',
  repository: 'Repository',
  browser: 'Browser',
  harness: 'Harness',
  console: 'Console',
  catalog: 'Catalog',
  skills: 'Skills',
  memory: 'Memory',
  research: 'Research',
  scientia: 'Scientia',
  'vox-search': 'Search Index',
  'discovery-review': 'Discovery Review',
  claims: 'Claims',
  publications: 'Publications',
  models: 'Models',
  mens: 'MENS',
  populi: 'Populi',
  oratio: 'Oratio',
  mesh: 'Mesh',
  coverage: 'Coverage',
  gamify: 'Gamify',
  'discovery-inbox': 'Discovery Inbox',
  'archive-panel': 'Archive',
};

export function labelForNavKey(key: string): string {
  return NAV_LABELS[key] ?? key.replace(/-/g, ' ');
}

export interface BreadcrumbSegment {
  key: string;
  label: string;
}

/** Breadcrumb trail for a resolved view key (parent › child when distinct). */
export function breadcrumbsForView(viewKey: string): BreadcrumbSegment[] {
  const { parent, child } = resolveNavigation(viewKey);
  const segments: BreadcrumbSegment[] = [
    { key: parent, label: labelForNavKey(parent) },
  ];
  if (child !== parent) {
    segments.push({ key: child, label: labelForNavKey(child) });
  }
  return segments;
}

const VIEW_HASH_PREFIX = '#view=';

/** Parse view key from location hash or ?view= query param. */
export function parseViewFromLocation(loc: Pick<Location, 'hash' | 'search'>): string | null {
  if (loc.hash.startsWith(VIEW_HASH_PREFIX)) {
    const key = decodeURIComponent(loc.hash.slice(VIEW_HASH_PREFIX.length));
    return key || null;
  }
  const params = new URLSearchParams(loc.search);
  const q = params.get('view');
  return q && q.length > 0 ? q : null;
}

export function viewToHash(viewKey: string): string {
  return `${VIEW_HASH_PREFIX}${encodeURIComponent(viewKey)}`;
}

export function syncViewToLocation(viewKey: string): void {
  const hash = viewToHash(viewKey);
  if (window.location.hash !== hash) {
    window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}${hash}`);
  }
}

export function resolveNavigation(viewKey: string): { parent: string; child: string } {
  const mapped = PARENT_CHILD_MAP[viewKey];
  if (mapped) {
    return { parent: mapped.parent, child: mapped.child ?? viewKey };
  }
  if (TOP_LEVEL_VIEWS.includes(viewKey as TopLevelView)) {
    const defaultChild = DEFAULT_CHILD_BY_PARENT[viewKey] ?? viewKey;
    return {
      parent: viewKey,
      child: defaultChild,
    };
  }
  return { parent: viewKey, child: viewKey };
}
