---
title: "Why Vox: Compiler-Verified AI Code"
description: "Why Python and TypeScript fail for LLM code generation, and how the Vox compiler solves the hallucination boundary."
category: "Concepts"
training_eligible: true

schema_type: "TechArticle"
keywords: ["AI code generation LLM", "LLM hallucination prevention", "compiler verified AI code", "type safe AI language"]
---

# Why Vox: Compiler-Verified AI Code

The primary barrier to AI-driven software engineering is not the model's intelligence, but the **hallucination boundary** of current languages.

## 1. The Python Problem
When an LLM generates Python code (FastAPI, SQLAlchemy, etc.), it is guessing across a massive, unconstrained state space:
- **Runtime Persistence**: Did it guess the correct column name?
- **Dependency Drift**: Is that library version actually installed?
- **Dynamic Typing**: Will this `None` propagate into a crash 5 minutes into execution?

In Python, the feedback loop is **runtime failure**. The model has to run the code, see the crash, and attempt a second guess. This is inefficient and risky for autonomous agents.

## 2. The Vox Solution: Compiler-Enforced Reality
Vox is designed so that the **compiler** acts as the guardrail for the LLM.

### `table`: The Database is the Source of Truth
In Vox, you don't write SQL strings or use a loose ORM. You define your schema with the bare `table` keyword (the older `@table` decorator form is a retired hard parse error).
```vox
table User {
    email: str
    points: int
}
```
If an LLM attempts to generate code that accesses `user.score` instead of `user.points`, the **Vox compiler fails immediately**. The model receives a precise type error: `Field 'score' not found on type 'User'`.

### Zero-Null Discipline
LLMs frequently forget to check for `null`. In Vox, `null` does not exist. You must handle `Option[T]` using `match`.
```vox
{{#include ../../../examples/golden/ref_types.vox:matching}}
```
If the LLM omits the `None` case, the compiler rejects the code for a **non-exhaustive match**. The model is forced to be correct.

## 3. Results: Practical Implications
By constraining the LLM's output to a strictly-typed, compiler-verified grammar:
- The compiler provides exact field-name errors rather than runtime stack traces, reducing the iteration cycle for LLM-driven code generation.
- **Lower K-Complexity**: A single `.vox` file replaces 10+ files of boilerplate across Rust and TypeScript.

---

**Next Steps**:
- [Language Reference](../reference/ref-syntax.md)
- [How-To: Build AI Agents](../how-to/how-to-ai-agents.md)
- [Installation](../reference/installation.md)


