# Setup & Installation

This guide covers everything you need to get Vox running on any platform.

## Quick Install (30 seconds)

```bash
git clone https://github.com/brbrainerd/vox && cd vox

# Linux / macOS / WSL
./scripts/install.sh

# Windows (PowerShell)
.\scripts\install.ps1
```

Both scripts install Rust, Node.js, and the `vox` CLI, then prompt you through AI key setup.

## Cross-Platform Setup Wizard

After installing `vox`, run the built-in setup wizard:

```bash
vox setup                    # Interactive setup
vox setup --dev              # Include dev tools (clippy, nextest)
vox setup --non-interactive  # CI mode (reads env vars only)
```

The wizard checks:

| Check | Required? | How to Fix |
|---|---|---|
| Rust ≥ 1.80 | ✅ | [rustup.rs](https://rustup.rs) |
| Node.js ≥ 18 | Optional | [nodejs.org](https://nodejs.org) |
| Git | ✅ | [git-scm.com](https://git-scm.com) |
| C compiler (MSVC/gcc/clang) | ✅ | Platform-specific (see below) |
| Google AI Studio Key | Recommended | Free at [aistudio.google.com/apikey](https://aistudio.google.com/apikey) |
| OpenRouter Key | Optional | [openrouter.ai/keys](https://openrouter.ai/keys) |
| Ollama | Optional | [ollama.com](https://ollama.com) |
| VoxDB directory writable | ✅ | `~/.vox/` must exist and be writable |

## AI Provider Keys

Vox uses a **three-layer model cascade** — you get free AI with just a Google account:

### Layer 1: Google AI Studio (Free, Primary)

No credit card required. Provides Gemini 2.5 Flash, Flash-Lite, and Pro.

```bash
# Get your key (takes 10 seconds):
# https://aistudio.google.com/apikey

vox login --registry google YOUR_KEY
```

### Layer 2: OpenRouter (Optional)

Free API key unlocks dozens of `:free` models (Devstral 2, Qwen3 Coder, Llama 4 Scout, Kimi K2). Paid key unlocks SOTA models (DeepSeek v3.2, Claude Sonnet 4.5, GPT-5, O3).

```bash
vox login --registry openrouter YOUR_KEY
```

### Layer 3: Ollama (Optional, Local)

Zero-auth local inference. Install Ollama, pull a model, and Vox auto-detects it.

```bash
ollama pull llama3.2
# Vox detects Ollama on localhost:11434 automatically
```

## Verify Your Environment

```bash
vox doctor
```

Example output:
```
  ✓  Rust / Cargo              cargo 1.82.0
  ✓  Node.js                   v20.11.0 (>= v18)
  ✓  Git                       git version 2.44.0
  ✓  C Compiler                MSVC Build Tools found
  ✓  Google AI Studio Key      configured (free Gemini models available)
  ○  OpenRouter Key (optional) not configured
  ○  Ollama Local (optional)   not running
  ✓  VoxDB directory           C:\Users\you\.vox (writable)

  ✓ All checks passed — you're ready to build with Vox!
```

## Docker

```bash
# Build from source
docker build -t vox .

# Run MCP server
docker run -e GEMINI_API_KEY=... -p 3000:3000 vox

# Full stack with docker compose
cp .env.example .env  # fill in GEMINI_API_KEY
docker compose up
```

## Platform-Specific Notes

### Windows
- **C Compiler:** Install VS Build Tools via `winget install Microsoft.VisualStudio.2022.BuildTools`
- Alternatively, use WSL: `wsl ./scripts/install.sh`

### macOS
- **C Compiler:** `xcode-select --install`

### Linux
- **C Compiler:** `sudo apt-get install build-essential` (Debian/Ubuntu)
