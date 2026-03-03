/**
 * review.ts
 *
 * Creates a dedicated OpenCode review session using the @reviewer subagent
 * and submits a file or list of changed files for review.
 *
 * Usage:
 *   node --loader ts-node/esm scripts/review.ts [file-or-glob]
 *
 * Examples:
 *   node --loader ts-node/esm scripts/review.ts crates/vox-parser/src/parser.rs
 *   node --loader ts-node/esm scripts/review.ts   # reviews latest git changes
 */
import { createOpencodeClient } from "@opencode-ai/sdk";
import { execSync } from "node:child_process";

const OPENCODE_BASE_URL = process.env.OPENCODE_URL ?? "http://localhost:4096";
const MODEL = {
  providerID: "anthropic",
  modelID: "claude-sonnet-4-20250514",
};

async function getChangedFiles(): Promise<string> {
  try {
    const files = execSync("git diff --name-only HEAD~1..HEAD", {
      cwd: "../..",
    })
      .toString()
      .trim();
    return files || "(no files changed in last commit)";
  } catch {
    return "(could not determine changed files)";
  }
}

async function main() {
  const targetFile = process.argv[2];
  const client = createOpencodeClient({ baseUrl: OPENCODE_BASE_URL });

  console.log(`🔍 Creating code review session...`);

  let files: string;
  if (targetFile) {
    files = targetFile;
  } else {
    files = await getChangedFiles();
    console.log(`📁 Reviewing changed files:\n${files}`);
  }

  const session = await client.session.create({
    body: { title: `vox code review — ${new Date().toISOString().slice(0, 10)}` },
  });

  const reviewPrompt =
    `You are the Vox code reviewer. Review the following files/changes:\n\n` +
    `${files}\n\n` +
    `Check for:\n` +
    `1. No .unwrap() in production code (use ? or .expect("message"))\n` +
    `2. No null — use Option<T> or Result<T, E>\n` +
    `3. Full pipeline coverage — any new AST nodes must be handled in parser → HIR → typeck → both codegens\n` +
    `4. Proper error handling with miette for user-facing errors\n` +
    `5. Scope discipline — env.push_scope()/pop_scope() wrapping binding expressions\n` +
    `6. Test coverage — every new feature must have unit tests\n\n` +
    `Provide a structured review with severity levels (Error, Warning, Info).`;

  const result = await client.session.prompt({
    path: { id: session.id },
    body: {
      model: MODEL,
      parts: [{ type: "text", text: reviewPrompt }],
    },
  });

  console.log(`\n✅ Review session created: ${session.id}`);
  console.log(`📋 Switch to it in OpenCode with <Leader>+Left/Right`);

  if (result && typeof result === 'object' && 'parts' in result) {
    const parts = result.parts as Array<{ type: string; text?: string }>;
    const textParts = parts.filter((p) => p.type === "text").map((p) => p.text ?? "");
    if (textParts.length > 0) {
      console.log("\n─── Review Preview ───");
      console.log(textParts.join("\n"));
    }
  }
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
