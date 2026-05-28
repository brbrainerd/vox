---
name: writing-plans
description: Use when you have a spec or requirements for a multi-step task, before touching code
---

# Writing Plans (Vox Adaptation)

## Overview

Write comprehensive implementation plans assuming the executor has zero context for our codebase and questionable taste. Document everything they need to know: which files to touch for each task, code, testing, docs they might need to check, how to test it. Give them the whole plan as bite-sized tasks. DRY. YAGNI. TDD. Frequent commits.

**Announce at start:** "I'm using the writing-plans skill to create the implementation plan."

**Context:** The plan MUST be formatted to comply with the Vox Orchestrator `PLANNER_SYSTEM_PROMPT`. You will output the plan as a JSON array of `PlanNode` objects.

## Scope Check

If the spec covers multiple independent subsystems, break this into separate JSON PlanNodes — one per subsystem. Each plan node should produce working, testable software on its own.

## File Structure

Before defining tasks, map out which files will be created or modified and what each one is responsible for. This is where decomposition decisions get locked in.

- Design units with clear boundaries and well-defined interfaces. Each file should have one clear responsibility.
- Files that change together should live together. Split by responsibility, not by technical layer.
- Ensure every PlanNode has a populated `file_manifest`.

## Bite-Sized Task Granularity

**Each node is one action (2-5 minutes):**
- Step 1: Write the failing test (`cargo test`, `pytest`, etc.)
- Step 2: Implement minimal code.
- Step 3: Run the test to pass.
- Step 4: `vox stub-check` for TOESTUB compliance.

## JSON PlanNode Structure

Every task must map to a `PlanNode`.

**Example:**
```json
[
  {
    "id": 1,
    "title": "Task 1: Component Name",
    "description": "Implement the core function and its test.",
    "file_manifest": ["exact/path/to/file.rs", "tests/exact/path/test.rs"],
    "verification": "cargo test --package vox-example --test component_test",
    "depends_on": [],
    "active_skill": "test-driven-development"
  }
]
```

## No Placeholders

Every node must contain the actual content an engineer needs. These are **plan failures** — never write them:
- "TBD", "TODO", "implement later"
- "Add appropriate error handling" (specify exactly what to handle)
- References to types or methods not defined in any task.
- Violations of the VOX ARCHITECTURE RULES (e.g., direct `std::env::var` reads).

## Execution Handoff

After outputting the JSON plan, the Vox Orchestrator will automatically dispatch `subagent-driven-development` campaigns for each node.
