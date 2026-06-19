import type { AttentionBudgetSnapshot } from '../../types/tauri';
import { AttentionBudgetMeter } from '../surfaces/AttentionBudgetMeter';

interface AttentionStripProps {
  budget: AttentionBudgetSnapshot | null | undefined;
  waitingQuestions: number;
  blockedTasks: number;
}

export function AttentionStrip({ budget, waitingQuestions, blockedTasks }: AttentionStripProps) {
  if (!budget) return null;
  return (
    <div className="flex items-center gap-3 px-3 py-1.5 bg-[#0c0c0e] border-b border-zinc-800">
      <AttentionBudgetMeter budget={budget} waitingQuestions={waitingQuestions} blockedTasks={blockedTasks} />
    </div>
  );
}
