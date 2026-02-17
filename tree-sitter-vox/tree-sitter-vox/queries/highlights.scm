; Vox syntax highlighting queries for tree-sitter

; ─── Keywords ─────────────────────────────────────────────
[
  "fn"
  "let"
  "mut"
  "if"
  "else"
  "match"
  "for"
  "in"
  "to"
  "ret"
  "import"
  "type"
  "pub"
  "with"
  "on"
  "actor"
  "workflow"
  "spawn"
  "http"
] @keyword

; ─── Decorators ───────────────────────────────────────────
[
  "@component"
  "@test"
  "@mcp.tool"
  "@external"
] @attribute

; ─── HTTP Methods ─────────────────────────────────────────
(http_method) @keyword.function

; ─── Operators ────────────────────────────────────────────
[
  "and"
  "or"
  "not"
] @keyword.operator

[
  "is"
  "isnt"
] @keyword.operator

[
  "+"
  "-"
  "*"
  "/"
  "<"
  ">"
  "<="
  ">="
  "|>"
  "->"
  "="
] @operator

; ─── Booleans ─────────────────────────────────────────────
(boolean) @constant.builtin

; ─── Functions ────────────────────────────────────────────
(function_declaration
  name: (identifier) @function)

(component_declaration
  name: (type_identifier) @function)

(test_declaration
  name: (identifier) @function)

(workflow_declaration
  name: (identifier) @function)

(actor_handler
  event: (identifier) @function.method)

(call_expression
  function: (identifier) @function.call)

(method_call_expression
  method: (identifier) @function.method)

; ─── Types ────────────────────────────────────────────────
(type_declaration
  name: (type_identifier) @type.definition)

(type_expression
  (type_identifier) @type)

(type_identifier) @type

; ─── Actors ───────────────────────────────────────────────
(actor_declaration
  name: (type_identifier) @type.definition)

; ─── Variables ────────────────────────────────────────────
(let_statement
  pattern: (identifier) @variable)

(parameter
  name: (identifier) @variable.parameter)

(field_access_expression
  field: (identifier) @property)

; ─── Patterns ─────────────────────────────────────────────
(constructor_pattern
  (type_identifier) @constructor)

(variant
  name: (type_identifier) @constructor)

(wildcard) @variable.builtin

; ─── Literals ─────────────────────────────────────────────
(integer) @number
(float) @number.float
(string) @string

; ─── Comments ─────────────────────────────────────────────
(comment) @comment

; ─── JSX ──────────────────────────────────────────────────
(jsx_element
  tag: (identifier) @tag)

(jsx_element
  closing_tag: (identifier) @tag)

(jsx_self_closing
  tag: (identifier) @tag)

(jsx_attribute
  name: (identifier) @tag.attribute)

(jsx_text) @string.special

; ─── Imports ──────────────────────────────────────────────
(import_declaration
  path: (module_path) @module)

; ─── Punctuation ──────────────────────────────────────────
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "," "."] @punctuation.delimiter
["<" ">" "</" "/>"] @tag.delimiter
