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
    render(<VersionMismatchBanner mismatch={{ daemon: '0.5.9', gui: '0.6.0' }} />);
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(screen.queryByTestId('version-mismatch-banner')).not.toBeInTheDocument();
  });
});
