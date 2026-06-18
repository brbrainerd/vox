// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  dockShellKeybindingForEvent,
  handleDockShellKeydown,
  isDockShellFocused,
  type DockShellKeyboardContext,
} from './DockShell';

function keyEvent(
  init: Partial<KeyboardEvent> & Pick<KeyboardEvent, 'key'>,
): KeyboardEvent {
  return {
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    defaultPrevented: false,
    ...init,
  } as KeyboardEvent;
}

function mockApi(overrides: Partial<DockShellKeyboardContext['api']> = {}) {
  const close = vi.fn();
  const activePanel = {
    id: 'main-surface',
    api: { close },
  };
  const panels = [activePanel];
  return {
    activePanel,
    panels,
    getPanel: vi.fn((id: string) => (id === 'main-surface' ? activePanel : undefined)),
    addPanel: vi.fn(() => ({ id: 'main-surface-split-1' })),
    ...overrides,
  } as NonNullable<DockShellKeyboardContext['api']>;
}

describe('dockShellKeybindingForEvent', () => {
  it('matches Meta+\\ as split-horizontal', () => {
    expect(
      dockShellKeybindingForEvent(keyEvent({ key: '\\', metaKey: true })),
    ).toBe('split-horizontal');
  });

  it('matches Ctrl+\\ as split-horizontal', () => {
    expect(
      dockShellKeybindingForEvent(keyEvent({ key: '\\', ctrlKey: true })),
    ).toBe('split-horizontal');
  });

  it('matches Meta+W as close-panel', () => {
    expect(
      dockShellKeybindingForEvent(keyEvent({ key: 'w', metaKey: true })),
    ).toBe('close-panel');
  });

  it('matches Ctrl+W as close-panel', () => {
    expect(
      dockShellKeybindingForEvent(keyEvent({ key: 'W', ctrlKey: true })),
    ).toBe('close-panel');
  });

  it('ignores unmodified keys', () => {
    expect(dockShellKeybindingForEvent(keyEvent({ key: '\\' }))).toBeNull();
    expect(dockShellKeybindingForEvent(keyEvent({ key: 'w' }))).toBeNull();
  });
});

describe('isDockShellFocused', () => {
  it('returns true when focus is inside the dock shell container', () => {
    const container = document.createElement('div');
    const child = document.createElement('button');
    container.appendChild(child);
    document.body.appendChild(container);
    child.focus();
    expect(isDockShellFocused(container)).toBe(true);
    document.body.removeChild(container);
  });

  it('returns false when focus is outside the dock shell container', () => {
    const container = document.createElement('div');
    const outside = document.createElement('input');
    document.body.appendChild(container);
    document.body.appendChild(outside);
    outside.focus();
    expect(isDockShellFocused(container)).toBe(false);
    document.body.removeChild(container);
    document.body.removeChild(outside);
  });
});

describe('handleDockShellKeydown', () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement('div');
    const child = document.createElement('button');
    child.type = 'button';
    container.appendChild(child);
    document.body.appendChild(container);
    child.focus();
  });

  it('splits the active panel horizontally on Meta+\\', () => {
    const api = mockApi();
    const event = keyEvent({ key: '\\', metaKey: true });

    const handled = handleDockShellKeydown(event, {
      api,
      container,
      panelId: 'main-surface',
      panelTitle: 'Dashboard',
      content: null,
    });

    expect(handled).toBe(true);
    expect(api.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({
        component: 'panel',
        position: expect.objectContaining({
          referencePanel: api.activePanel,
          direction: 'right',
        }),
      }),
    );
  });

  it('closes the active panel on Meta+W when more than one panel exists', () => {
    const close = vi.fn();
    const panelA = { id: 'a', api: { close } };
    const panelB = { id: 'b', api: { close: vi.fn() } };
    const api = mockApi({
      activePanel: panelA,
      panels: [panelA, panelB],
    });
    const event = keyEvent({ key: 'w', metaKey: true });

    const handled = handleDockShellKeydown(event, {
      api,
      container,
      panelId: 'a',
      panelTitle: 'Dashboard',
      content: null,
    });

    expect(handled).toBe(true);
    expect(close).toHaveBeenCalledTimes(1);
  });

  it('does not close on Meta+W when only one panel remains', () => {
    const close = vi.fn();
    const panel = { id: 'main-surface', api: { close } };
    const api = mockApi({
      activePanel: panel,
      panels: [panel],
    });
    const event = keyEvent({ key: 'w', metaKey: true });

    const handled = handleDockShellKeydown(event, {
      api,
      container,
      panelId: 'main-surface',
      panelTitle: 'Dashboard',
      content: null,
    });

    expect(handled).toBe(true);
    expect(close).not.toHaveBeenCalled();
  });

  it('ignores shortcuts when the dock shell is not focused', () => {
    const api = mockApi();
    const outside = document.createElement('input');
    document.body.appendChild(outside);
    outside.focus();

    const handled = handleDockShellKeydown(
      keyEvent({ key: '\\', metaKey: true }),
      {
        api,
        container,
        panelId: 'main-surface',
        panelTitle: 'Dashboard',
        content: null,
      },
    );

    expect(handled).toBe(false);
    expect(api.addPanel).not.toHaveBeenCalled();
    document.body.removeChild(outside);
  });
});
