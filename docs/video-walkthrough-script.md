# Vox ↔ AI Agents: Setup & Multi-Agent Coding Session
## Video Walkthrough Script

**Duration**: ~8 minutes
**Audience**: Developers new to Vox
**Format**: Screen recording + voiceover

---

## Scene 1: Introduction (0:00–0:30)

> *Screen: Terminal, blank*

"Welcome to Vox — the AI-native programming language built for multi-agent development workflows.

In this video, I'll show you how to set up a fully integrated Vox + AI Agent development environment, and kick off a real multi-agent coding session with parallel task execution."

---

## Scene 2: Install Vox CLI (0:30–1:00)

> *Screen: Terminal*

```bash
# Install Vox CLI
cargo install --path crates/vox-cli
```

"First, install the Vox CLI. This gives you the `vox` command for everything — building, testing, code review, and orchestration."

---

## Scene 3: Initialize AI Agents (1:00–1:40)

> *Screen: Terminal*

```bash
# Initialize Vox AI integration
vox agent init
```

> *Screen shows: colored output with ✅ checks for each component*

"Run `vox agent init` — this scaffolds the default JSON config and runs a preflight check to make sure everything is wired up correctly."

"You'll see it check: the MCP server, the LSP, and your agent prompts."

---

## Scene 4: Inspect the Scaffold (1:40–2:20)

> *Screen: File tree, then editor showing vox-agent.json*

"Vox has created a `vox-agent.json` at the root. This wires the LSP and native MCP server."

```json
{
  "mcp": {
    "vox": {
      "type": "local",
      "command": ["vox-mcp"]
    }
  }
}
```

"And in `.vox/agents/`, you'll find role-specific agent prompts — orchestrator, visualizer — each scoped to specific parts of the codebase."

---

## Scene 5: Launch the Dashboard (2:20–3:00)

> *Screen: New terminal*

```bash
vox dashboard
```

> *Screen: Browser opens to dashboard at localhost:8080*

"Before we start coding, let's launch the Vox Dashboard. This gives us a real-time view of every agent, task queue, and file lock."

"Right now it's empty — we'll watch it come alive as we spawn agents."

---

## Scene 6: Launch Editor (3:00–4:00)

> *Screen: VS Code opens, Vox extension initializes*

"Now we launch our Editor. You'll see the Vox plugin initialize in the status bar — it's connected to our native MCP server."

"The plugin shows our live cost ticker, agent topology overlay, and gamification HUD — all pulling from the Vox orchestrator in real time."

---

## Scene 7: Submit a Multi-Agent Task (4:00–5:30)

> *Screen: Vox Agent Chat panel*

"Let's give the orchestrator something interesting. I'll ask it to build a new REST endpoint and write the tests in parallel."

```
@plan Design a /api/tasks endpoint for CRUD operations.
Submit separate tasks to the backend agent and test agent simultaneously.
```

> *Screen: Switch to Dashboard — two agents appear, task cards animate in*

"Watch the Vox Dashboard — two agents have spawned. The backend agent has claimed `src/routes/tasks.rs` and the test agent is working on `tests/tasks_test.rs`. File locks prevent conflicts automatically."

---

## Scene 8: Shell Completions (5:30–6:00)

> *Screen: Terminal*

```bash
# Set up completions for your shell
vox completions bash > ~/.local/share/bash-completion/completions/vox

# Now Tab works!
vox agent <TAB>
# init  setup  doctor  dashboard  spawn  review  config  sync  logs  share
```

"Pro tip: generate shell completions to get full tab-complete for every Vox subcommand."

---

## Scene 9: Wrap Up (6:00–8:00)

> *Screen: Terminal showing `vox agent doctor` output*

```bash
vox agent doctor
```

> *Screen: Colored pass/fail output for all checks*

"Run `vox agent doctor` any time to get a health check of your integration."

"In summary, we've initialized the native AI integration, scaffolded a multi-agent config, launched the real-time dashboard, and watched two agents collaborate on a feature — automatically, in parallel, without conflicts."

"Check out the docs at `docs/how-to-ai-agents.md` and `docs/mcp-tool-reference.md` for a full reference."

"Thanks for watching!"

---

## Production Notes

- Record at 1920×1080, 60fps
- Use the Vox dark theme terminal (`Windows Terminal` with `One Dark Pro`)
- Typewriter speed: ~60 WPM for demo commands
- Dashboard recording: use `vox dashboard` on port 8080
- Suggested editing tool: DaVinci Resolve or ScreenStudio
