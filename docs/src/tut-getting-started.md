# Getting Started with Vox

This guide takes you from zero to a running full-stack app in under 5 minutes.

## Prerequisites

Before you begin, make sure you have:

- **Rust** (1.75+) — [Install](https://rustup.rs/)
- **Node.js** (18+) — [Install](https://nodejs.org/)
- **pnpm** — `npm install -g pnpm`

> **Tip**: Run `vox doctor` to check all dependencies are installed correctly.

## Step 1: Install Vox

```bash
# Clone and build from source
git clone https://github.com/brbrainerd/vox.git
cd vox
cargo build --release

# Add to your PATH
export PATH="$PWD/target/release:$PATH"
```

## Step 2: Create a New Project

```bash
# Create a new Vox application
mkdir my-app && cd my-app
vox init --kind application
```

This creates:
```
my-app/
├── Vox.toml          # Project manifest
├── src/
│   └── main.vox      # Your app source (full-stack in one file!)
└── .vox_modules/     # Package cache
```

## Step 3: Explore the Generated Code

Open `src/main.vox`. You'll see a starter app with:

```vox
# Skip-Test
# Data layer — creates a database table
@table type Note:
    title: str
    content: str
    created_at: str

# API layer — creates backend endpoints
@server fn add_note(title: str, content: str) to Result[str]:
    ret Ok("Added: " + title)

# UI layer — creates React components
@component fn App() to Element:
    <div class="app">
        <h1>"My Vox App"</h1>
        <p>"Edit src/main.vox to get started"</p>
    </div>

# Routing — maps URLs to components
routes:
    "/" to App
```

## Step 4: Build

```bash
vox build src/main.vox -o dist
```

You'll see step-by-step progress:

```
  [1/6] Lexing... ✓ (42 tokens, 1ms)
  [2/6] Parsing... ✓ (5 declarations, 2ms)
  [3/6] Type checking... ✓ (1ms)
  [4/6] Lowering to HIR... ✓ (0ms)
  [5/6] Generating TypeScript... ✓ (2 files, 1ms)
  [6/6] Generating Rust... ✓ (3 files, 2ms)

✓ Built in 7ms — 2 TS file(s), 3 Rust file(s) generated
```

## Step 5: Run

```bash
vox run src/main.vox
```

Open [http://localhost:3000](http://localhost:3000) in your browser. You should see your app!

## Step 6: Edit and Rebuild

Make a change to `src/main.vox` — maybe add a new server function:

```vox
@server fn greet(name: str) to Result[str]:
    ret Ok("Hello, " + name + "!")
```

Then rebuild and re-run:

```bash
vox build src/main.vox -o dist && vox run src/main.vox
```

## Key Concepts

| Decorator | What it does | Compiles to |
|-----------|-------------|-------------|
| `@table` | Defines a database table | Rust migration + query types |
| `@server fn` | Creates an API endpoint | Rust Axum handler + TS client |
| `@component fn` | Creates a UI component | React TSX component |
| `@query fn` | Read-only database query | Rust query function |
| `@mutation fn` | Write database operation | Rust mutation function |
| `workflow` | Durable async process | Rust async with journal |
| `activity` | Retryable workflow step | Rust async with retry config |

## What's Next?

- **[Hello Vox Example](../examples/hello-vox/)** — A complete note-taking app
- **[Language Guide](./ref-language.md)** — Full syntax reference
- **[AI Agents Guide](./how-to-ai-agents.md)** — Use built-in AI agents with Vox
- **[Deployment Guide](./deployment.md)** — Ship to production
