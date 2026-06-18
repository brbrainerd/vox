// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Kpi } from './Kpi';

describe('Kpi Component', () => {
  it('renders the label, value, and delta indicators', () => {
    render(<Kpi label="Mesh node count" value={5} delta={1} trend="up" />);
    expect(screen.getByText('Mesh node count')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();
    expect(screen.getByText('▲1')).toBeInTheDocument();
  });
});
