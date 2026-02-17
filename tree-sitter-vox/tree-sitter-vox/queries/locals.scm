; Vox locals queries for tree-sitter

; ─── Scopes ───────────────────────────────────────────────
(function_declaration) @local.scope
(component_declaration) @local.scope
(actor_handler) @local.scope
(workflow_declaration) @local.scope
(test_declaration) @local.scope
(http_route) @local.scope
(lambda) @local.scope
(block) @local.scope

; ─── Definitions ──────────────────────────────────────────
(let_statement
  pattern: (identifier) @local.definition)

(parameter
  name: (identifier) @local.definition)

(for_expression
  binding: (identifier) @local.definition)

; ─── References ───────────────────────────────────────────
(identifier) @local.reference
