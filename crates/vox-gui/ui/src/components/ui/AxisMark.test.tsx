// @vitest-environment jsdom
import { it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { AxisMark } from './AxisMark';

it('renders an accessible axis mark that inherits color', () => {
  const { getByRole } = render(<AxisMark />);
  const svg = getByRole('img', { name: /axis/i });
  expect(svg).toBeInTheDocument();
  expect(svg.querySelector('[stroke="currentColor"]')).toBeTruthy();
});
