/**
 * Resolve a view key to its top-level nav parent and optional child tab.
 * Intent-first grouping: Direct(chat) → Review(runs) → Agents → Knowledge →
 * Workspace → Commands → Compute → Settings.
 */
export const PARENT_CHILD_MAP: Record<string, { parent: string; child?: string }> = {
  // Review — approvals first: the human's review queue.
  approvals: { parent: 'runs', child: 'approvals' },
  'needs-you': { parent: 'runs', child: 'needs-you' },
  runs: { parent: 'runs', child: 'runs' },
  policies: { parent: 'runs', child: 'policies' },
  // Agents — watch/steer the swarm.
  dashboard: { parent: 'agents', child: 'dashboard' },
  flow: { parent: 'agents', child: 'flow' },
  tasks: { parent: 'agents', child: 'tasks' },
  mesh: { parent: 'agents', child: 'mesh' },
  'sub-agents': { parent: 'agents', child: 'sub-agents' },
  // Knowledge — find/recall/review what the system knows.
  memory: { parent: 'knowledge', child: 'memory' },
  scientia: { parent: 'knowledge', child: 'scientia' },
  research: { parent: 'knowledge', child: 'research' },
  activity: { parent: 'knowledge', child: 'activity' },
  publications: { parent: 'knowledge', child: 'publications' },
  'vox-search': { parent: 'knowledge', child: 'vox-search' },
  // Workspace — act on the dev environment.
  console: { parent: 'workspace', child: 'console' },
  repository: { parent: 'workspace', child: 'repository' },
  browser: { parent: 'workspace', child: 'browser' },
  harness: { parent: 'workspace', child: 'harness' },
  // Commands.
  catalog: { parent: 'commands', child: 'catalog' },
  skills: { parent: 'commands', child: 'skills' },
  // Compute.
  models: { parent: 'compute', child: 'models' },
  mens: { parent: 'compute', child: 'mens' },
  populi: { parent: 'compute', child: 'populi' },
  oratio: { parent: 'compute', child: 'oratio' },
  // Settings.
  coverage: { parent: 'settings', child: 'coverage' },
  gamify: { parent: 'settings', child: 'gamify' },
};

/**
 * Migration ledger (gui-ia-blueprint §5): retired view keys resolve to their
 * surviving absorber so old #view= deep-links and bookmarks never dead-end.
 * Silent alias for one release, then hard-remove.
 */
export const LEGACY_VIEW_ALIASES: Record<string, string> = {
  search: 'memory',
  claims: 'scientia',
  review: 'scientia',
  matrix: 'chat',
  'discovery-inbox': 'activity',
  'discovery-review': 'activity',
  'archive-panel': 'activity',
};

/** Discovery preset carried by retired discovery deep-links (read by DiscoverySurface). */
export const DISCOVERY_PRESET_BY_LEGACY_KEY: Record<string, 'inbox' | 'review' | 'archive'> = {
  'discovery-inbox': 'inbox',
  'discovery-review': 'review',
  'archive-panel': 'archive',
};

export const DISCOVERY_PRESET_SEED_KEY = 'vox_discovery_preset_seed';

/** Seed the Discovery preset when a retired discovery key is navigated to. */
export function seedDiscoveryPresetForLegacyKey(viewKey: string): void {
  const preset = DISCOVERY_PRESET_BY_LEGACY_KEY[viewKey];
  if (!preset) return;
  try {
    window.localStorage.setItem(DISCOVERY_PRESET_SEED_KEY, preset);
  } catch {
    /* localStorage unavailable — surface still switches, preset defaults */
  }
}

/** Stable default child when navigating to a top-level parent (breadcrumb / sidebar). */
export const DEFAULT_CHILD_BY_PARENT: Record<string, string> = {
  chat: 'chat',
  runs: 'approvals',
  agents: 'dashboard',
  knowledge: 'memory',
  workspace: 'console',
  commands: 'catalog',
  compute: 'models',
  mercatus: 'mercatus',
  settings: 'settings',
};

export const TOP_LEVEL_VIEWS = [
  'chat',
  'runs',
  'agents',
  'knowledge',
  'workspace',
  'commands',
  'compute',
  'mercatus',
  'settings',
] as const;

export type TopLevelView = typeof TOP_LEVEL_VIEWS[number];

/** Sub-tab display order per parent (registry rows are alphabetical; UI order is intent order). */
export const CHILD_ORDER_BY_PARENT: Record<string, string[]> = {
  runs: ['approvals', 'needs-you', 'runs', 'policies'],
  agents: ['dashboard', 'flow', 'tasks', 'mesh', 'sub-agents'],
  knowledge: ['memory', 'scientia', 'research', 'activity', 'publications', 'vox-search'],
  workspace: ['console', 'repository', 'browser', 'harness'],
  commands: ['catalog', 'skills'],
  compute: ['models', 'mens', 'populi', 'oratio'],
  settings: ['settings', 'coverage', 'gamify'],
};

/** Sort child view keys by the parent's intent order; unknown keys keep relative order at the end. */
export function orderedChildren(parent: string, children: string[]): string[] {
  const order = CHILD_ORDER_BY_PARENT[parent];
  if (!order) return children;
  const rank = new Map(order.map((k, i) => [k, i]));
  return [...children].sort(
    (a, b) => (rank.get(a) ?? order.length) - (rank.get(b) ?? order.length),
  );
}

/** Human-readable labels for breadcrumb segments. */
export const NAV_LABELS: Record<string, string> = {
  chat: 'Chat',
  runs: 'Review',
  agents: 'Agents',
  knowledge: 'Knowledge',
  workspace: 'Workspace',
  commands: 'Commands',
  compute: 'Compute',
  mercatus: 'Mercatus',
  settings: 'Settings',
  dashboard: 'Dashboard',
  flow: 'Flow',
  tasks: 'Tasks',
  approvals: 'Approvals',
  'needs-you': 'Needs You',
  policies: 'Policies',
  repository: 'Repository',
  browser: 'Browser',
  harness: 'Harness',
  console: 'Console',
  catalog: 'Catalog',
  skills: 'Skills',
  memory: 'Memory',
  research: 'Research',
  scientia: 'Findings',
  activity: 'Discovery',
  'vox-search': 'Search Index',
  publications: 'Publications',
  models: 'Models',
  mens: 'Training',
  populi: 'Nodes',
  oratio: 'Voice',
  mesh: 'Mesh',
  'sub-agents': 'Sub-Agents',
  coverage: 'Coverage',
  gamify: 'Gamify',
};

export function labelForNavKey(key: string): string {
  return NAV_LABELS[key] ?? key.replace(/-/g, ' ');
}

/** Short label for workbench tab bar chips. */
export function tabLabelFor(viewKey: string): string {
  return labelForNavKey(viewKey);
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
  const key = LEGACY_VIEW_ALIASES[viewKey] ?? viewKey;
  const mapped = PARENT_CHILD_MAP[key];
  if (mapped) {
    return { parent: mapped.parent, child: mapped.child ?? key };
  }
  if (TOP_LEVEL_VIEWS.includes(key as TopLevelView)) {
    const defaultChild = DEFAULT_CHILD_BY_PARENT[key] ?? key;
    return {
      parent: key,
      child: defaultChild,
    };
  }
  return { parent: key, child: key };
}
