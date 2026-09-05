// Requires `postgres` and `mysql`: this test asserts cross-dialect
// conformance using `BackendKind::Postgres`/`BackendKind::MySql`, which
// vox-sql cfg-gates behind those features independently of `runtime` (see
// crates/vox-sql/Cargo.toml and THE TRAP note on `BackendKind`).
#![cfg(all(feature = "postgres", feature = "mysql"))]

use vox_sql::BackendKind;
use vox_sql::SqlDialect;
use vox_sql::build::{SqlPredicate, placeholder_sql, predicate_sql};
use vox_sql::type_map::{UnsupportedTypePolicy, vox_type_to_sql};

#[derive(Clone)]
struct Case {
    name: &'static str,
    predicate: SqlPredicate,
    expected_sqlite: &'static str,
    expected_postgres: &'static str,
    expected_mysql: &'static str,
}

fn conformance_cases() -> Vec<Case> {
    vec![
        Case {
            name: "nested-and-or",
            predicate: SqlPredicate::And(vec![
                SqlPredicate::Eq {
                    field: "age".to_string(),
                },
                SqlPredicate::Or(vec![
                    SqlPredicate::Lt {
                        field: "score".to_string(),
                    },
                    SqlPredicate::Gte {
                        field: "score".to_string(),
                    },
                ]),
            ]),
            expected_sqlite: "(age = ?1) AND ((score < ?2) OR (score >= ?3))",
            expected_postgres: "(age = $1) AND ((score < $2) OR (score >= $3))",
            expected_mysql: "(age = ?) AND ((score < ?) OR (score >= ?))",
        },
        Case {
            name: "contains-and-in",
            predicate: SqlPredicate::And(vec![
                SqlPredicate::Contains {
                    field: "title".to_string(),
                },
                SqlPredicate::In {
                    field: "id".to_string(),
                    arity: 3,
                },
            ]),
            expected_sqlite: "(title LIKE '%' || ?1 || '%') AND (id IN (?2, ?3, ?4))",
            expected_postgres: "(title LIKE '%' || $1 || '%') AND (id IN ($2, $3, $4))",
            expected_mysql: "(title LIKE CONCAT('%', ?, '%')) AND (id IN (?, ?, ?))",
        },
        Case {
            name: "not-is-null",
            predicate: SqlPredicate::Not(Box::new(SqlPredicate::IsNull {
                field: "deleted_at".to_string(),
            })),
            expected_sqlite: "NOT (deleted_at IS NULL)",
            expected_postgres: "NOT (deleted_at IS NULL)",
            expected_mysql: "NOT (deleted_at IS NULL)",
        },
        Case {
            name: "leaf-ops-sequence",
            predicate: SqlPredicate::And(vec![
                SqlPredicate::Neq {
                    field: "status".to_string(),
                },
                SqlPredicate::Lte {
                    field: "age".to_string(),
                },
                SqlPredicate::Gt {
                    field: "priority".to_string(),
                },
                SqlPredicate::In {
                    field: "team_id".to_string(),
                    arity: 2,
                },
            ]),
            expected_sqlite: "(status <> ?1) AND (age <= ?2) AND (priority > ?3) AND (team_id IN (?4, ?5))",
            expected_postgres: "(status <> $1) AND (age <= $2) AND (priority > $3) AND (team_id IN ($4, $5))",
            expected_mysql: "(status <> ?) AND (age <= ?) AND (priority > ?) AND (team_id IN (?, ?))",
        },
    ]
}

#[test]
fn predicate_builder_conformance_across_dialects() {
    for case in conformance_cases() {
        let mut p = 1usize;
        let got_sqlite = predicate_sql(&SqlDialect::sqlite(), &case.predicate, &mut p);
        assert_eq!(got_sqlite, case.expected_sqlite, "case {}", case.name);

        let mut p = 1usize;
        let got_postgres = predicate_sql(&SqlDialect::postgres(), &case.predicate, &mut p);
        assert_eq!(got_postgres, case.expected_postgres, "case {}", case.name);

        let mut p = 1usize;
        let got_mysql = predicate_sql(&SqlDialect::mysql(), &case.predicate, &mut p);
        assert_eq!(got_mysql, case.expected_mysql, "case {}", case.name);
    }
}

#[test]
fn placeholder_builder_conformance_across_dialects() {
    let sqlite = (1..=3)
        .map(|i| placeholder_sql(&SqlDialect::sqlite(), i))
        .collect::<Vec<_>>();
    assert_eq!(sqlite, vec!["?1", "?2", "?3"]);

    let postgres = (1..=3)
        .map(|i| placeholder_sql(&SqlDialect::postgres(), i))
        .collect::<Vec<_>>();
    assert_eq!(postgres, vec!["$1", "$2", "$3"]);

    let mysql = (1..=3)
        .map(|i| placeholder_sql(&SqlDialect::mysql(), i))
        .collect::<Vec<_>>();
    assert_eq!(mysql, vec!["?", "?", "?"]);
}

#[test]
fn type_mapping_conformance_across_backends() {
    let cases = vec![
        ("int", "INTEGER", "BIGINT", "BIGINT"),
        ("float", "REAL", "DOUBLE PRECISION", "DOUBLE"),
        ("bool", "INTEGER", "BOOLEAN", "TINYINT(1)"),
        ("id[User]", "INTEGER", "BIGINT", "BIGINT"),
        ("option[str]", "TEXT", "TEXT", "TEXT"),
    ];

    for (vox_ty, sqlite, postgres, mysql) in cases {
        assert_eq!(
            vox_type_to_sql(BackendKind::Libsql, vox_ty, UnsupportedTypePolicy::Reject)
                .expect("sqlite map"),
            sqlite,
            "sqlite mapping for {vox_ty}"
        );
        assert_eq!(
            vox_type_to_sql(BackendKind::Postgres, vox_ty, UnsupportedTypePolicy::Reject)
                .expect("postgres map"),
            postgres,
            "postgres mapping for {vox_ty}"
        );
        assert_eq!(
            vox_type_to_sql(BackendKind::MySql, vox_ty, UnsupportedTypePolicy::Reject)
                .expect("mysql map"),
            mysql,
            "mysql mapping for {vox_ty}"
        );
    }
}

#[test]
fn unsupported_type_policy_conformance_across_backends() {
    let ty = "legacy_payload";
    assert!(
        vox_type_to_sql(BackendKind::Libsql, ty, UnsupportedTypePolicy::Reject).is_err(),
        "reject policy must fail unknown types"
    );

    assert_eq!(
        vox_type_to_sql(BackendKind::Libsql, ty, UnsupportedTypePolicy::JsonText)
            .expect("sqlite json fallback"),
        "TEXT"
    );
    assert_eq!(
        vox_type_to_sql(BackendKind::Postgres, ty, UnsupportedTypePolicy::JsonText)
            .expect("postgres json fallback"),
        "TEXT"
    );
    assert_eq!(
        vox_type_to_sql(BackendKind::MySql, ty, UnsupportedTypePolicy::JsonText)
            .expect("mysql json fallback"),
        "LONGTEXT"
    );
}
