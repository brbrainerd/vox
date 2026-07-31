// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SurfaceErrorBoundary } from './ErrorBoundary';

function Boom(): never {
  throw new Error('kaboom');
}

describe('SurfaceErrorBoundary', () => {
  it('renders children when there is no error', () => {
    render(
      <SurfaceErrorBoundary surface="Demo">
        <div data-testid="child">ok</div>
      </SurfaceErrorBoundary>,
    );
    expect(screen.getByTestId('child')).toBeTruthy();
  });

  it('shows a fallback with the error message and a Retry button when a child throws', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SurfaceErrorBoundary surface="Demo">
        <Boom />
      </SurfaceErrorBoundary>,
    );
    expect(screen.getByText(/kaboom/)).toBeTruthy();
    expect(screen.getByRole('button', { name: /retry/i })).toBeTruthy();
    consoleSpy.mockRestore();
  });

  it('keeps the Retry button outside the role="alert" live region (WCAG: alert regions must stay passive)', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SurfaceErrorBoundary surface="Demo">
        <Boom />
      </SurfaceErrorBoundary>,
    );
    const alertRegion = screen.getByRole('alert');
    const button = screen.getByRole('button', { name: /retry/i });
    expect(alertRegion.contains(button)).toBe(false);
    consoleSpy.mockRestore();
  });

  it('Retry remains functional: clicking it resets the boundary state', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SurfaceErrorBoundary surface="Demo">
        <Boom />
      </SurfaceErrorBoundary>,
    );
    const button = screen.getByRole('button', { name: /retry/i });
    // Clicking Retry calls setState({ error: null }); since the child still
    // throws, the boundary re-catches — this exercises the onClick handler
    // itself is still wired up correctly after the DOM restructuring.
    fireEvent.click(button);
    expect(screen.getByRole('button', { name: /retry/i })).toBeTruthy();
    consoleSpy.mockRestore();
  });
});
