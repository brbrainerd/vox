// Tree-sitter grammar for the Vox programming language
// Vox is an AI-native, indentation-sensitive, full-stack language
// with first-class actors, ADTs, JSX, HTTP routes, and workflows.

module.exports = grammar({
  name: 'vox',

  extras: $ => [/[ \t]/, $.comment],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.call_expression, $.primary_expression],
    [$.method_call_expression, $.field_access_expression],
  ],

  rules: {
    // ─── Top-level ─────────────────────────────────────────────
    source_file: $ => repeat(seq(optional($._newline), $._declaration)),

    _declaration: $ => choice(
      $.import_declaration,
      $.type_declaration,
      $.function_declaration,
      $.component_declaration,
      $.http_route,
      $.actor_declaration,
      $.workflow_declaration,
      $.activity_declaration,
      $.test_declaration,
    ),

    // ─── Activities ────────────────────────────────────────────
    activity_declaration: $ => seq(
      'activity',
      field('name', $.identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── Imports ───────────────────────────────────────────────
    import_declaration: $ => seq(
      'import',
      field('path', $.module_path),
      $._newline,
    ),

    module_path: $ => seq(
      $.identifier,
      repeat(seq('.', $.identifier)),
    ),

    // ─── Type declarations (ADTs) ─────────────────────────────
    type_declaration: $ => seq(
      optional('pub'),
      'type',
      field('name', $.type_identifier),
      '=',
      $._newline,
      repeat1($.variant),
    ),

    variant: $ => seq(
      '|',
      field('name', $.type_identifier),
      optional($.variant_fields),
      $._newline,
    ),

    variant_fields: $ => seq(
      '(',
      commaSep1($.typed_field),
      ')',
    ),

    typed_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $.type_expression),
    ),

    // ─── Functions ─────────────────────────────────────────────
    function_declaration: $ => seq(
      optional('pub'),
      'fn',
      field('name', $.identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── Components (React-like) ──────────────────────────────
    component_declaration: $ => seq(
      '@component',
      'fn',
      field('name', $.type_identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── HTTP Routes ──────────────────────────────────────────
    http_route: $ => seq(
      'http',
      field('method', $.http_method),
      field('path', $.string),
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    http_method: $ => choice('get', 'post', 'put', 'delete'),

    // ─── Actors ────────────────────────────────────────────────
    actor_declaration: $ => seq(
      'actor',
      field('name', $.type_identifier),
      ':',
      $._newline,
      repeat1($.actor_handler),
    ),

    actor_handler: $ => seq(
      'on',
      field('event', $.identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── Workflows ─────────────────────────────────────────────
    workflow_declaration: $ => seq(
      'workflow',
      field('name', $.identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── Tests ─────────────────────────────────────────────────
    test_declaration: $ => seq(
      '@test',
      'fn',
      field('name', $.identifier),
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      ':',
      $._newline,
      $.block,
    ),

    // ─── Parameters ────────────────────────────────────────────
    parameters: $ => commaSep1($.parameter),

    parameter: $ => seq(
      field('name', $.identifier),
      optional(seq(':', field('type', $.type_expression))),
      optional(seq('=', field('default', $._expression))),
    ),

    // ─── Types ─────────────────────────────────────────────────
    type_expression: $ => choice(
      $.type_identifier,
      $.generic_type,
      $.function_type,
    ),

    generic_type: $ => seq(
      $.type_identifier,
      '<',
      commaSep1($.type_expression),
      '>',
    ),

    function_type: $ => seq(
      '(',
      optional(commaSep1($.type_expression)),
      ')',
      'to',
      $.type_expression,
    ),

    // ─── Statements ────────────────────────────────────────────
    block: $ => repeat1($._statement),

    _statement: $ => choice(
      $.let_statement,
      $.assignment_statement,
      $.return_statement,
      $.expression_statement,
    ),

    let_statement: $ => seq(
      'let',
      optional('mut'),
      field('pattern', $._pattern),
      optional(seq(':', field('type', $.type_expression))),
      '=',
      field('value', $._expression),
      $._newline,
    ),

    assignment_statement: $ => seq(
      field('target', $._expression),
      '=',
      field('value', $._expression),
      $._newline,
    ),

    return_statement: $ => seq(
      'ret',
      optional(field('value', $._expression)),
      $._newline,
    ),

    expression_statement: $ => seq(
      $._expression,
      $._newline,
    ),

    // ─── Patterns ──────────────────────────────────────────────
    _pattern: $ => choice(
      $.identifier,
      $.tuple_pattern,
      $.constructor_pattern,
      $.wildcard,
    ),

    tuple_pattern: $ => seq(
      '(',
      commaSep1($._pattern),
      ')',
    ),

    constructor_pattern: $ => seq(
      $.type_identifier,
      optional(seq('(', commaSep1($._pattern), ')')),
    ),

    wildcard: $ => '_',

    // ─── Expressions ───────────────────────────────────────────
    _expression: $ => choice(
      $.binary_expression,
      $.unary_expression,
      $.pipe_expression,
      $.call_expression,
      $.method_call_expression,
      $.field_access_expression,
      $.if_expression,
      $.match_expression,
      $.for_expression,
      $.lambda,
      $.spawn_expression,
      $.with_expression,
      $.jsx_element,
      $.jsx_self_closing,
      $.primary_expression,
    ),

    with_expression: $ => prec.left(5, seq(
      field('operand', $._expression),
      'with',
      field('options', $._expression),
    )),

    primary_expression: $ => choice(
      $.integer,
      $.float,
      $.string,
      $.boolean,
      $.identifier,
      $.type_identifier,
      $.list_literal,
      $.object_literal,
      $.parenthesized_expression,
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    binary_expression: $ => choice(
      ...[
        ['+', 6],
        ['-', 6],
        ['*', 7],
        ['/', 7],
        ['<', 4],
        ['>', 4],
        ['<=', 4],
        ['>=', 4],
        ['is', 3],
        ['isnt', 3],
        ['and', 2],
        ['or', 1],
      ].map(([op, precedence]) =>
        prec.left(precedence, seq(
          field('left', $._expression),
          field('operator', op),
          field('right', $._expression),
        ))
      ),
    ),

    unary_expression: $ => prec(8, choice(
      seq('not', field('operand', $._expression)),
      seq('-', field('operand', $._expression)),
    )),

    pipe_expression: $ => prec.left(0, seq(
      field('left', $._expression),
      '|>',
      field('right', $._expression),
    )),

    call_expression: $ => prec(9, seq(
      field('function', $._expression),
      '(',
      optional($.arguments),
      ')',
    )),

    arguments: $ => commaSep1($.argument),

    argument: $ => choice(
      seq(field('name', $.identifier), ':', field('value', $._expression)),
      field('value', $._expression),
    ),

    method_call_expression: $ => prec(10, seq(
      field('object', $._expression),
      '.',
      field('method', $.identifier),
      '(',
      optional($.arguments),
      ')',
    )),

    field_access_expression: $ => prec(10, seq(
      field('object', $._expression),
      '.',
      field('field', $.identifier),
    )),

    if_expression: $ => seq(
      'if',
      field('condition', $._expression),
      ':',
      $._newline,
      field('then', $.block),
      optional(seq('else', ':', $._newline, field('else', $.block))),
    ),

    match_expression: $ => seq(
      'match',
      field('value', $._expression),
      ':',
      $._newline,
      repeat1($.match_arm),
    ),

    match_arm: $ => seq(
      field('pattern', $._pattern),
      optional(seq('if', field('guard', $._expression))),
      '->',
      field('body', $._expression),
      $._newline,
    ),

    for_expression: $ => seq(
      'for',
      field('binding', $.identifier),
      'in',
      field('iterable', $._expression),
      ':',
      $._newline,
      field('body', $.block),
    ),

    lambda: $ => seq(
      'fn',
      '(',
      optional($.parameters),
      ')',
      optional(seq('to', field('return_type', $.type_expression))),
      field('body', $._expression),
    ),

    spawn_expression: $ => seq(
      'spawn',
      '(',
      field('actor', $._expression),
      ')',
    ),

    // ─── JSX ───────────────────────────────────────────────────
    jsx_element: $ => seq(
      '<',
      field('tag', $.identifier),
      repeat($.jsx_attribute),
      '>',
      repeat($.jsx_child),
      '</',
      field('closing_tag', $.identifier),
      '>',
    ),

    jsx_self_closing: $ => seq(
      '<',
      field('tag', $.identifier),
      repeat($.jsx_attribute),
      '/>',
    ),

    jsx_attribute: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $.jsx_attribute_value),
    ),

    jsx_attribute_value: $ => choice(
      $.string,
      seq('{', $._expression, '}'),
    ),

    jsx_child: $ => choice(
      $.jsx_element,
      $.jsx_self_closing,
      seq('{', $._expression, '}'),
      $.jsx_text,
    ),

    jsx_text: $ => /[^<>{}\n]+/,

    // ─── Literals ──────────────────────────────────────────────
    list_literal: $ => seq(
      '[',
      optional(commaSep1($._expression)),
      ']',
    ),

    object_literal: $ => seq(
      '{',
      commaSep1($.object_field),
      '}',
    ),

    object_field: $ => seq(
      field('key', $.identifier),
      ':',
      field('value', $._expression),
    ),

    // ─── Terminals ─────────────────────────────────────────────
    identifier: $ => /[a-z_][a-zA-Z0-9_]*/,
    type_identifier: $ => /[A-Z][a-zA-Z0-9_]*/,
    integer: $ => /[0-9]+/,
    float: $ => /[0-9]+\.[0-9]+/,
    string: $ => choice(
      seq('"', /([^"\\]|\\.)*/, '"'),
      seq("'", /([^'\\]|\\.)*/, "'"),
    ),
    boolean: $ => choice('true', 'false'),
    comment: $ => /#[^\r\n]*/,
    _newline: $ => /\r?\n/,
  },
});

function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)), optional(','));
}
