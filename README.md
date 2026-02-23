<div align="center">
  <img src="docs/src/assets/logo.png" alt="Vox Logo" width="200" height="auto" />
  <h1>Vox Programming Language</h1>
  <p><strong>The Native Language of the AI Era.</strong></p>
  <p>Write full-stack, durable, type-safe applications in a single file designed for LLMs to generate flawlessly.</p>
</div>

<img src="docs/src/assets/hero_bg.png" alt="Vox Hero" width="100%" />

---

## 🚀 The Vox Philosophy

The software development workflow is fundamentally changing. Humans are transitioning from typists to architects. **Vox** is built from the ground up to be the ultimate target compilation language for frontier models (like Populi, Claude, and GPT-4).

- **Type-Safe & Predictable**: Strong static typing, Algebraic Data Types (ADTs), and an exhaustive match guarantee prevent hallucinations.
- **Zero-Boilerplate Full-Stack**: Define your data layer natively (`@table`), write your backend (`@server fn`), and compose your React UI (`@component`) — all compiled natively into Rust and TypeScript.
- **Next-Gen Durable Execution**: Integrated workflow and actor systems ensure that your process is robust. Awaiting an `activity` behaves locally, but compiles to distributed, retryable task execution.
- **Built for Agents**: Natively integrated `@mcp.tool` decorators directly export functions into the Model Context Protocol format.

## ⚡ Quick Start

### Installation

```bash
cargo install --path crates/vox-cli
```

### The "All-in-One" Experience

```vox
@table type Task:
    title: str
    done: bool

@server fn toggle_task(id: int) to Result[Unit]:
    // Built-in SQL execution mapped from your ADTs
    let task = db.query_one("SELECT * FROM Task WHERE id = ?", id)
    db.execute("UPDATE Task SET done = ? WHERE id = ?", !task.done, id)
    ret Ok(())

@component fn TaskItem(task: Task) to Element:
    ret <li>
        <input type="checkbox" checked={task.done} onChange={fn(e) toggle_task(task.id)} />
        {task.title}
    </li>
```

### Build, Bundle, Ship

```bash
# Develop instantly with live-reload
vox build app.vox -o dist --watch

# Ship a single statically-linked binary containing Frontend + Backend + SQLite
vox bundle app.vox --release --target x86_64-unknown-linux-musl
```

## 🧠 Documentation & Internals

Vox is powered by a robust compiler pipeline written in pure Rust:
Lexer (`logos`) ➔ Parser ➔ HIR Lowering ➔ Typeck (Hindley-Milner) ➔ Codegen (`axum` / `react`).

For a deep dive into the language architecture, development roadmap, and the Populi LLM model, see [AGENTS.md](./AGENTS.md).

## 🌍 Community & License

Vox is open-source under the **Apache-2.0** License. Join us in building the language of the AI era.
