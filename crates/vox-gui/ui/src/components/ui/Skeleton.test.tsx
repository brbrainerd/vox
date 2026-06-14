// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { Skeleton } from './Skeleton';

describe('Skeleton', () => {
  it('renders with aria-hidden="true" so it is invisible to screen readers', () => {
    const { container } = render(<Skeleton />);
    const el = container.firstChild as HTMLElement;
    expect(el.getAttribute('aria-hidden')).toBe('true');
  });

  it('has a data-slot="skeleton" attribute for test selection', () => {
    const { container } = render(<Skeleton />);
    const el = container.firstChild as HTMLElement;
    expect(el.getAttribute('data-slot')).toBe('skeleton');
  });

  it('forwards className', () => {
    const { container } = render(<Skeleton className="my-class" />);
    expect((container.firstChild as HTMLElement).className).toContain('my-class');
  });

  it('applies inline height style when height prop is provided as number', () => {
    const { container } = render(<Skeleton height={48} />);
    expect((container.firstChild as HTMLElement).style.height).toBe('48px');
  });

  it('applies inline width style when width prop is provided as number', () => {
    const { container } = render(<Skeleton width={200} />);
    expect((container.firstChild as HTMLElement).style.width).toBe('200px');
  });

  it('applies inline height/width as strings when passed as strings', () => {
    const { container } = render(<Skeleton height="2rem" width="100%" />);
    const el = container.firstChild as HTMLElement;
    expect(el.style.height).toBe('2rem');
    expect(el.style.width).toBe('100%');
  });

  it('renders as a <div> by default', () => {
    const { container } = render(<Skeleton />);
    expect((container.firstChild as HTMLElement).tagName.toLowerCase()).toBe('div');
  });
});
