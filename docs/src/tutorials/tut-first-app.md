---
title: "Tutorial: Building a Collaborative Task List"
description: "Build a full-stack Task app end to end with Vox."
category: "Tutorials"
status: "current"
sort_order: 2
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---
# Tutorial: Building a Collaborative Task List

Learn how to build a full-stack, collaborative task list app with Vox. This tutorial covers data modeling, server-side logic, and UI integration using a single `.vox` file.

## 1. Project Initialization

Create a new directory and initialize a Vox application:

```bash
mkdir vox-task-list
cd vox-task-list
vox init --kind application
```

## 2. Define the Data Model

Open `src/main.vox`. We'll start by defining what a "Task" is. Using the `@table` decorator, we create a persistent database table.

```vox
{{#include ../../../examples/golden/getting_started.vox:data_model}}
```

## 3. Implement Server Logic

Next, we add `@endpoint(kind: mutation)` and `@endpoint(kind: query)` functions to interact with the database.

```vox
{{#include ../../../examples/golden/getting_started.vox:logic}}
```

## 4. Build the UI

Now, we'll create the frontend using a `component` declaration. Vox components use a JSX-like syntax and compile to plain React/TSX components consumed by the external frontend.

```vox
{{#include ../../../examples/golden/getting_started.vox:ui}}
```

## 5. Wiring It Together

Finally, we map a route to our `TaskList` component.

```vox
// vox:skip
routes {
    "/" -> TaskList
}
```

## 6. Build and Run

Compile your app and start the development server:

```bash
vox check src/main.vox
vox build src/main.vox
vox run src/main.vox
```

Visit `http://localhost:3000` to see your collaborative task list in action!

---

**Next Steps**:
- [Actor Basics](tut-actor-basics.md) — Add real-time collaboration with shared state.
- [Durable Workflows](tut-workflow-durability.md) — Automate task reminders.
