use crate::{PlaceholderStyle, SqlDialect};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqlPredicate {
    Eq { field: String },
    Neq { field: String },
    Lt { field: String },
    Lte { field: String },
    Gt { field: String },
    Gte { field: String },
    Contains { field: String },
    IsNull { field: String },
    In { field: String, arity: usize },
    And(Vec<SqlPredicate>),
    Or(Vec<SqlPredicate>),
    Not(Box<SqlPredicate>),
}

pub fn placeholder_sql(dialect: &SqlDialect, index: usize) -> String {
    match dialect.placeholder_style {
        PlaceholderStyle::QuestionMarkNumbered => format!("?{index}"),
        PlaceholderStyle::DollarNumbered => format!("${index}"),
        PlaceholderStyle::QuestionMark => "?".to_string(),
    }
}

pub fn equality_predicate_sql(dialect: &SqlDialect, field: &str, index: usize) -> String {
    format!("{field} = {}", placeholder_sql(dialect, index))
}

pub fn scalar_predicate_sql(
    dialect: &SqlDialect,
    field: &str,
    op_sql: &str,
    index: usize,
) -> String {
    format!("{field} {op_sql} {}", placeholder_sql(dialect, index))
}

pub fn contains_predicate_sql(dialect: &SqlDialect, field: &str, index: usize) -> String {
    let slot = placeholder_sql(dialect, index);
    if dialect.name == "mysql" {
        format!("{field} LIKE CONCAT('%', {slot}, '%')")
    } else {
        format!("{field} LIKE '%' || {slot} || '%'")
    }
}

pub fn predicate_sql(dialect: &SqlDialect, pred: &SqlPredicate, next_param: &mut usize) -> String {
    match pred {
        SqlPredicate::Eq { field } => next_scalar(dialect, field, "=", next_param),
        SqlPredicate::Neq { field } => next_scalar(dialect, field, "<>", next_param),
        SqlPredicate::Lt { field } => next_scalar(dialect, field, "<", next_param),
        SqlPredicate::Lte { field } => next_scalar(dialect, field, "<=", next_param),
        SqlPredicate::Gt { field } => next_scalar(dialect, field, ">", next_param),
        SqlPredicate::Gte { field } => next_scalar(dialect, field, ">=", next_param),
        SqlPredicate::Contains { field } => {
            let idx = *next_param;
            *next_param += 1;
            contains_predicate_sql(dialect, field, idx)
        }
        SqlPredicate::IsNull { field } => format!("{field} IS NULL"),
        SqlPredicate::In { field, arity } => {
            let mut slots = Vec::with_capacity(*arity);
            for _ in 0..*arity {
                let idx = *next_param;
                *next_param += 1;
                slots.push(placeholder_sql(dialect, idx));
            }
            format!("{field} IN ({})", slots.join(", "))
        }
        SqlPredicate::And(parts) => combine_parts(dialect, parts, " AND ", next_param),
        SqlPredicate::Or(parts) => combine_parts(dialect, parts, " OR ", next_param),
        SqlPredicate::Not(inner) => {
            let inner_sql = predicate_sql(dialect, inner, next_param);
            format!("NOT ({inner_sql})")
        }
    }
}

fn next_scalar(dialect: &SqlDialect, field: &str, op: &str, next_param: &mut usize) -> String {
    let idx = *next_param;
    *next_param += 1;
    scalar_predicate_sql(dialect, field, op, idx)
}

fn combine_parts(
    dialect: &SqlDialect,
    parts: &[SqlPredicate],
    sep: &str,
    next_param: &mut usize,
) -> String {
    parts
        .iter()
        .map(|p| format!("({})", predicate_sql(dialect, p, next_param)))
        .collect::<Vec<_>>()
        .join(sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_sql_uses_postgres_numbering() {
        let pred = SqlPredicate::And(vec![
            SqlPredicate::Eq {
                field: "name".to_string(),
            },
            SqlPredicate::Contains {
                field: "email".to_string(),
            },
        ]);
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(sql, "(name = $1) AND (email LIKE '%' || $2 || '%')");
        assert_eq!(next, 3);
    }

    #[test]
    fn predicate_sql_uses_mysql_concat_for_contains() {
        let pred = SqlPredicate::Contains {
            field: "title".to_string(),
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::mysql(), &pred, &mut next);
        assert_eq!(sql, "title LIKE CONCAT('%', ?, '%')");
        assert_eq!(next, 2);
    }

    #[test]
    fn predicate_sql_renders_in_arity_with_dialect_placeholders() {
        let pred = SqlPredicate::In {
            field: "id".to_string(),
            arity: 3,
        };
        let mut next = 2usize;
        let sql = predicate_sql(&SqlDialect::sqlite(), &pred, &mut next);
        assert_eq!(sql, "id IN (?2, ?3, ?4)");
        assert_eq!(next, 5);
    }
}

#[cfg(test)]
mod semcov_wave39_tests {
    use super::*;
    use crate::{PlaceholderStyle, SqlDialect};

    // --- placeholder_sql ---

    #[test]
    fn placeholder_sql_postgres_starts_at_zero() {
        // Catches: caller passes index=0 and gets "$0" which Postgres rejects (1-indexed)
        let sql = placeholder_sql(&SqlDialect::postgres(), 0);
        assert_eq!(
            sql, "$0",
            "placeholder_sql must emit exactly what caller provides; callers must start at 1"
        );
    }

    #[test]
    fn placeholder_sql_sqlite_numbered_not_positional() {
        // Catches: confusing ?1 (SQLite ordinal) with ? (positional MySQL) — wrong dialect routing
        let sql = placeholder_sql(&SqlDialect::sqlite(), 3);
        assert_eq!(sql, "?3");
        // must NOT be bare "?"
        assert_ne!(sql, "?");
    }

    #[test]
    fn placeholder_sql_mysql_ignores_index() {
        // Catches: MySQL variant accidentally emitting "?1" instead of bare "?"
        let sql0 = placeholder_sql(&SqlDialect::mysql(), 1);
        let sql9 = placeholder_sql(&SqlDialect::mysql(), 99);
        assert_eq!(sql0, "?");
        assert_eq!(sql9, "?");
    }

    // --- predicate_sql counter advancement ---

    #[test]
    fn param_counter_advances_correctly_through_and() {
        // Catches: counter not incremented for each child in And — duplicate placeholder indices
        let pred = SqlPredicate::And(vec![
            SqlPredicate::Eq {
                field: "a".to_string(),
            },
            SqlPredicate::Eq {
                field: "b".to_string(),
            },
            SqlPredicate::Eq {
                field: "c".to_string(),
            },
        ]);
        let mut next = 1usize;
        predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(next, 4, "three Eq predicates must advance counter by 3");
    }

    #[test]
    fn param_counter_advances_correctly_through_or() {
        // Catches: Or sharing the same counter slot as And — counter reset on branch
        let pred = SqlPredicate::Or(vec![
            SqlPredicate::Lt {
                field: "x".to_string(),
            },
            SqlPredicate::Gt {
                field: "y".to_string(),
            },
        ]);
        let mut next = 5usize;
        predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(next, 7);
    }

    #[test]
    fn in_arity_zero_produces_empty_in_list() {
        // Catches: arity=0 panicking (empty Vec) or producing "IN ()" which is invalid SQL on some backends
        let pred = SqlPredicate::In {
            field: "id".to_string(),
            arity: 0,
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(sql, "id IN ()");
        assert_eq!(next, 1, "zero-arity IN must not advance counter");
    }

    #[test]
    fn in_arity_one_emits_single_placeholder_no_trailing_comma() {
        // Catches: join separator leaking a trailing ", " for single-element IN
        let pred = SqlPredicate::In {
            field: "uid".to_string(),
            arity: 1,
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(sql, "uid IN ($1)");
        assert!(!sql.contains(", )"), "must not have trailing comma: {sql}");
    }

    #[test]
    fn not_wraps_inner_sql_in_parentheses() {
        // Catches: NOT emitted without wrapping parens, breaking precedence for compound inners
        let pred = SqlPredicate::Not(Box::new(SqlPredicate::Or(vec![
            SqlPredicate::Eq {
                field: "deleted".to_string(),
            },
            SqlPredicate::IsNull {
                field: "deleted".to_string(),
            },
        ])));
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert!(
            sql.starts_with("NOT ("),
            "NOT must wrap inner in parens: {sql}"
        );
        assert!(sql.ends_with(')'), "NOT wrapper must close: {sql}");
    }

    #[test]
    fn is_null_does_not_consume_param_slot() {
        // Catches: IsNull accidentally incrementing next_param, off-by-one in subsequent predicates
        let pred = SqlPredicate::And(vec![
            SqlPredicate::IsNull {
                field: "archived_at".to_string(),
            },
            SqlPredicate::Eq {
                field: "status".to_string(),
            },
        ]);
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert!(
            sql.contains("archived_at IS NULL"),
            "IsNull must emit IS NULL: {sql}"
        );
        assert!(
            sql.contains("$1"),
            "Eq after IsNull must get slot 1, not 2: {sql}"
        );
        assert_eq!(next, 2);
    }

    #[test]
    fn neq_predicate_emits_angle_brackets() {
        // Catches: Neq accidentally emitting != instead of <> (standard SQL)
        let pred = SqlPredicate::Neq {
            field: "role".to_string(),
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert!(sql.contains("<>"), "Neq must use <> not !=: {sql}");
    }

    #[test]
    fn contains_postgres_uses_concat_operator_not_concat_fn() {
        // Catches: contains_predicate_sql routing mysql CONCAT to postgres accidentally
        let pred = SqlPredicate::Contains {
            field: "bio".to_string(),
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert!(
            sql.contains("|| $1 ||"),
            "postgres LIKE must use || concat: {sql}"
        );
        assert!(
            !sql.contains("CONCAT("),
            "postgres must NOT use CONCAT(): {sql}"
        );
    }

    #[test]
    fn contains_mysql_uses_concat_fn_not_operator() {
        // Catches: mysql accidentally using || which requires PIPES_AS_CONCAT sql_mode
        let pred = SqlPredicate::Contains {
            field: "name".to_string(),
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::mysql(), &pred, &mut next);
        assert!(sql.contains("CONCAT("), "mysql must use CONCAT(): {sql}");
        assert!(!sql.contains("||"), "mysql must NOT use || operator: {sql}");
    }

    #[test]
    fn nested_not_and_advances_counter_exactly_once() {
        // Catches: double-counting inside NOT(And([single_Eq])) — counter ends at 2 not 1
        let pred = SqlPredicate::Not(Box::new(SqlPredicate::And(vec![SqlPredicate::Eq {
            field: "f".to_string(),
        }])));
        let mut next = 1usize;
        predicate_sql(&SqlDialect::sqlite(), &pred, &mut next);
        assert_eq!(next, 2);
    }

    #[test]
    fn field_names_with_spaces_pass_through_unquoted() {
        // Catches: build functions silently accepting injection-enabling field names;
        // documents that callers must quote identifiers themselves (no auto-quoting here)
        let pred = SqlPredicate::Eq {
            field: "a b".to_string(),
        };
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        // We assert the output is "a b = $1" — i.e., the crate does NOT auto-quote.
        // If this test ever fails because auto-quoting was added, update the assertion.
        assert_eq!(sql, "a b = $1");
    }

    #[test]
    fn equality_predicate_sql_uses_correct_placeholder_style() {
        // Catches: equality_predicate_sql hard-coding "?" regardless of dialect
        let pg = equality_predicate_sql(&SqlDialect::postgres(), "email", 7);
        assert_eq!(pg, "email = $7");
        let my = equality_predicate_sql(&SqlDialect::mysql(), "email", 7);
        assert_eq!(my, "email = ?");
        let sq = equality_predicate_sql(&SqlDialect::sqlite(), "email", 7);
        assert_eq!(sq, "email = ?7");
    }

    #[test]
    fn gte_lte_emit_correct_operators() {
        // Catches: Gte/Lte swapped — emitting <= for >= and vice versa
        let mut n = 1usize;
        let gte = predicate_sql(
            &SqlDialect::postgres(),
            &SqlPredicate::Gte {
                field: "score".to_string(),
            },
            &mut n,
        );
        assert!(gte.contains(">="), "Gte must emit >=: {gte}");
        let mut n = 1usize;
        let lte = predicate_sql(
            &SqlDialect::postgres(),
            &SqlPredicate::Lte {
                field: "score".to_string(),
            },
            &mut n,
        );
        assert!(lte.contains("<="), "Lte must emit <=: {lte}");
    }

    #[test]
    fn deeply_nested_and_or_placeholder_sequence_is_monotone() {
        // Catches: counter reverting or forking when deeply nested And/Or recurse
        let pred = SqlPredicate::And(vec![
            SqlPredicate::Or(vec![
                SqlPredicate::Eq {
                    field: "a".to_string(),
                },
                SqlPredicate::Eq {
                    field: "b".to_string(),
                },
            ]),
            SqlPredicate::And(vec![
                SqlPredicate::Eq {
                    field: "c".to_string(),
                },
                SqlPredicate::Eq {
                    field: "d".to_string(),
                },
            ]),
        ]);
        let mut next = 1usize;
        let sql = predicate_sql(&SqlDialect::postgres(), &pred, &mut next);
        assert_eq!(next, 5, "four leaf Eq predicates must yield next=5");
        // placeholders must appear in order $1..$4
        for i in 1usize..=4 {
            assert!(sql.contains(&format!("${i}")), "missing ${i} in: {sql}");
        }
    }
}
