# Tutorial: Building a Collaborative Todo List

Learn how to build a full-stack, collaborative todo list app with Vox. This tutorial covers data modeling, server-side logic, and UI integration.

## 1. Project Initialization

Create a new directory and initialize a Vox application:

```bash
mkdir vox-todo && cd vox-todo
vox init --kind application
```

## 2. Define the Data Model

Open `src/main.vox`. We'll start by defining what a "Todo" is. Using the `@table` decorator, we create a persistent database table.

```vox
@table type Todo:
    title: str
    completed: bool
    created_at: int
```

## 3. Implement Server Logic

Next, we add `@server` functions to create and update todos. These functions automatically generate the necessary Rust handlers and TypeScript clients.

```vox
@server fn add_todo(title: str) to Result[str]:
    # In a real app, you'd perform a db.insert here
    ret Ok("Created: " + title)

@server fn toggle_todo(id: str) to Result[bool]:
    # Flip the completed state
    ret Ok(true)
```

## 4. Build the UI

Now, we'll create the frontend using the `@component` decorator. Vox components use a JSX-like syntax that compiles to high-performance React code.

```vox
# Skip-Test
@component fn TodoList() to Element:
    <div class="p-4 max-w-md mx-auto">
        <h1 class="text-2xl font-bold mb-4">"Vox Todos"</h1>
        <div class="flex gap-2 mb-4">
            <input type="text" placeholder="New task..." class="border p-2 flex-grow" />
            <button class="bg-blue-500 text-white p-2">"Add"</button>
        </div>
        <ul>
            <li class="flex items-center gap-2 py-2">
                <input type="checkbox" />
                <span>"Learn Vox Architecture"</span>
            </li>
        </ul>
    </div>
```

## 5. Wiring It Together

Finally, we map a route to our `TodoList` component.

```vox
routes:
    "/" to TodoList
```

## 6. Build and Run

Compile your app and start the development server:

```bash
vox build src/main.vox -o dist
vox run src/main.vox
```

Visit `http://localhost:3000` to see your collaborative todo list in action!

---

**Next Steps**:
- [Actor Basics](tut-actor-basics.md) — Add real-time collaboration with shared state.
- [Durable Workflows](tut-workflow-durability.md) — Automate task reminders.
