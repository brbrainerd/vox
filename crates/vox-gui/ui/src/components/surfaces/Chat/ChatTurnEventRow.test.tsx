// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChatTurnEventRow } from './ChatTurnEventRow';
import type { TurnEventDto } from '../../../types/dashboard';

describe('ChatTurnEventRow', () => {
  it('renders a chip naming the activated skill', () => {
    render(<ChatTurnEventRow event={{ kind: 'skill_activated', skill_id: 'ponytail' }} />);
    expect(screen.getByTestId('chat-turn-event-row')).toHaveTextContent('ponytail');
  });

  it('calls onExcludeSkill with the skill id when "not this one" is clicked', () => {
    const onExcludeSkill = vi.fn();
    render(
      <ChatTurnEventRow
        event={{ kind: 'skill_activated', skill_id: 'ponytail' }}
        onExcludeSkill={onExcludeSkill}
      />,
    );
    fireEvent.click(screen.getByText('not this one'));
    expect(onExcludeSkill).toHaveBeenCalledWith('ponytail');
  });

  it('does not offer exclusion for an unresolved ("unknown") skill id', () => {
    const onExcludeSkill = vi.fn();
    render(
      <ChatTurnEventRow
        event={{ kind: 'skill_activated', skill_id: 'unknown' }}
        onExcludeSkill={onExcludeSkill}
      />,
    );
    expect(screen.queryByText('not this one')).not.toBeInTheDocument();
  });

  it('renders without throwing on an unrecognized event kind', () => {
    const unknownEvent = { kind: 'some_future_kind_the_ui_has_never_seen' } as TurnEventDto;
    expect(() => render(<ChatTurnEventRow event={unknownEvent} />)).not.toThrow();
    expect(screen.queryByTestId('chat-turn-event-row')).not.toBeInTheDocument();
  });
});
