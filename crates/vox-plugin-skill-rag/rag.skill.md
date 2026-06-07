---
name = "skill-rag"
description = "Multi-modal Visual Retrieval-Augmented Generation RAG handler orchestrating queries to connected intelligent backends."

[metadata]
"vox-id" = "vox.rag"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "research"
"vox-tools" = ["vox_visual_rag_query"]
"vox-tags" = ["rag", "visual", "vision", "image", "multimodal", "search"]
"vox-permissions" = []
---

# Vox Visual RAG

Use this tool when you need to answer complex visual reasoning questions or perform extraction strictly from image content in the workspace.

## Scope

The Visual RAG tool integrates with external multi-modal backend providers (e.g. `oratio-vlm-bridge`) via an MCP proxy. As an AI Agent, you **do not** interact directly with the external language model. You provide image paths or base64 streams directly to the `vox_visual_rag_query` tool, which wraps and brokers the request to ensure "Zero Syntactic Configurability" invariants are upheld.

## Tools

- **`vox_visual_rag_query`** — Dispatches a user query against provided image structures, and returns a verified contextual answer combined with external intelligence sources.

## Important Requirements

- You **must** provide at least one image path or base64 payload when calling the capability.
- Ensure the image files actually exist within your workspace sandbox.
- Do not use this tool for text-only searches; use standard `vox_repo_query_text` or `rg` for textual workspace context.
