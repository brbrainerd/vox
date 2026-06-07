// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import React from 'react';

// Mock the Tauri invoke boundary: the review queue resolves empty so the panel
// renders its empty state without a real backend.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock the transport event bridge: listenScientiaQueue rejects (no Tauri), so
// the component falls back to its interval — exactly the browser-dev path.
vi.mock('../../../transport', () => ({
  listenScientiaQueue: vi.fn().mockRejectedValue(new Error('not in tauri')),
}));

import { DiscoveryReview } from './DiscoveryReview';

describe('DiscoveryReview', () => {
  beforeEach(() => cleanup());

  it('renders the Discovery Review heading', () => {
    render(<DiscoveryReview pushToast={vi.fn()} />);
    expect(screen.getByText('Discovery Review')).toBeTruthy();
  });

  it('shows the empty-state prompt before a publication id is entered', () => {
    render(<DiscoveryReview pushToast={vi.fn()} />);
    expect(
      screen.getByText('Enter a publication id to load its review queue.'),
    ).toBeTruthy();
  });
});
