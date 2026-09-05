/**
 * Shell persistence key SSOT — mirrors contracts/gui/shell-persistence.v1.yaml
 */

export const SHELL_PREFERENCE_KEYS = {
  sidebarMode: 'vox_sidebar_mode',
  parentTabs: 'vox_parent_tabs',
  dockLayout: 'gui.layout.v1',
  dashboardLayout: 'gui.dashboard.layout.v1',
  hudTiles: 'gui.hud.tiles.v1',
  chatDocked: 'gui.shell.chat_docked',
  theme: 'gui.theme',
  telemetry: 'gui.telemetry',
  sign: 'gui.sign',
  checkpointMins: 'gui.checkpointMins',
  memoryAutoRecall: 'gui.memory.autoRecall',
  chatModelOverride: 'vox_chat_model.v1',
} as const;

export type ShellPreferenceKey =
  (typeof SHELL_PREFERENCE_KEYS)[keyof typeof SHELL_PREFERENCE_KEYS];

export const SPARK_SERIES_PREFIX = 'vox.spark.kpi.';

export function sparkSeriesKey(metric: string): string {
  return `${SPARK_SERIES_PREFIX}${metric}`;
}
