---
title: "Getting Started with Vox"
description: "Zero to full-stack in under 5 minutes. Initial setup, first app, and basic concepts."
category: "Tutorials"
sort_order: 1

schema_type: "HowTo"
keywords: ["Vox installation", "getting started Vox", "AI programming language tutorial", "Rust TypeScript compiler"]
---
# Getting Started with Vox

This guide takes you from zero to a running full-stack app in under 5 minutes.

## Prerequisites

Before you begin, make sure you have:

- **Rust** (1.81+) — [Install](https://rustup.rs/)
- **Node.js** (20+) — [Install](https://nodejs.org/)
- **pnpm** (9+) — `npm install -g pnpm`

> **Tip**: Run `vox doctor` to check all dependencies and environment variables are configured correctly.

## Step 1: Install Vox

> [!IMPORTANT]
> **Pre-Release Status:** Vox has not yet reached version 1.0. To use Vox today, you must build it from source.

```bash
git clone https://github.com/vox-foundation/vox.git
cd vox
cargo install --path crates/vox-cli
```

## Step 2: Create a New Project

Use the Vox CLI to scaffold a new application:

```bash
vox init my-app
cd my-app
```

This scaffolds a complete project structure containing a `src/main.vox` entrypoint.

## Step 3: Explore the Generated Code

Open `src/main.vox`. You'll see a starter app that includes a database table, a server endpoint, an interactive UI component, and a routing block. 

```vox
{{#include ../../../examples/golden/getting_started.vox:start}}
```

## Step 4: Type Check

Run a fast static analysis and type check:

```bash
vox check src/main.vox
```

## Step 5: Build

Compile the application to its backend Rust crate and frontend TypeScript components:

```bash
vox build src/main.vox -o dist
```

You'll see step-by-step progress indicating lexical analysis and code generation.

## Step 6: Run

Run the generated binary directly:

```bash
vox run src/main.vox
```

Open `http://localhost:3000` in your browser to view the application.

## Key Concepts

| Decorator | What it does | Resulting Output |
|-----------|-------------|------------------|
| `@table` | Defines a database table | Rust types + Codex migrations |
| `@endpoint(kind: server) fn` | Defines an API endpoint | Axum handler + TS service |
| `component` | Defines a UI component | React/TSX component (Vite) |
| `@endpoint(kind: query) fn` | Read-only db operation | Optimized SQL query fn |
| `@endpoint(kind: mutation) fn`| Write-enabled db operation | SQL insert/update fn |
| `@mcp.tool` | Exposes logic to agents | MCP Tool Definition |
| `workflow` | Durable async process | Logged process (Populi) |
| `activity` | Retriable workflow step | Bound worker (Vox-Dei) |

## What's Next?

- **[Golden Examples](../examples/golden.md)** — Strictly verified code snippets
- **[Language Reference](../reference/ref-syntax.md)** — Full syntax reference
- **[Building Agents](../how-to/how-to-ai-agents.md)** — Build MCP tools and agents
- **[Deployment Guide](../how-to/how-to-deploy.md)** — Production rollout


