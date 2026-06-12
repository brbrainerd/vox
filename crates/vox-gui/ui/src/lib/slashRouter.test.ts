import { describe, expect, it } from 'vitest';
import {
  formatSessionBudget,
  isAppSlashCommand,
  resolveInternalModeSlash,
  slashCommandBase,
} from './slashRouter';

describe('slashRouter', () => {
  it('normalizes slash tokens to base command', () => {
    expect(slashCommandBase('/plan draft')).toBe('/plan');
    expect(slashCommandBase('  /AUDIT  ')).toBe('/audit');
  });

  it('resolves internal mode slashes', () => {
    expect(resolveInternalModeSlash('/plan')).toBe('plan');
    expect(resolveInternalModeSlash('/verify run')).toBe('verify');
    expect(resolveInternalModeSlash('/act')).toBe('act');
    expect(resolveInternalModeSlash('/spawn')).toBeNull();
  });

  it('detects app-level slash commands', () => {
    expect(isAppSlashCommand('/memory')).toBe(true);
    expect(isAppSlashCommand('/rollback now')).toBe(true);
    expect(isAppSlashCommand('/plan')).toBe(false);
  });

  it('formats session budget for display', () => {
    expect(formatSessionBudget(1.234, 50)).toBe('session $1.23 / $50.00');
  });
});
