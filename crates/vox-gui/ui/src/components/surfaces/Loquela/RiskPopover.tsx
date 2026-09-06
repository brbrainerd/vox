import React, { useEffect, useRef } from 'react';
import { RISK_POSTURES, type RiskId, type ControlState } from '../../../lib/driveConsole';

const COPY: Record<RiskId, string> = {
  high: 'Break things — auto-approve more, gates shadow-only, fewer safety tokens.',
  moderate: 'Confirm + enforce grounding. Balanced safety.',
  low: 'Enforce verification + grounding, raise approval, spend safety tokens, lean model up.',
};

interface RiskPopoverProps {
  risk: RiskId;
  open: boolean;
  onChange: (next: Partial<ControlState>) => void;
  onClose: () => void;
}

export function RiskPopover({ risk, open, onChange, onClose }: RiskPopoverProps) {
  const activeBtnRef = useRef<HTMLButtonElement>(null);

  // Move focus into the dialog when it opens so keyboard users land on the
  // currently-selected posture (Escape handling already returns focus out).
  useEffect(() => {
    if (open) activeBtnRef.current?.focus();
  }, [open]);

  if (!open) return null;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };

  return (
    <div
      role="dialog"
      aria-label="Configure acceptable risk"
      onKeyDown={handleKeyDown}
      className="absolute bottom-full left-0 z-50 mb-1.5 w-72 rounded-lg border border-white/10 bg-[#0b0b0e] p-3 text-[11px] shadow-xl"
    >
      <div className="mb-2 text-[10px] uppercase tracking-widest text-zinc-500">Acceptable risk</div>
      {RISK_POSTURES.map(p => (
        <button
          key={p.id}
          type="button"
          ref={risk === p.id ? activeBtnRef : undefined}
          onClick={() => { onChange({ risk: p.id }); }}
          aria-pressed={risk === p.id}
          className={`mb-1 flex w-full flex-col rounded-md border px-2 py-1.5 text-left ${
            risk === p.id ? 'border-brass/40 bg-brass/10' : 'border-white/8 hover:border-white/20'
          }`}
        >
          <span className="font-medium capitalize">{p.label} risk</span>
          <span className="text-zinc-400">{COPY[p.id]}</span>
        </button>
      ))}
    </div>
  );
}
