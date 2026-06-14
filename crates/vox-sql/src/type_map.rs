use crate::BackendKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedTypePolicy {
    Reject,
    JsonText,
    OpaqueBlob,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedTypeError {
    pub vox_type: String,
}

impl std::fmt::Display for UnsupportedTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported Vox type for SQL mapping: {}", self.vox_type)
    }
}

impl std::error::Error for UnsupportedTypeError {}

pub fn vox_type_to_sql(
    backend: BackendKind,
    vox_type: &str,
    policy: UnsupportedTypePolicy,
) -> Result<String, UnsupportedTypeError> {
    let ty = normalize_vox_type(vox_type);
    let mapped = match ty.as_str() {
        "str" => Some(sql_for_scalar(backend, "text")),
        "int" => Some(sql_for_scalar(backend, "int")),
        "float" => Some(sql_for_scalar(backend, "float")),
        "bool" => Some(sql_for_scalar(backend, "bool")),
        "bytes" => Some(sql_for_scalar(backend, "bytes")),
        "decimal" | "dec" => Some(sql_for_scalar(backend, "decimal")),
        _ if ty.starts_with("id[") => Some(sql_for_scalar(backend, "id")),
        _ if ty.starts_with("option[") => {
            let inner = ty
                .trim_start_matches("option[")
                .trim_end_matches(']')
                .to_string();
            return vox_type_to_sql(backend, &inner, policy);
        }
        _ => None,
    };

    if let Some(sql) = mapped {
        return Ok(sql.to_string());
    }

    match policy {
        UnsupportedTypePolicy::Reject => Err(UnsupportedTypeError {
            vox_type: vox_type.to_string(),
        }),
        UnsupportedTypePolicy::JsonText => Ok(match backend {
            BackendKind::Libsql => "TEXT".to_string(),
            BackendKind::Postgres => "TEXT".to_string(),
            BackendKind::MySql => "LONGTEXT".to_string(),
        }),
        UnsupportedTypePolicy::OpaqueBlob => Ok(match backend {
            BackendKind::Libsql => "BLOB".to_string(),
            BackendKind::Postgres => "BYTEA".to_string(),
            BackendKind::MySql => "LONGBLOB".to_string(),
        }),
    }
}

fn normalize_vox_type(ty: &str) -> String {
    ty.trim().to_ascii_lowercase().replace(' ', "")
}

fn sql_for_scalar(backend: BackendKind, scalar: &str) -> &'static str {
    match (backend, scalar) {
        (BackendKind::Libsql, "text") => "TEXT",
        (BackendKind::Postgres, "text") => "TEXT",
        (BackendKind::MySql, "text") => "TEXT",
        (BackendKind::Libsql, "int") => "INTEGER",
        (BackendKind::Postgres, "int") => "BIGINT",
        (BackendKind::MySql, "int") => "BIGINT",
        (BackendKind::Libsql, "float") => "REAL",
        (BackendKind::Postgres, "float") => "DOUBLE PRECISION",
        (BackendKind::MySql, "float") => "DOUBLE",
        (BackendKind::Libsql, "bool") => "INTEGER",
        (BackendKind::Postgres, "bool") => "BOOLEAN",
        (BackendKind::MySql, "bool") => "TINYINT(1)",
        (BackendKind::Libsql, "bytes") => "BLOB",
        (BackendKind::Postgres, "bytes") => "BYTEA",
        (BackendKind::MySql, "bytes") => "BLOB",
        (BackendKind::Libsql, "decimal") => "TEXT",
        (BackendKind::Postgres, "decimal") => "NUMERIC",
        (BackendKind::MySql, "decimal") => "DECIMAL(65,30)",
        (BackendKind::Libsql, "id") => "INTEGER",
        (BackendKind::Postgres, "id") => "BIGINT",
        (BackendKind::MySql, "id") => "BIGINT",
        _ => "TEXT",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_types_per_backend() {
        assert_eq!(
            vox_type_to_sql(BackendKind::Postgres, "bool", UnsupportedTypePolicy::Reject)
                .expect("map bool"),
            "BOOLEAN"
        );
        assert_eq!(
            vox_type_to_sql(BackendKind::MySql, "int", UnsupportedTypePolicy::Reject)
                .expect("map int"),
            "BIGINT"
        );
        assert_eq!(
            vox_type_to_sql(
                BackendKind::Libsql,
                "Option[str]",
                UnsupportedTypePolicy::Reject
            )
            .expect("map option"),
            "TEXT"
        );
    }

    #[test]
    fn unsupported_policy_rejects_or_falls_back() {
        assert!(
            vox_type_to_sql(
                BackendKind::Postgres,
                "TaskPayload",
                UnsupportedTypePolicy::Reject
            )
            .is_err()
        );

        assert_eq!(
            vox_type_to_sql(
                BackendKind::MySql,
                "TaskPayload",
                UnsupportedTypePolicy::JsonText
            )
            .expect("json fallback"),
            "LONGTEXT"
        );
    }
}

#[cfg(test)]
mod semcov_wave39_tests {
    use super::*;
    use crate::BackendKind;

    // --- normalize_vox_type whitespace/case ---

    #[test]
    fn type_with_surrounding_whitespace_maps_correctly() {
        // Catches: normalize_vox_type not trimming; " str " fails to hit "str" arm → spurious reject
        let result = vox_type_to_sql(
            BackendKind::Postgres,
            " str ",
            UnsupportedTypePolicy::Reject,
        );
        assert_eq!(result.expect("should map trimmed str"), "TEXT");
    }

    #[test]
    fn type_name_uppercase_str_maps_via_normalization() {
        // Catches: case-sensitive match — "STR" not matching "str" arm
        let result = vox_type_to_sql(BackendKind::Postgres, "STR", UnsupportedTypePolicy::Reject);
        assert_eq!(result.expect("STR should normalize to str"), "TEXT");
    }

    #[test]
    fn option_wrapper_strips_correctly_for_all_backends() {
        // Catches: option[ ] stripping only working on one backend; inner type normalization broken
        for backend in [
            BackendKind::Libsql,
            BackendKind::Postgres,
            BackendKind::MySql,
        ] {
            let result = vox_type_to_sql(backend, "option[int]", UnsupportedTypePolicy::Reject)
                .unwrap_or_else(|_| panic!("option[int] should unwrap for {backend:?}"));
            // int maps to INTEGER (libsql) or BIGINT (pg/mysql)
            assert!(
                result == "INTEGER" || result == "BIGINT",
                "option[int] → unexpected {result} for {backend:?}"
            );
        }
    }

    #[test]
    fn option_uppercase_wrapper_also_unwraps() {
        // Catches: option stripping only handled lowercase; "Option[str]" fails
        let result = vox_type_to_sql(
            BackendKind::Libsql,
            "Option[str]",
            UnsupportedTypePolicy::Reject,
        );
        assert_eq!(result.expect("Option[str] should unwrap"), "TEXT");
    }

    #[test]
    fn dec_alias_maps_same_as_decimal() {
        // Catches: only one of "decimal"/"dec" in the match arm; the other falls through to unsupported
        let dec = vox_type_to_sql(BackendKind::Postgres, "dec", UnsupportedTypePolicy::Reject)
            .expect("dec alias should map");
        let decimal = vox_type_to_sql(
            BackendKind::Postgres,
            "decimal",
            UnsupportedTypePolicy::Reject,
        )
        .expect("decimal should map");
        assert_eq!(dec, decimal, "dec and decimal must map identically");
    }

    #[test]
    fn id_bracket_prefix_maps_on_all_backends() {
        // Catches: id[ ] prefix match missing for MySql or Libsql
        for (backend, expected) in [
            (BackendKind::Libsql, "INTEGER"),
            (BackendKind::Postgres, "BIGINT"),
            (BackendKind::MySql, "BIGINT"),
        ] {
            let result = vox_type_to_sql(backend, "id[User]", UnsupportedTypePolicy::Reject)
                .unwrap_or_else(|_| panic!("id[User] should map for {backend:?}"));
            assert_eq!(result, expected, "{backend:?}");
        }
    }

    #[test]
    fn opaque_blob_policy_returns_backend_specific_blob_type() {
        // Catches: OpaqueBlob falling through to JsonText or returning uniform "BLOB" regardless of backend
        assert_eq!(
            vox_type_to_sql(
                BackendKind::Postgres,
                "Unknown",
                UnsupportedTypePolicy::OpaqueBlob
            )
            .expect("opaque blob postgres"),
            "BYTEA"
        );
        assert_eq!(
            vox_type_to_sql(
                BackendKind::MySql,
                "Unknown",
                UnsupportedTypePolicy::OpaqueBlob
            )
            .expect("opaque blob mysql"),
            "LONGBLOB"
        );
        assert_eq!(
            vox_type_to_sql(
                BackendKind::Libsql,
                "Unknown",
                UnsupportedTypePolicy::OpaqueBlob
            )
            .expect("opaque blob libsql"),
            "BLOB"
        );
    }

    #[test]
    fn json_text_policy_returns_longtext_for_mysql_not_text() {
        // Catches: JsonText returning plain "TEXT" for MySQL (breaks large JSON docs >64KB)
        let result = vox_type_to_sql(
            BackendKind::MySql,
            "CustomRecord",
            UnsupportedTypePolicy::JsonText,
        )
        .expect("json text mysql");
        assert_eq!(
            result, "LONGTEXT",
            "MySQL JsonText must use LONGTEXT not TEXT: {result}"
        );
    }

    #[test]
    fn reject_policy_error_message_contains_original_type_name() {
        // Catches: error message stripping or mangling the type name, making diagnostics useless
        let err = vox_type_to_sql(
            BackendKind::Postgres,
            "MyCustomType",
            UnsupportedTypePolicy::Reject,
        )
        .unwrap_err();
        assert!(
            err.vox_type.contains("MyCustomType"),
            "error must preserve original type name, got: {:?}",
            err.vox_type
        );
    }

    #[test]
    fn float_maps_to_double_precision_on_postgres_not_float() {
        // Catches: postgres float mapped to FLOAT instead of DOUBLE PRECISION (loses mantissa bits)
        let result = vox_type_to_sql(
            BackendKind::Postgres,
            "float",
            UnsupportedTypePolicy::Reject,
        )
        .expect("float postgres");
        assert_eq!(result, "DOUBLE PRECISION");
    }

    #[test]
    fn bool_maps_to_integer_on_libsql_not_boolean() {
        // Catches: SQLite/libsql receiving BOOLEAN DDL which SQLite accepts but stores as NUMERIC affinity
        let result = vox_type_to_sql(BackendKind::Libsql, "bool", UnsupportedTypePolicy::Reject)
            .expect("bool libsql");
        assert_eq!(result, "INTEGER", "libsql bool must be INTEGER not BOOLEAN");
    }

    #[test]
    fn bytes_maps_to_bytea_on_postgres_not_blob() {
        // Catches: postgres bytes returning generic "BLOB" (invalid in Postgres DDL)
        let result = vox_type_to_sql(
            BackendKind::Postgres,
            "bytes",
            UnsupportedTypePolicy::Reject,
        )
        .expect("bytes postgres");
        assert_eq!(result, "BYTEA");
    }
}
