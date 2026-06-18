import { describe, expect, it } from 'vitest';
import { viewKeyForLocator } from './locatorNavigation';

describe('viewKeyForLocator', () => {
  it('maps file locator to repository', () => {
    expect(viewKeyForLocator({ kind: 'file', value: 'src/foo.rs' })).toBe('repository');
  });

  it('maps web locator to browser', () => {
    expect(viewKeyForLocator({ kind: 'web', value: 'https://example.com' })).toBe('browser');
  });

  it('maps command locator to catalog', () => {
    expect(viewKeyForLocator({ kind: 'command', value: 'vox ci check' })).toBe('catalog');
  });

  it('maps chat locator to chat', () => {
    expect(viewKeyForLocator({ kind: 'chat', value: 'session-1' })).toBe('chat');
  });

  it('maps setting locator to settings', () => {
    expect(viewKeyForLocator({ kind: 'setting', value: '{"section":"llm"}' })).toBe('settings');
  });

  it('falls back to search for unknown kinds', () => {
    expect(viewKeyForLocator({ kind: 'none', value: '' })).toBe('search');
  });
});
