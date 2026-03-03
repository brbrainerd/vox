# Glossary: Vox Terminology

A centralized registry of technical terms and concepts used within the Vox project.

## A
- **Actor**: A stateful unit of concurrency that communicates via asynchronous message passing.
- **Activity**: A retryable, idempotent step within a Vox workflow.
- **ADTs (Algebraic Data Types)**: Types that can represent several distinct variants (e.g., enums in Rust).

## C
- **Checkpointing**: The process of saving workflow state to persistent storage to allow recovery after a crash.

## D
- **Durable Execution**: A system guarantee that a process will eventually complete despite hardware or software failures.
- **Diátaxis**: A framework for structuring technical documentation into four distinct pillars (Tutorials, How-To, Explanation, Reference).

## H
- **HIR (High-level Intermediate Representation)**: A compiler representation that includes symbol resolution and type information.

## J
- **Journal**: An append-only log of all operations performed within a durable workflow.

## L
- **Lowering**: The process of transforming code from a higher-level representation to a lower-level one (e.g., AST -> HIR).
- **LIR (Low-level Intermediate Representation)**: A target-specific representation optimized for final code generation.

## M
- **MCP (Model Context Protocol)**: An open protocol that enables AI models to interact with local tools and data.

## R
- **Reduction Budget**: A fairness mechanism in the scheduler that limits how long a single process can run before yielding.
- **Replay**: The process of recreating an actor or workflow's state by re-executing its journaled operations.

## T
- **Technical Unification**: The philosophy of bridging the gap between frontend, backend, and data layers using a single language and toolchain.
- **TOESTUB**: An architectural enforcement standard for detecting AI-coding anti-patterns.
