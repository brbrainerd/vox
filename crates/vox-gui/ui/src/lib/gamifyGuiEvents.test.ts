import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../transport', () => ({
  voxTransport: {
    recordGuiEvent: vi.fn(),
  },
}));

import { voxTransport } from '../transport';
import {
  recordGamifyGuiEvent,
  setGamifyGuiEventResultListener,
} from './gamifyGuiEvents';

const mockRecord = voxTransport.recordGuiEvent as ReturnType<typeof vi.fn>;

describe('recordGamifyGuiEvent', () => {
  beforeEach(() => {
    mockRecord.mockReset();
    mockRecord.mockResolvedValue({
      xpGranted: 5,
      lumensGranted: 0,
      achievementTitle: 'XP',
    });
    setGamifyGuiEventResultListener(null);
  });

  it('calls voxTransport.recordGuiEvent when enabled', async () => {
    await recordGamifyGuiEvent('chat_message_sent', undefined, { enabled: true });
    expect(mockRecord).toHaveBeenCalledWith('chat_message_sent', undefined);
  });

  it('does not call transport when enabled is false', async () => {
    const result = await recordGamifyGuiEvent('chat_message_sent', undefined, { enabled: false });
    expect(mockRecord).not.toHaveBeenCalled();
    expect(result).toBeNull();
  });

  it('returns the transport DTO when enabled', async () => {
    const result = await recordGamifyGuiEvent('chat_message_sent', undefined, { enabled: true });
    expect(result).toEqual({
      xpGranted: 5,
      lumensGranted: 0,
      achievementTitle: 'XP',
    });
  });

  it('notifies the registered result listener on XP grants', async () => {
    const listener = vi.fn();
    setGamifyGuiEventResultListener(listener);
    await recordGamifyGuiEvent('chat_message_sent', undefined, { enabled: true });
    expect(listener).toHaveBeenCalledWith({
      xpGranted: 5,
      lumensGranted: 0,
      achievementTitle: 'XP',
    });
  });
});
