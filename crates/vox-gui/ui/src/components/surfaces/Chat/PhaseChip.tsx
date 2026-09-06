import React from "react";

/** High-level Plan/Act/Verify phase tag. Mirrors the Rust `PavPhase` enum. */
export type PavPhase = "planning" | "acting" | "verifying" | "done";

const LABEL: Record<PavPhase, string> = {
  planning: "Planning…",
  acting: "Acting…",
  verifying: "Verifying…",
  done: "Done",
};

interface PhaseChipProps {
  phase: PavPhase;
  onApprovePlan: () => void;
  onSkipVerify: () => void;
  onForceVerify: () => void;
}

/**
 * Renders the current PAV phase alongside contextual intervention buttons:
 *
 * - `planning`  → "Approve Plan →" button
 * - `acting`    → "Force Verify" button
 * - `verifying` → "Skip Verify" button
 * - `done`      → checkmark, no buttons
 */
export function PhaseChip({
  phase,
  onApprovePlan,
  onSkipVerify,
  onForceVerify,
}: PhaseChipProps) {
  return (
    <span className="inline-flex items-center gap-2 text-[10px]">
      {/* Phase badge */}
      <span
        className="rounded-sm border border-brass/30 px-1.5 py-0.5 font-mono text-brass"
        data-testid="phase-chip-label"
      >
        {LABEL[phase]}
      </span>

      {phase === "planning" && (
        <button
          type="button"
          aria-label="approve plan"
          className="text-zinc-400 hover:text-zinc-200 transition-colors"
          onClick={onApprovePlan}
        >
          Approve Plan →
        </button>
      )}

      {phase === "acting" && (
        <button
          type="button"
          aria-label="force verify"
          className="text-zinc-400 hover:text-zinc-200 transition-colors"
          onClick={onForceVerify}
        >
          Force Verify
        </button>
      )}

      {phase === "verifying" && (
        <button
          type="button"
          aria-label="skip verify"
          className="text-zinc-400 hover:text-zinc-200 transition-colors"
          onClick={onSkipVerify}
        >
          Skip Verify
        </button>
      )}

      {phase === "done" && (
        <span className="text-emerald-400" aria-label="phase complete">
          ✓
        </span>
      )}
    </span>
  );
}
