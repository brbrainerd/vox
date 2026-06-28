// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WidgetErrorBoundary } from './WidgetErrorBoundary';

function Boom(): JSX.Element {
  throw new Error('kaboom in mesh');
}

describe('WidgetErrorBoundary', () => {
  // React logs the caught error to console.error; silence it for a clean run.
  let spy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => { spy = vi.spyOn(console, 'error').mockImplementation(() => {}); });
  afterEach(() => { spy.mockRestore(); });

  it('renders a compact error tile instead of crashing when a child throws', () => {
    render(
      <WidgetErrorBoundary label="Mesh">
        <Boom />
      </WidgetErrorBoundary>,
    );
    const tile = screen.getByTestId('widget-error-tile');
    expect(tile).toBeTruthy();
    expect(tile.textContent).toContain('Mesh');
    expect(tile.textContent).toContain('kaboom in mesh');
  });

  it('renders children unchanged when nothing throws', () => {
    render(
      <WidgetErrorBoundary label="OK">
        <div data-testid="ok-body">fine</div>
      </WidgetErrorBoundary>,
    );
    expect(screen.getByTestId('ok-body')).toBeTruthy();
    expect(screen.queryByTestId('widget-error-tile')).toBeNull();
  });
});
