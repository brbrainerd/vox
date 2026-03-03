# opencode-vox-plugin

Native [OpenCode AI](https://opencode.ai) plugin for the [Vox programming language](https://github.com/vox-lang/vox) — providing real-time orchestrator integration, cost tracking, and multi-agent visualization.

## Features

- **Cost Ticker Overlay** — live token cost in the TUI status bar
- **Agent Topology View** — ASCII graph of active orchestrator agents
- **Gamification HUD** — companion mood, XP, and quest progress
- **File Lock Indicator** — 🔒 on locked files in tool output
- **Auto-Rebalance Notification** — surface `UrgentRebalanceTriggered` events
- **VCS Status Overlay** — snapshot count, conflicts, workspace status

## Requirements

- OpenCode AI >= 0.2.0
- Node.js >= 18
- Vox CLI with `vox-mcp` running (start with: `vox opencode install`)

## Installation

```bash
npm install -g opencode-vox-plugin
```

Or install locally inside your project:

```bash
cd .opencode/plugins/vox-opencode-plugin
npm install
npm run build
```

## Configuration

The plugin is automatically loaded by OpenCode when placed in `.opencode/plugins/`. No additional configuration is required.

Ensure `opencode.json` has the MCP server configured:

```json
{
  "mcp": {
    "vox": {
      "type": "local",
      "command": ["vox-mcp"],
      "enabled": true
    }
  }
}
```

## Compatibility

| Plugin Version | OpenCode Version | Vox CLI Version |
|:-:|:-:|:-:|
| 0.1.x | >= 0.2.0 | >= 0.1.0 |

## License

Apache-2.0
