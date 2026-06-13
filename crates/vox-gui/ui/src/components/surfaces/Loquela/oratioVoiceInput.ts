import { invoke } from '@tauri-apps/api/core';

/** DTO returned by the `oratio_transcribe` Tauri command (A4). */
export interface TranscribeResult {
  text: string;
  raw_text?: string;
  refined_text?: string | null;
}

/**
 * Capture `seconds` of microphone audio and transcribe it via the Oratio
 * plugin, returning the trimmed transcript text (empty string if nothing was
 * recognized). Throws on capture/transcription failure so callers surface it.
 */
export async function captureVoiceTranscript(seconds = 5): Promise<string> {
  const result = await invoke<TranscribeResult>('oratio_transcribe', { seconds });
  return (result?.text ?? '').trim();
}
