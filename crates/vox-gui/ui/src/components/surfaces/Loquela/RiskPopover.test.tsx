// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import { RiskPopover } from './RiskPopover';

describe('RiskPopover', () => {
  it('renders all three posture buttons when open', () => {
    render(<RiskPopover risk="moderate" onChange={() => {}} open onClose={() => {}} />);
    expect(screen.getByRole('button', { name: /high risk/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /moderate risk/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /low risk/i })).toBeTruthy();
  });

  it('emits the chosen posture', () => {
    const onChange = vi.fn();
    render(<RiskPopover risk="moderate" onChange={onChange} open onClose={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: /low risk/i }));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ risk: 'low' }));
  });

  it('renders nothing when closed', () => {
    const { container } = render(
      <RiskPopover risk="moderate" onChange={() => {}} open={false} onClose={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('calls onClose on Escape key', () => {
    const onClose = vi.fn();
    render(<RiskPopover risk="moderate" onChange={() => {}} open onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
