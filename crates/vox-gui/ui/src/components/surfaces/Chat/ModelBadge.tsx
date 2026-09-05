import React, { useState } from 'react';

/// Mirrors Rust `SelectionSource` (`crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`),
/// serialized `#[serde(rename_all = "snake_case")]`. No generated-types file
/// exists for this DTO yet (checked `crates/vox-gui/ui/src/types/`), so it is
/// hand-declared here alongside its only consumer.
export type SelectionSource = 'user_override' | 'global' | 'auto_routed' | 'fallback';

export interface SelectionDto {
  model: string;
  source: SelectionSource;
  rationale?: string | null;
}

const SELECTION_SOURCE_LABEL: Record<SelectionSource, string> = {
  user_override: 'Your pick',
  global: 'Sticky default',
  auto_routed: 'Auto-routed',
  fallback: 'Fell back',
};

interface ModelBadgeProps {
  model?: string;
  provider?: string;
  reqTokens?: number;
  respTokens?: number;
  costUsd?: number;
  selectionReason?: string;
  latencyMs?: number;
  selection?: SelectionDto;
}

export function ModelBadge({
  model,
  provider,
  reqTokens,
  respTokens,
  costUsd,
  selectionReason,
  latencyMs,
  selection,
}: ModelBadgeProps) {
  const [open, setOpen] = useState(false);

  if (!model) {
    return <span className="text-[10px] text-zinc-600">model unknown</span>;
  }

  return (
    <span className="relative">
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        aria-expanded={open}
        aria-label={`Completed by ${model} — details`}
        className="rounded border border-brass/30 px-1.5 py-0.5 text-[10px] text-brass hover:bg-brass/[0.08]"
      >
        {model}
        {reqTokens != null && (
          <span className="ml-1 text-zinc-500">{reqTokens}↑ {respTokens}↓</span>
        )}
        {costUsd != null && (
          <span className="ml-1 text-zinc-500">${costUsd.toFixed(2)}</span>
        )}
        <span className="ml-1 text-zinc-600" aria-hidden>ⓘ</span>
      </button>
      {open && (
        <div
          className="absolute right-0 z-50 mt-1 w-64 rounded-md border border-white/10 bg-[#0b0b0e] p-2 text-[10px] text-zinc-300"
        >
          {selection && (
            <div className="font-medium text-zinc-200">
              {SELECTION_SOURCE_LABEL[selection.source]}
              {selection.source === 'fallback' && selection.rationale && (
                <span className="ml-1 font-normal text-zinc-400">— {selection.rationale}</span>
              )}
            </div>
          )}
          {provider && <div>provider: {provider}</div>}
          {selectionReason && <div>reason: {selectionReason}</div>}
          {latencyMs != null && <div>latency: {latencyMs} ms</div>}
          <div className="mt-1 text-zinc-500">
            What was sent / received is available when I/O capture is enabled.
          </div>
        </div>
      )}
    </span>
  );
}
