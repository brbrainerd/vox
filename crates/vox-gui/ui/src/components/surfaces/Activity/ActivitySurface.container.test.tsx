// @vitest-environment jsdom
import { render, screen, act } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import React from 'react';

// activityQuery is TYPED Promise<ActivityRowDto[]>, but at runtime it can resolve
// null (e.g. the visual-audit Tauri mock, or a backend/IPC returning null). The
// surface must not crash on that.
vi.mock('../../../transport', () => ({
  activityQuery: vi.fn().mockResolvedValue(null),
  listenActivityAppended: vi.fn().mockResolvedValue(() => {}),
}));

import { ActivitySurface } from './ActivitySurface';

/** Local error-boundary probe: makes a render-time throw observable as DOM. */
class Probe extends React.Component<{ children: React.ReactNode }, { msg: string | null }> {
  state = { msg: null as string | null };
  static getDerivedStateFromError(err: Error) {
    return { msg: err.message };
  }
  render() {
    return this.state.msg ? <div data-testid="probe-error">{this.state.msg}</div> : this.props.children;
  }
}

describe('ActivitySurface null-safety', () => {
  it('renders without crashing when activityQuery resolves null', async () => {
    const { container } = render(
      <Probe>
        <ActivitySurface pushToast={vi.fn()} />
      </Probe>,
    );
    // Flush mount effects + the awaited activityQuery() microtask chain so the
    // setRows(null) re-render (and any crash) has definitely happened before we assert.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    // Before the guard, setRows(null) makes rows.map() throw "Cannot read properties of
    // null (reading 'map')", caught by the Probe → probe-error. After the guard, the
    // surface stays mounted and shows its empty state for the null result.
    expect(screen.queryByTestId('probe-error')).toBeNull();
    // (container.textContent avoids RTL's icon-split / <option> matcher quirks)
    expect(container.textContent).toContain('No Activity Logged');
  });
});
