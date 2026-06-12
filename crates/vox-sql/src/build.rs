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
