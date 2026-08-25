/**
 * Minimal client for Codex / Oratio audio HTTP routes (see contracts/codex-api.openapi.yaml).
 * Point `baseUrl` at the host running `vox-audio-ingress` or your bundled dashboard server.
 *
 * Not used by on-device-only apps — those call `mobile.transcribe_microphone()` instead.
 */

export async function fetchAudioStatus(baseUrl: string): Promise<unknown> {
  const url = `${baseUrl.replace(/\/$/, "")}/api/audio/status`;
  const res = await fetch(url, { method: "GET" });
  if (!res.ok) throw new Error(`GET ${url} failed: ${res.status}`);
  return res.json();
}

export async function transcribePath(
  baseUrl: string,
  path: string,
  languageHint?: string,
): Promise<unknown> {
  const url = `${baseUrl.replace(/\/$/, "")}/api/audio/transcribe`;
  const body: Record<string, string> = { path };
  if (languageHint) body.language_hint = languageHint;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`POST ${url} failed: ${res.status}`);
  return res.json();
}
