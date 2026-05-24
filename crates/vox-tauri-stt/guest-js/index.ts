/**
 * Guest (WebView) facade for `vox-tauri-stt`.
 * Contract: `transcribe(): Promise<{ text: string; confidence?: number }>`
 */
import { invoke } from "@tauri-apps/api/core";

export interface TranscribeResult {
  text: string;
  confidence?: number;
}

const DEBUG =
  typeof localStorage !== "undefined" &&
  localStorage.getItem("VOX_DEBUG_STT") === "1";

export async function transcribe(): Promise<TranscribeResult> {
  if (DEBUG) {
    console.debug("[vox-tauri-stt] invoke transcribe with empty payload");
  }
  return invoke<TranscribeResult>("plugin:vox-stt|transcribe", {});
}
