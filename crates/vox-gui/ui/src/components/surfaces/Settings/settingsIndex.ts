import { GENERATED_SETTINGS_INDEX } from '../../../config/generatedSettingsIndex';

export interface SettingEntry {
  id: string; // unique, kebab-case
  section: string; // section id used by SettingsView
  label: string; // visible Row label
  hint: string; // visible Row hint
  keywords: string[];
}

export const SETTINGS_INDEX: SettingEntry[] = [
  { id: 'orch-max-agents', section: 'orchestrator', label: 'Max concurrent agents', hint: 'Hard cap before scheduler back-pressure', keywords: ['concurrency', 'fleet', 'parallel'] },
  { id: 'orch-budget', section: 'orchestrator', label: 'Global budget cap', hint: 'Soft + hard USD cap', keywords: ['cost', 'cap', 'spend', 'usd'] },
  { id: 'orch-doubt-threshold', section: 'orchestrator', label: 'Auto-doubt threshold', hint: 'Confidence floor for Augur', keywords: ['trust', 'augur', 'verify'] },
  { id: 'orch-isolation', section: 'orchestrator', label: 'Default isolation tier', hint: 'Runtime sandbox: wasm, container, native', keywords: ['sandbox', 'wasm', 'container', 'scope'] },
  { id: 'scaling-enabled', section: 'scaling', label: 'Auto-scaling', hint: 'Spawn/retire agents based on load and resources', keywords: ['scale', 'autoscale', 'dynamic'] },
  { id: 'scaling-min-agents', section: 'scaling', label: 'Min agents', hint: 'Never retire below this fleet size', keywords: ['floor', 'scale down'] },
  { id: 'scaling-threshold', section: 'scaling', label: 'Queue threshold', hint: 'Per-agent load that triggers scale-up', keywords: ['queue', 'pressure', 'load'] },
  { id: 'scaling-cpu-ceiling', section: 'scaling', label: 'CPU ceiling', hint: 'Block agent spawn above this local CPU usage', keywords: ['cpu', 'resources', 'host'] },
  { id: 'scaling-mem-floor', section: 'scaling', label: 'Memory floor', hint: 'Block agent spawn below this free RAM', keywords: ['ram', 'memory', 'resources'] },
  { id: 'llm-max-concurrency', section: 'llm', label: 'Max parallel LLM requests', hint: 'Global ceiling across providers', keywords: ['openrouter', 'parallel', 'concurrency', 'rate limit', 'throttle'] },
  { id: 'llm-openrouter-cap', section: 'llm', label: 'OpenRouter override', hint: 'Provider-specific concurrency cap', keywords: ['openrouter', 'provider', 'cap'] },
  { id: 'llm-retry', section: 'llm', label: '429 retry attempts', hint: 'Backoff retries on rate limit', keywords: ['retry', 'backoff', '429', 'rate limit'] },
  { id: 'routing-priority', section: 'routing', label: 'Model routing emphasis', hint: 'Intelligence / efficiency / responsiveness', keywords: ['model', 'routing', 'efficiency', 'precision', 'latency'] },
  { id: 'mesh-nodes', section: 'mesh', label: 'Mesh nodes', hint: 'Discover and trust mesh peers', keywords: ['populi', 'peers', 'nodes', 'distributed'] },
  { id: 'signing-keys', section: 'signing', label: 'Signing keys', hint: 'ed25519 key status and rotation', keywords: ['ed25519', 'rotate', 'signature'] },
  { id: 'secrets-keys', section: 'secrets', label: 'Keys & secrets', hint: 'Provider API keys (OpenRouter, Gemini, …)', keywords: ['api key', 'openrouter', 'anthropic', 'token', 'clavis'] },
  { id: 'telemetry-mode', section: 'telemetry', label: 'Telemetry', hint: 'Off, local OTLP, or cloud', keywords: ['otlp', 'tracing', 'privacy'] },
  { id: 'keybinds', section: 'keybinds', label: 'Keybinds', hint: 'Global keyboard shortcuts', keywords: ['shortcuts', 'hotkeys', 'keyboard'] },
  { id: 'theme', section: 'theme', label: 'Theme', hint: 'Arcane, Void, or Glacier', keywords: ['dark', 'appearance', 'color'] },
  { id: 'gamify', section: 'gamify', label: 'Gamification', hint: 'Enable and pick a mode', keywords: ['ludus', 'rewards', 'xp'] },
  // Registry-derived entries (generated from CONFIG_KEYS by `vox ci config-gui-codegen`).
  ...GENERATED_SETTINGS_INDEX,
];

export function searchSettings(query: string): SettingEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return SETTINGS_INDEX.filter(
    s =>
      s.label.toLowerCase().includes(q) ||
      s.hint.toLowerCase().includes(q) ||
      s.keywords.some(k => k.includes(q)),
  );
}
