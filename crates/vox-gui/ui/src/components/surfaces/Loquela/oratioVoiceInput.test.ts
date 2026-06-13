import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock the Tauri core invoke bridge so the helper runs outside Tauri.
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { captureVoiceTranscript } from './oratioVoiceInput';

describe('captureVoiceTranscript (A4)', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('invokes oratio_transcribe with the seconds arg and returns trimmed text', async () => {
    invokeMock.mockResolvedValue({ text: '  open the orchestrator panel  ' });

    const transcript = await captureVoiceTranscript(5);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    const [cmd, payload] = invokeMock.mock.calls[0];
    expect(cmd).toBe('oratio_transcribe');
    expect(payload).toEqual({ seconds: 5 });
    expect(transcript).toBe('open the orchestrator panel');
  });

  it('returns empty string when nothing was recognized', async () => {
    invokeMock.mockResolvedValue({ text: '' });
    expect(await captureVoiceTranscript()).toBe('');
  });

  it('propagates capture/transcription errors', async () => {
    invokeMock.mockRejectedValue(new Error('microphone capture: no default input device'));
    await expect(captureVoiceTranscript(5)).rejects.toThrow('no default input device');
  });
});
