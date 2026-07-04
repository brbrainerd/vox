// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';

vi.mock('./ScientiaDashboard', () => ({
  ScientiaDashboard: () => <div data-testid="scientia-dashboard" />,
}));
vi.mock('./ClaimsView', () => ({
  ClaimsView: () => <div data-testid="scientia-claims" />,
}));

import { ScientiaSurface } from './ScientiaSurface';

describe('ScientiaSurface (Findings)', () => {
  it('defaults to the dashboard and exposes a Claims tab (claims MERGE)', () => {
    render(<ScientiaSurface pushToast={() => {}} />);
    expect(screen.getByTestId('scientia-dashboard')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Claims' }));
    expect(screen.getByTestId('scientia-claims')).toBeInTheDocument();
  });
});
