// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { AxisMark } from './AxisMark';

describe('AxisMark', () => {
  it('renders an accessible gimbal SVG with the spin axis', () => {
    const { container } = render(<AxisMark className="size-6 text-brass" title="Axis" />);
    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute('viewBox')).toBe('0 0 1024 1024');
    // monochrome via currentColor (caller controls hue through text-*)
    expect(svg?.innerHTML).toMatch(/currentColor/);
    expect(container.querySelector('title')?.textContent).toBe('Axis');
  });

  it('passes the className through for sizing + color', () => {
    const { container } = render(<AxisMark className="size-4 text-brass" />);
    expect(container.querySelector('svg')?.getAttribute('class')).toContain('size-4');
  });
});
