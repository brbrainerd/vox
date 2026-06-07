// DTOs mirror crates/vox-gui/src/commands/policy.rs (#[serde(rename_all = "camelCase")]).
export interface PolicyRow {
  id: string;
  domain: string;
  title: string;
  group: string;
  severity: string | null;
  blocking: boolean;
  protected: boolean;
}

export interface PolicyDetail extends PolicyRow {
  description: string;
  runsOn: string[];
  origin: string;
  docs: string | null;
  sourceKind: string;
  sourceRef: string;
  sourceDetail: string | null;
}

export interface BranchInfo {
  branch: string;
  path: string;
  isCurrent: boolean;
}

export type RunStatus = 'pass' | 'fail' | 'warn' | 'not_run';

export interface PolicyStatus {
  branch: string;
  id: string;
  status: RunStatus;
  hits: number;
}

/** Worst-first status precedence for roll-ups + the master badge. */
export const STATUS_RANK: Record<RunStatus, number> = { fail: 3, warn: 2, pass: 1, not_run: 0 };
