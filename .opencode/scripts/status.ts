/**
 * status.ts
 *
 * Queries all active OpenCode sessions and prints a summary dashboard.
 * Also calls the vox_orchestrator_status MCP tool via shell to show the
 * Rust-side orchestrator state alongside OpenCode sessions.
 *
 * Usage:
 *   node --loader ts-node/esm scripts/status.ts
 */
import { createOpencodeClient } from "@opencode-ai/sdk";
import { execSync } from "node:child_process";

const OPENCODE_BASE_URL = process.env.OPENCODE_URL ?? "http://localhost:4096";

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

async function main() {
  const client = createOpencodeClient({ baseUrl: OPENCODE_BASE_URL });

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("  🧑‍💻 Vox OpenCode Dashboard");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

  // --- OpenCode Sessions ---
  let sessions;
  try {
    sessions = await client.session.list();
  } catch (err) {
    console.log("\n⚠️  OpenCode not reachable at", OPENCODE_BASE_URL);
    console.log("   Make sure OpenCode is running in another terminal.");
    return;
  }

  console.log(`\n📋 OpenCode Sessions (${sessions.length} total):`);
  if (sessions.length === 0) {
    console.log("   (none)");
  } else {
    const maxTitle = 40;
    for (const s of sessions) {
      const title = truncate(s.title ?? s.id, maxTitle);
      console.log(`   • ${title.padEnd(maxTitle)} [${s.id.slice(0, 8)}]`);
    }
  }

  // --- Vox Orchestrator Status ---
  console.log("\n🔧 Vox Orchestrator Status:");
  try {
    const orchJson = execSync(
      "cargo run -q -p vox-mcp --release -- status 2>/dev/null",
      { cwd: "..", timeout: 15_000 }
    ).toString();

    let orch: {
      data?: {
        agent_count?: number;
        completed?: number;
        markdown_summary?: string;
        companion?: {
          name?: string;
          mood?: string;
          message?: string;
          ascii_sprite?: string;
        }
      }
    } = {};

    try {
      orch = JSON.parse(orchJson);
    } catch {
      // Not a JSON response
    }

    if (orch.data?.markdown_summary) {
      console.log("\n" + orch.data.markdown_summary.trim());
    } else if (orch.data) {
      const d = orch.data;
      console.log(
        `   Agents: ${d.agent_count ?? "?"} | Completed tasks: ${d.completed ?? "?"}`
      );
      if (d.companion) {
        console.log(
          `   Companion: ${d.companion.name ?? "Vox"} [${d.companion.mood ?? "?"}]`
        );
      }
    } else {
      console.log("   (query mode not available — start vox-mcp separately)");
    }
  } catch (err) {
    console.log("   (vox-mcp binary failed or timed out)");
  }

  console.log("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

main().catch((err) => {
  console.error("Fatal error in dashboard:");
  if (err && typeof err === 'object') {
    console.error(JSON.stringify(err, Object.getOwnPropertyNames(err), 2));
  } else {
    console.error(err);
  }
  process.exit(1);
});
