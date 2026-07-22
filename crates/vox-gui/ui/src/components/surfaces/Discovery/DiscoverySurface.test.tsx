// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { DISCOVERY_PRESET_SEED_KEY } from '../../../lib/navigation';

vi.mock('../Activity/ActivitySurface', () => ({
  ActivitySurface: () => <div data-testid="preset-timeline" />,
}));
vi.mock('../Scientia/DiscoveryInbox', () => ({
  DiscoveryInbox: () => <div data-testid="preset-inbox" />,
}));
vi.mock('../Scientia/DiscoveryReview', () => ({
  DiscoveryReview: () => <div data-testid="preset-review" />,
}));
vi.mock('../Scientia/ArchivePanel', () => ({
  ArchivePanel: () => <div data-testid="preset-archive" />,
}));

import { DiscoverySurface } from './DiscoverySurface';

const noopToast = () => {};

describe('DiscoverySurface', () => {
  beforeEach(() => window.localStorage.clear());

  it('defaults to the activity timeline', () => {
    render(<DiscoverySurface pushToast={noopToast} />);
    expect(screen.getByTestId('preset-timeline')).toBeInTheDocument();
  });

  it('switches presets via the tab strip', () => {
    render(<DiscoverySurface pushToast={noopToast} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Inbox' }));
    expect(screen.getByTestId('preset-inbox')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Review' }));
    expect(screen.getByTestId('preset-review')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Archive' }));
    expect(screen.getByTestId('preset-archive')).toBeInTheDocument();
  });

  it('condensed prop renders only the current preset label, not the preset tabs or nested surface', () => {
    render(<DiscoverySurface pushToast={noopToast} condensed />);
    // Reuses the same `preset` state the tab strip already tracks — just the
    // active preset's own label, no tab strip and no mounted nested surface.
    expect(screen.getByText('Timeline')).toBeInTheDocument();
    expect(screen.queryByRole('tab')).toBeNull();
    expect(screen.queryByTestId('preset-timeline')).toBeNull();
  });

  it('opens on the preset seeded by a legacy deep-link and consumes the seed', () => {
    window.localStorage.setItem(DISCOVERY_PRESET_SEED_KEY, 'review');
    render(<DiscoverySurface pushToast={noopToast} />);
    expect(screen.getByTestId('preset-review')).toBeInTheDocument();
    expect(window.localStorage.getItem(DISCOVERY_PRESET_SEED_KEY)).toBeNull();
  });
});
