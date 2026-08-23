import type { Page } from '@playwright/test';

export interface ReviewViewport { name: 'wide' | 'laptop' | 'compact'; width: number; height: number; }
export const VIEWPORTS: ReviewViewport[] = [
  { name: 'wide', width: 1440, height: 900 },
  { name: 'laptop', width: 1100, height: 720 },
  { name: 'compact', width: 900, height: 600 },
];

export type MockKind = 'rich' | 'empty' | 'error' | 'none';

export interface ReviewState {
  name: string;
  /** Drive the page into the state AFTER the surface has rendered. */
  setup?: (page: Page) => Promise<void>;
  /** Restrict to viewports where this state's UI exists. */
  viewports?: Array<'wide' | 'laptop' | 'compact'>;
  /** Which mock installer backs this capture (default 'rich').
   * 'empty'/'error' subsume screenshots-variants.spec.ts; 'none' captures
   * true no-backend browser mode (BackendBanner regression). */
  mock?: MockKind;
}

const DEFAULT: ReviewState = { name: 'default' };
/** Empty/error coverage inherited from screenshots-variants KEY_SURFACES. */
const VARIANT: ReviewState[] = [
  { name: 'empty', mock: 'empty' },
  { name: 'error', mock: 'error' },
];
const VARIANT_SURFACES = new Set([
  'dashboard', 'chat', 'runs', 'approvals', 'models',
  'memory', 'vox-search', 'policies', 'gamify', 'settings',
]);

/** Selector ground truth verified 2026-07-18; a failed setup records
 * state_ok:false in the entry (evidence), it does not fail the run. */
export const SURFACE_STATES: Record<string, ReviewState[]> = Object.fromEntries(
  // Every surface starts with an explicit [DEFAULT]; specifics below override.
  ([
    'activity', 'approvals', 'browser', 'catalog', 'chat', 'console',
    'coverage', 'dashboard', 'flow', 'harness', 'harness-health', 'memory', 'mercatus', 'mesh',
    'models', 'needs-you', 'policies', 'publications', 'runs', 'settings',
    'skills', 'sub-agents', 'tasks', 'vox-search', 'gamify', 'repository',
    'scientia', 'mens', 'populi', 'research', 'oratio',
  ] as string[]).map((k) => [k, [DEFAULT]]),
);

SURFACE_STATES['chat'] = [
  DEFAULT,
  {
    name: 'model-picker-open',
    // Scoped: 'model:' prefix could collide with transcript text.
    setup: async (p) => { await p.getByTestId('chat-surface-layout').getByRole('button', { name: /^model:/i }).click(); },
  },
  {
    name: 'session-menu-open',
    // Viewport-tolerant: at compact width the rail hides behind a toggle;
    // opening it ALSO captures the overlay-over-transcript occlusion case.
    setup: async (p) => {
      const toggle = p.getByTestId('chat-session-rail-toggle');
      if (await toggle.isVisible()) await toggle.click();
      await p.getByRole('button', { name: /session actions for/i }).first().click();
    },
  },
  {
    name: 'composer-filled',
    setup: async (p) => {
      await p.getByLabel('Task composer').fill(
        'A deliberately long composer draft that should wrap across multiple lines and reveal any clipping or overlap issues in the dock '.repeat(2),
      );
    },
  },
  {
    name: 'rails-overlay-open',
    viewports: ['compact'],
    setup: async (p) => { await p.getByTestId('chat-session-rail-toggle').click(); },
  },
  ...VARIANT,
];

SURFACE_STATES['tasks'] = [
  DEFAULT,
  {
    name: 'composer-filled',
    setup: async (p) => {
      await p.getByLabel('Add a task').fill(
        'Draft task with an intentionally very long title to probe truncation and row overflow behavior in the composer',
      );
    },
  },
  // NOTE: priority-select-open is intentionally omitted — native <select>
  // popups render outside the page and cannot be screenshotted.
];

SURFACE_STATES['settings'] = [
  DEFAULT,
  { name: 'search-filtered', setup: async (p) => { await p.getByLabel('Search settings').fill('key'); } },
  { name: 'section-keybinds', setup: async (p) => { await p.getByRole('button', { name: 'Keybinds' }).click(); } },
  ...VARIANT,
];

SURFACE_STATES['approvals'] = [
  DEFAULT,
  {
    name: 'row-focused',
    setup: async (p) => { for (let i = 0; i < 4; i++) await p.keyboard.press('Tab'); },
  },
  ...VARIANT,
];

SURFACE_STATES['dashboard'] = [
  DEFAULT,
  { name: 'omnibar-open', setup: async (p) => { await p.keyboard.press('Control+k'); } },
  { name: 'sidebar-collapsed', setup: async (p) => { await p.getByRole('button', { name: 'Collapse sidebar' }).click(); } },
  { name: 'achievements-open', setup: async (p) => { await p.getByRole('button', { name: 'Open achievements' }).click(); } },
  {
    name: 'hud-hidden',
    setup: async (p) => { await p.keyboard.press('Control+Shift+H'); await p.keyboard.press('Control+Shift+H'); },
  },
  {
    name: 'focus-visible',
    setup: async (p) => { for (let i = 0; i < 4; i++) await p.keyboard.press('Tab'); },
  },
  // The Phase A regression: no mock at all -> banner must render, zero raw
  // TypeErrors; the banner's own placement gets AI-reviewed.
  { name: 'no-backend', mock: 'none', viewports: ['wide'] },
  ...VARIANT,
];

for (const k of VARIANT_SURFACES) {
  if (!SURFACE_STATES[k].some((s) => s.name === 'empty')) SURFACE_STATES[k].push(...VARIANT);
}

/** Themes captured for default states at wide/chromium (Task 7). */
export const AUDIT_THEMES = ['high-contrast'] as const;

export function statesFor(viewKey: string): ReviewState[] {
  return SURFACE_STATES[viewKey] ?? [DEFAULT];
}
