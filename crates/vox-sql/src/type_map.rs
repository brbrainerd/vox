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
