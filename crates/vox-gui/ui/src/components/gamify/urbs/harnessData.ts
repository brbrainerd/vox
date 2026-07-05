// crates/vox-gui/ui/src/components/gamify/urbs/harnessData.ts
import { voxTransport } from '../../../transport';
import type { HarnessSnapshot } from './worldRenderer';

async function tryTap<T>(name: string, fn: () => Promise<T>): Promise<T | null> {
  try {
    return await fn();
  } catch (err) {
    console.error(`[urbs] ${name} unavailable:`, err);
    return null;
  }
}

/** One poll of every harness tap. Each failure → null (landmark unlit). */
export async function fetchHarnessSnapshot(): Promise<HarnessSnapshot> {
  const [ci, vcs, hopper] = await Promise.all([
    tryTap('harness_ci_fleet_status', () => voxTransport.harnessCiFleetStatus()),
    tryTap('vcs_town_status', () => voxTransport.vcsTownStatus()),
    tryTap('hopper_list', () => voxTransport.hopperList()),
  ]);
  return {
    ci: ci ? { runners: ci.runners.filter((r) => r.online), queued: ci.queued } : null,
    vcs: vcs
      ? {
          branches: vcs.branches.map((b) => ({ name: b.name, isHead: b.is_head, track: b.track })),
          prs: vcs.prs_available ? vcs.prs.map((p) => ({ number: p.number, title: p.title })) : [],
        }
      : null,
    // hopper_list returns inbox PLUS assigned (in-flight); only non-assigned
    // items are honestly "queued" for the PORTVS ship count.
    queueLen: hopper ? hopper.filter((t) => t.state !== 'assigned').length : null,
    // No MCP server-list command exists, and get_orchestrator_status
    // serializes a closed struct that can never carry one — AQVAE stays
    // unconditionally unlit until a dedicated command lands (spec §7.1).
    mcp: null,
  };
}

export const HARNESS_POLL_MS = 20_000;
