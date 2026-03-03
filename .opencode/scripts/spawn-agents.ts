/**
 * spawn-agents.ts
 *
 * Uses the OpenCode SDK to programmatically spawn multiple Vox agents
 * in parallel sessions — one per crate domain — and inject context.
 *
 * Usage:
 *   node --loader ts-node/esm scripts/spawn-agents.ts
 *
 * Requires a running OpenCode instance (opencode must be running locally).
 */
import { createOpencodeClient } from "@opencode-ai/sdk";

const OPENCODE_BASE_URL = process.env.OPENCODE_URL ?? "http://localhost:4096";
const MODEL = {
  providerID: "anthropic",
  modelID: "claude-sonnet-4-20250514",
};

/** Crate domain ↔ agent task pairings */
const AGENT_DOMAINS = [
  {
    title: "vox — Parser & AST Agent",
    crate: "vox-parser",
    task:
      "You are the Vox Parser and AST specialist. Review and improve the recursive descent parser in crates/vox-parser and the AST definitions in crates/vox-ast. " +
      "Focus on error recovery, pattern parsing (grammar.rs), and ensuring all new syntax forms have corresponding AST wrappers. " +
      "Use `vox_collect_diagnostics` and `vox_run_tests` to verify after changes.",
  },
  {
    title: "vox — HIR & Typeck Agent",
    crate: "vox-hir",
    task:
      "You are the Vox HIR and type-checking specialist. Review crates/vox-hir and crates/vox-typeck. " +
      "Ensure all AST nodes are correctly lowered to HIR (lower.rs) and that bidirectional type inference (check.rs) is complete and sound. " +
      "Focus on name resolution and scope handling. Run cargo test -p vox-typeck after any changes.",
  },
  {
    title: "vox — CodeGen Agent (Rust/TS)",
    crate: "vox-codegen-rust",
    task:
      "You are the Vox code generation specialist. Review crates/vox-codegen-rust and crates/vox-codegen-ts. " +
      "Ensure every HIR expression variant has complete emission in both Rust and TypeScript backends. " +
      "Verify JSX/TSX emission logic in vox-codegen-ts. Run cargo test -p vox-codegen-rust after any changes.",
  },
  {
    title: "vox — Orchestrator & MCP Agent",
    crate: "vox-orchestrator",
    task:
      "You are the Vox orchestration specialist. Review crates/vox-orchestrator and crates/vox-mcp. " +
      "Ensure MCP tool schemas are complete, file affinity logic is robust, and all orchestrator features are covered by tests. " +
      "Focus on the rmcp integration and gamification persistence. Run cargo test -p vox-orchestrator after any changes.",
  },
] as const;

async function main() {
  const client = createOpencodeClient({ baseUrl: OPENCODE_BASE_URL });

  console.log(`🚀 Connecting to OpenCode at ${OPENCODE_BASE_URL}...`);

  // Verify connectivity
  const sessions = await client.session.list().catch(() => null);
  if (sessions === null) {
    console.error(
      "❌ Could not connect to OpenCode. Make sure it is running:\n  opencode"
    );
    process.exit(1);
  }
  console.log(`✅ Connected. ${sessions.length} existing sessions.`);

  const created: Array<{ title: string; id: string }> = [];

  for (const domain of AGENT_DOMAINS) {
    console.log(`\n🤖 Spawning session: ${domain.title}`);

    const session = await client.session.create({
      body: { title: domain.title },
    });

    // Inject initial context without triggering a response yet
    await client.session.prompt({
      path: { id: session.id },
      body: {
        noReply: true,
        parts: [
          {
            type: "text",
            text:
              `You are working exclusively on the \`${domain.crate}\` crate domain. ` +
              `Project root: vox/. Always run targeted tests after changes. ` +
              `Never use .unwrap() in production code — use ? or .expect("message").`,
          },
        ],
      },
    });

    // Send the actual task prompt
    await client.session.prompt({
      path: { id: session.id },
      body: {
        model: MODEL,
        parts: [{ type: "text", text: domain.task }],
      },
    });

    created.push({ title: domain.title, id: session.id });
    console.log(`  ✅ Created session ${session.id}`);
  }

  console.log(`\n✨ Spawned ${created.length} agent sessions:`);
  for (const s of created) {
    console.log(`  • [${s.id}] ${s.title}`);
  }
  console.log(
    "\nSwitch between sessions in OpenCode with <Leader>+Left/Right."
  );
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
