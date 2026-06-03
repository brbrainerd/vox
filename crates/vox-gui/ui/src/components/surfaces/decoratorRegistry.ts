import React from 'react';
import { CommandCardsView, SurfaceCard } from './CommandCardsView';
import { ScientiaDashboard } from './Scientia/ScientiaDashboard';
import { ClaimsView } from './Scientia/ClaimsView';
import { CoverageView } from './Coverage/CoverageView';

/**
 * Props every surface decorator receives. Decorators are hand-built views that
 * replace the default (generated/built-in) view for a surface key. They MUST
 * route command execution through the shared `execute_command` Tauri path so
 * every surface stays on one run seam.
 */
export interface SurfaceDecoratorProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/** Build a read-only command-cards decorator for a Tier-3 CLI surface. */
function commandSurface(
  title: string,
  subtitle: string,
  cards: SurfaceCard[]
): React.ComponentType<SurfaceDecoratorProps> {
  return function Surface({ pushToast }: SurfaceDecoratorProps) {
    return React.createElement(CommandCardsView, { title, subtitle, cards, pushToast });
  };
}

/**
 * Surface key → decorator. `App.tsx::renderView` consults this before its
 * built-in switch, so promoting a surface to a decorated view is a one-line
 * registration here and removing the entry reverts to the default with no other
 * change. Each command below is an arg-free, read-only CLI command.
 */
export const surfaceDecorators: Record<string, React.ComponentType<SurfaceDecoratorProps>> = {
  scientia: ScientiaDashboard,
  claims: ClaimsView,
  coverage: CoverageView,
  mens: commandSurface('Vox Mens', 'ML training & local models', [
    { key: 'status', title: 'Training Status', description: 'Latest run telemetry', path: ['mens', 'status'] },
    { key: 'models', title: 'Model Registry', description: 'Locally trained models', path: ['mens', 'models'] },
    { key: 'probe', title: 'GPU Probe', description: 'Detected accelerators + LoRA fit', path: ['mens', 'probe'] },
  ]),
  populi: commandSurface('Vox Populi', 'Distributed mesh network', [
    { key: 'status', title: 'Mesh Status', description: 'Network health + overlay diagnostics', path: ['populi', 'status'] },
    { key: 'registry', title: 'Local Snapshot', description: 'On-disk registry + environment', path: ['populi', 'registry-snapshot'] },
  ]),
  research: commandSurface('Vox Research', 'Deep-research backends', [
    { key: 'status', title: 'Backend Status', description: 'SearXNG / DDG / Tavily health', path: ['research', 'status'] },
    { key: 'history', title: 'Recent Sessions', description: 'Persisted research sessions', path: ['research', 'history'] },
    { key: 'config', title: 'Configuration', description: 'Resolved research config', path: ['research', 'config', 'show'] },
  ]),
  oratio: commandSurface('Vox Oratio', 'Speech-to-code runtime', [
    { key: 'doctor', title: 'Runtime Health', description: 'Oratio runtime + configuration diagnostics', path: ['oratio', 'doctor'] },
    { key: 'status', title: 'Backend Status', description: 'Available backends + passthrough modes', path: ['oratio', 'status'] },
  ]),
};
