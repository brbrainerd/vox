// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { VersionMismatchBanner } from './VersionMismatchBanner';

describe('VersionMismatchBanner', () => {
  it('renders nothing when there is no mismatch', () => {
    const { container } = render(<VersionMismatchBanner mismatch={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows both versions when mismatched', () => {
    render(<VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />);
    expect(screen.getByTestId('version-mismatch-banner')).toHaveTextContent('0.5.9');
    expect(screen.getByTestId('version-mismatch-banner')).toHaveTextContent('0.6.0');
  });

  it('dismisses on click and stays dismissed', () => {
    const { rerender } = render(
      <VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />
    );
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(screen.queryByTestId('version-mismatch-banner')).not.toBeInTheDocument();

    // Re-rendering with the same mismatch keeps it dismissed.
    rerender(<VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />);
    expect(screen.queryByTestId('version-mismatch-banner')).not.toBeInTheDocument();
  });

  it('reappears when a new, different mismatch arrives after dismissal', () => {
    const { rerender } = render(
      <VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />
    );
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(screen.queryByTestId('version-mismatch-banner')).not.toBeInTheDocument();

    // Daemon restarts on a different version — a new mismatch — banner should reappear.
    rerender(<VersionMismatchBanner mismatch={{ daemon: '0.5.8', gui: '0.6.0' }} />);
    expect(screen.getByTestId('version-mismatch-banner')).toHaveTextContent('0.5.8');
  });

  it('keeps the dismiss button outside the role="alert" live region (WCAG: alert regions must stay passive)', () => {
    render(<VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />);
    const alertRegion = screen.getByRole('alert');
    const button = screen.getByRole('button', { name: /dismiss/i });
    expect(alertRegion.contains(button)).toBe(false);
    // The button remains clickable and functional.
    fireEvent.click(button);
    expect(screen.queryByTestId('version-mismatch-banner')).not.toBeInTheDocument();
  });
});
