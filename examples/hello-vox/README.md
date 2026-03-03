# Hello Vox Example

A complete full-stack note-taking app in a single `.vox` file, demonstrating:

- `@table` — database table definition
- `@server fn` — backend API endpoints
- `@component fn` — React UI components
- `routes:` — URL routing
- `style:` — inline CSS

## Quick Start

```bash
# 1. Build the Vox source (generates Rust + TypeScript)
vox build src/main.vox -o dist

# 2. Run the full-stack app (backend + frontend)
vox run src/main.vox

# 3. Open in browser
open http://localhost:3000
```

## What Happens

When you run `vox build`, the compiler:

1. **Lexes** the `.vox` source into tokens
2. **Parses** into an AST (abstract syntax tree)
3. **Type checks** using Hindley-Milner inference
4. **Lowers** to HIR (high-level intermediate representation)
5. **Generates TypeScript** for `@component` and client-side code
6. **Generates Rust** for `@table`, `@server`, and backend logic

When you run `vox run`, it:

1. Compiles the generated Rust into a single binary
2. Bundles the TypeScript with Vite
3. Starts the server on port 3000
4. Serves both the API and the frontend

## Project Structure

```
hello-vox/
├── Vox.toml          # Project manifest
├── src/
│   └── main.vox      # Your entire app in one file
├── dist/             # Generated TypeScript (after build)
└── target/
    └── generated/    # Generated Rust (after build)
```

## Next Steps

- Add more `@table` types for different data
- Add `@query` and `@mutation` functions for database access
- Add more `@component` functions for UI pages
- Add `workflow` and `activity` blocks for durable async operations
