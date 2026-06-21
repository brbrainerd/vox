// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import React from 'react';
import { FunGauge } from './FunGauge';

describe('FunGauge', () => {
  it('renders derived metrics from KpiSummaryDto', () => {
    render(<FunGauge grindRatio={0.2} avgMultiplier={1.5} questsCompleted={3} />);
    expect(screen.getByTestId('grind-ratio')).toHaveTextContent('20%');
    expect(screen.getByTestId('avg-multiplier')).toHaveTextContent('1.5x');
    expect(screen.getByTestId('quests-completed')).toHaveTextContent('3');
  });
});
