// @vitest-environment jsdom
import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ContextWindowMeter } from './ContextWindowMeter';

describe('ContextWindowMeter', () => {
  it('renders percentage label', () => {
    render(
      <ContextWindowMeter
        usedTokens={64_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    // 64000/128000 = 50%
    expect(screen.getByText('50%')).toBeInTheDocument();
  });

  it('shows strategy name', () => {
    render(
      <ContextWindowMeter
        usedTokens={0}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="aggressive"
      />
    );
    expect(screen.getByText('aggressive')).toBeInTheDocument();
  });

  it('applies green class when under 70%', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={50_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    // The fill bar should have data-zone="safe"
    expect(container.querySelector('[data-zone="safe"]')).toBeTruthy();
  });

  it('applies amber class at 80% usage', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={102_400}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(container.querySelector('[data-zone="warn"]')).toBeTruthy();
  });

  it('applies red class above 90% usage', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={120_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(container.querySelector('[data-zone="danger"]')).toBeTruthy();
  });

  it('clamps percent to 100 when used exceeds max', () => {
    render(
      <ContextWindowMeter
        usedTokens={999_999}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('renders 0% when usedTokens is 0', () => {
    render(
      <ContextWindowMeter
        usedTokens={0}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(screen.getByText('0%')).toBeInTheDocument();
  });
});
