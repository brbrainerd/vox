import { describe, it, expect } from 'vitest';
import { SHELL_PREFERENCE_KEYS, sparkSeriesKey } from './shellPersistence';

describe('shellPersistence', () => {
  it('exposes canonical sidebar and dashboard layout keys', () => {
    expect(SHELL_PREFERENCE_KEYS.sidebarMode).toBe('vox_sidebar_mode');
    expect(SHELL_PREFERENCE_KEYS.dashboardLayout).toBe('gui.dashboard.layout.v1');
  });

  it('builds spark series keys under contract prefix', () => {
    expect(sparkSeriesKey('budget_burn')).toBe('vox.spark.kpi.budget_burn');
  });
});
