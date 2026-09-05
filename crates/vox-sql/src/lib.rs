//! Engine-agnostic SQL surface for Vox app-plane data backends.
//!
//! Connects via `VOX_APP_DB_URL`, normalizes rows/values, and exposes DDL,
//! introspection, and migration helpers for brownfield database interop.

#[cfg(feature = "runtime")]
use async_trait::async_trait;
#[cfg(all(feature = "runtime", feature = "mysql"))]
use sqlx::mysql::{MySqlArguments, MySqlPoolOptions};
#[cfg(all(feature = "runtime", feature = "postgres"))]
use sqlx::postgres::{PgArguments, PgPoolOptions};
#[cfg(all(feature = "runtime", feature = "postgres"))]
use sqlx::{
    Arguments as _, Column as _, PgPool, Postgres, Row as _,
    pool::PoolConnection as PgPoolConnection,
};
#[cfg(all(feature = "runtime", feature = "mysql"))]
use sqlx::{MySql, MySqlPool, pool::PoolConnection as MyPoolConnection};
#[cfg(all(feature = "runtime", any(feature = "postgres", feature = "mysql")))]
use tokio::sync::Mutex;
#[cfg(feature = "runtime")]
use vox_db::{Codex, DbConfig};
#[cfg(feature = "runtime")]
use vox_secrets::SecretId;

pub mod build;
pub mod ddl;
#[cfg(feature = "runtime")]
pub mod introspect;
#[cfg(feature = "runtime")]
pub mod migrate;
pub mod schema_model;
pub mod type_map;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderStyle {
    QuestionMarkNumbered,
    DollarNumbered,
    QuestionMark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierQuoteStyle {
    DoubleQuote,
    Backtick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertStyle {
    OnConflict,
    OnDuplicateKeyUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnClauseStyle {
    Returning,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlDialect {
    pub name: &'static str,
    pub placeholder_style: PlaceholderStyle,
    pub identifier_quote_style: IdentifierQuoteStyle,
    pub upsert_style: UpsertStyle,
    pub return_clause_style: ReturnClauseStyle,
}

impl SqlDialect {
    pub const fn sqlite() -> Self {
        Self {
            name: "sqlite",
            placeholder_style: PlaceholderStyle::QuestionMarkNumbered,
            identifier_quote_style: IdentifierQuoteStyle::DoubleQuote,
            upsert_style: UpsertStyle::OnConflict,
            return_clause_style: ReturnClauseStyle::Returning,
        }
    }

    pub const fn postgres() -> Self {
        Self {
            name: "postgres",
            placeholder_style: PlaceholderStyle::DollarNumbered,
            identifier_quote_style: IdentifierQuoteStyle::DoubleQuote,
            upsert_style: UpsertStyle::OnConflict,
            return_clause_style: ReturnClauseStyle::Returning,
        }
    }

    pub const fn mysql() -> Self {
        Self {
            name: "mysql",
            placeholder_style: PlaceholderStyle::QuestionMark,
            identifier_quote_style: IdentifierQuoteStyle::Backtick,
            upsert_style: UpsertStyle::OnDuplicateKeyUpdate,
            return_clause_style: ReturnClauseStyle::Unsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

pub type SqlRow = Vec<(String, SqlValue)>;

#[derive(Debug, Clone)]
pub struct SqlBackendError {
    pub message: String,
}

impl SqlBackendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SqlBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SqlBackendError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Libsql,
    #[cfg(feature = "postgres")]
    Postgres,
    #[cfg(feature = "mysql")]
    MySql,
}

impl BackendKind {
    pub fn from_url(url: &str) -> Result<Self, SqlBackendError> {
        let lower = url.to_ascii_lowercase();
        #[cfg(feature = "postgres")]
        if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
            return Ok(Self::Postgres);
        }
        #[cfg(feature = "mysql")]
        if lower.starts_with("mysql://") {
            return Ok(Self::MySql);
        }
        if lower.starts_with("libsql://")
            || lower.starts_with("sqlite://")
            || lower.starts_with("sqlite:")
            || lower.starts_with("file:")
        {
            return Ok(Self::Libsql);
        }
        Err(SqlBackendError::new(format!(
            "unsupported VOX_APP_DB_URL scheme: {url}"
        )))
    }
}

#[cfg(feature = "runtime")]
#[async_trait]
pub trait SqlBackend: Send + Sync {
    async fn connect(url: &str) -> Result<Self, SqlBackendError>
    where
        Self: Sized;

    fn dialect(&self) -> &SqlDialect;

    async fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<SqlRow>, SqlBackendError>;

    async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlBackendError>;

    async fn begin_transaction(&self) -> Result<(), SqlBackendError>;
    async fn commit_transaction(&self) -> Result<(), SqlBackendError>;
    async fn rollback_transaction(&self) -> Result<(), SqlBackendError>;
}

#[cfg(feature = "runtime")]
pub enum AnySqlBackend {
    Libsql(LibsqlBackend),
    #[cfg(feature = "postgres")]
    Postgres(PostgresBackend),
    #[cfg(feature = "mysql")]
    MySql(MySqlBackend),
}

#[cfg(feature = "runtime")]
impl AnySqlBackend {
    #[must_use]
    pub fn backend_kind(&self) -> BackendKind {
        match self {
            Self::Libsql(_) => BackendKind::Libsql,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => BackendKind::Postgres,
            #[cfg(feature = "mysql")]
            Self::MySql(_) => BackendKind::MySql,
        }
    }

    pub async fn connect_from_url(url: &str) -> Result<Self, SqlBackendError> {
        match BackendKind::from_url(url)? {
            BackendKind::Libsql => Ok(Self::Libsql(LibsqlBackend::connect(url).await?)),
            #[cfg(feature = "postgres")]
            BackendKind::Postgres => Ok(Self::Postgres(PostgresBackend::connect(url).await?)),
            #[cfg(feature = "mysql")]
            BackendKind::MySql => Ok(Self::MySql(MySqlBackend::connect(url).await?)),
        }
    }

    pub async fn connect_from_app_env() -> Result<Self, SqlBackendError> {
        let app_url = vox_secrets::resolve_secret(SecretId::VoxAppDbUrl)
            .expose()
            .map(str::to_owned)
            .or_else(|| {
                vox_secrets::resolve_secret(SecretId::VoxDbUrl)
                    .expose()
                    .map(str::to_owned)
            });

        if let Some(url) = app_url {
            return Self::connect_from_url(&url).await;
        }

        let cfg = DbConfig::resolve_canonical().map_err(SqlBackendError::new)?;
        Ok(Self::Libsql(LibsqlBackend::connect_from_config(cfg).await?))
    }

    pub fn dialect(&self) -> &SqlDialect {
        match self {
            Self::Libsql(backend) => backend.dialect(),
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.dialect(),
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.dialect(),
        }
    }

    pub async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<SqlRow>, SqlBackendError> {
        match self {
            Self::Libsql(backend) => backend.query(sql, params).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.query(sql, params).await,
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.query(sql, params).await,
        }
    }

    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlBackendError> {
        match self {
            Self::Libsql(backend) => backend.execute(sql, params).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.execute(sql, params).await,
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.execute(sql, params).await,
        }
    }

    pub async fn begin_transaction(&self) -> Result<(), SqlBackendError> {
        match self {
            Self::Libsql(backend) => backend.begin_transaction().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.begin_transaction().await,
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.begin_transaction().await,
        }
    }

    pub async fn commit_transaction(&self) -> Result<(), SqlBackendError> {
        match self {
            Self::Libsql(backend) => backend.commit_transaction().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.commit_transaction().await,
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.commit_transaction().await,
        }
    }

    pub async fn rollback_transaction(&self) -> Result<(), SqlBackendError> {
        match self {
            Self::Libsql(backend) => backend.rollback_transaction().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(backend) => backend.rollback_transaction().await,
            #[cfg(feature = "mysql")]
            Self::MySql(backend) => backend.rollback_transaction().await,
        }
    }
}

#[cfg(feature = "runtime")]
pub struct LibsqlBackend {
    codex: Codex,
    dialect: SqlDialect,
}

#[cfg(feature = "runtime")]
impl LibsqlBackend {
    #[must_use]
    pub fn codex(&self) -> &Codex {
        &self.codex
    }

    pub async fn connect_from_config(config: DbConfig) -> Result<Self, SqlBackendError> {
        let codex = Codex::connect(config)
            .await
            .map_err(|e| SqlBackendError::new(format!("libsql connect failed: {e}")))?;
        Ok(Self {
            codex,
            dialect: SqlDialect::sqlite(),
        })
    }
}

#[cfg(feature = "runtime")]
#[async_trait]
impl SqlBackend for LibsqlBackend {
    async fn connect(url: &str) -> Result<Self, SqlBackendError> {
        let token = vox_secrets::resolve_secret(SecretId::VoxDbToken)
            .expose()
            .unwrap_or_default()
            .to_owned();
        Self::connect_from_config(DbConfig::remote(url, token)).await
    }

    fn dialect(&self) -> &SqlDialect {
        &self.dialect
    }

    async fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<SqlRow>, SqlBackendError> {
        let mut args = Vec::with_capacity(params.len());
        for value in params {
            args.push(sql_value_to_turso_value(value));
        }

        let mut rows = self
            .codex
            .connection()
            .query(sql, turso::params_from_iter(args))
            .await
            .map_err(|e| SqlBackendError::new(format!("libsql query failed: {e}")))?;

        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| SqlBackendError::new(format!("libsql row iteration failed: {e}")))?
        {
            out.push(map_turso_row(&row)?);
        }
        Ok(out)
    }

    async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlBackendError> {
        if params.is_empty() {
            return self
                .codex
                .connection()
                .execute(sql, ())
                .await
                .map_err(|e| SqlBackendError::new(format!("libsql execute failed: {e}")));
        }

        let mut args = Vec::with_capacity(params.len());
        for value in params {
            args.push(sql_value_to_turso_value(value));
        }

        self.codex
            .connection()
            .execute(sql, turso::params_from_iter(args))
            .await
            .map_err(|e| SqlBackendError::new(format!("libsql execute failed: {e}")))
    }

    async fn begin_transaction(&self) -> Result<(), SqlBackendError> {
        self.codex
            .connection()
            .execute("BEGIN IMMEDIATE", ())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("libsql begin transaction failed: {e}")))
    }

    async fn commit_transaction(&self) -> Result<(), SqlBackendError> {
        self.codex
            .connection()
            .execute("COMMIT", ())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("libsql commit failed: {e}")))
    }

    async fn rollback_transaction(&self) -> Result<(), SqlBackendError> {
        self.codex
            .connection()
            .execute("ROLLBACK", ())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("libsql rollback failed: {e}")))
    }
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
pub struct PostgresBackend {
    pool: PgPool,
    dialect: SqlDialect,
    tx_conn: Mutex<Option<PgPoolConnection<Postgres>>>,
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
impl PostgresBackend {
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
#[async_trait]
impl SqlBackend for PostgresBackend {
    async fn connect(url: &str) -> Result<Self, SqlBackendError> {
        let pool = PgPoolOptions::new()
            .connect(url)
            .await
            .map_err(|e| SqlBackendError::new(format!("postgres connect failed: {e}")))?;
        Ok(Self {
            pool,
            dialect: SqlDialect::postgres(),
            tx_conn: Mutex::new(None),
        })
    }

    fn dialect(&self) -> &SqlDialect {
        &self.dialect
    }

    async fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<SqlRow>, SqlBackendError> {
        let sql_owned = sql.to_owned();
        let mut args = PgArguments::default();
        for param in params {
            add_pg_argument(&mut args, param)?;
        }
        let mut guard = self.tx_conn.lock().await;
        let _rows = if let Some(conn) = guard.as_mut() {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .fetch_all(conn.as_mut())
                .await
                .map_err(|e| SqlBackendError::new(format!("postgres query failed: {e}")))?
        } else {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SqlBackendError::new(format!("postgres query failed: {e}")))?
        };
        let mut out = Vec::with_capacity(_rows.len());
        for row in _rows {
            out.push(map_postgres_row(&row)?);
        }
        Ok(out)
    }

    async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlBackendError> {
        let sql_owned = sql.to_owned();
        let mut args = PgArguments::default();
        for param in params {
            add_pg_argument(&mut args, param)?;
        }
        let mut guard = self.tx_conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .execute(conn.as_mut())
                .await
                .map(|done| done.rows_affected())
                .map_err(|e| SqlBackendError::new(format!("postgres execute failed: {e}")))
        } else {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .execute(&self.pool)
                .await
                .map(|done| done.rows_affected())
                .map_err(|e| SqlBackendError::new(format!("postgres execute failed: {e}")))
        }
    }

    async fn begin_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        if guard.is_some() {
            return Err(SqlBackendError::new(
                "postgres begin transaction failed: transaction already active",
            ));
        }
        let mut conn =
            self.pool.acquire().await.map_err(|e| {
                SqlBackendError::new(format!("postgres begin transaction failed: {e}"))
            })?;
        sqlx::query("BEGIN")
            .execute(conn.as_mut())
            .await
            .map_err(|e| SqlBackendError::new(format!("postgres begin transaction failed: {e}")))?;
        *guard = Some(conn);
        Ok(())
    }

    async fn commit_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        let Some(mut conn) = guard.take() else {
            return Err(SqlBackendError::new(
                "postgres commit failed: no active transaction",
            ));
        };
        sqlx::query("COMMIT")
            .execute(conn.as_mut())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("postgres commit failed: {e}")))
    }

    async fn rollback_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        let Some(mut conn) = guard.take() else {
            return Err(SqlBackendError::new(
                "postgres rollback failed: no active transaction",
            ));
        };
        sqlx::query("ROLLBACK")
            .execute(conn.as_mut())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("postgres rollback failed: {e}")))
    }
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
pub struct MySqlBackend {
    pool: MySqlPool,
    dialect: SqlDialect,
    tx_conn: Mutex<Option<MyPoolConnection<MySql>>>,
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
impl MySqlBackend {
    #[must_use]
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
#[async_trait]
impl SqlBackend for MySqlBackend {
    async fn connect(url: &str) -> Result<Self, SqlBackendError> {
        let pool = MySqlPoolOptions::new()
            .connect(url)
            .await
            .map_err(|e| SqlBackendError::new(format!("mysql connect failed: {e}")))?;
        Ok(Self {
            pool,
            dialect: SqlDialect::mysql(),
            tx_conn: Mutex::new(None),
        })
    }

    fn dialect(&self) -> &SqlDialect {
        &self.dialect
    }

    async fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<SqlRow>, SqlBackendError> {
        let sql_owned = sql.to_owned();
        let mut args = MySqlArguments::default();
        for param in params {
            add_mysql_argument(&mut args, param)?;
        }
        let mut guard = self.tx_conn.lock().await;
        let _rows = if let Some(conn) = guard.as_mut() {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .fetch_all(conn.as_mut())
                .await
                .map_err(|e| SqlBackendError::new(format!("mysql query failed: {e}")))?
        } else {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SqlBackendError::new(format!("mysql query failed: {e}")))?
        };
        let mut out = Vec::with_capacity(_rows.len());
        for row in _rows {
            out.push(map_mysql_row(&row)?);
        }
        Ok(out)
    }

    async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqlBackendError> {
        let sql_owned = sql.to_owned();
        let mut args = MySqlArguments::default();
        for param in params {
            add_mysql_argument(&mut args, param)?;
        }
        let mut guard = self.tx_conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .execute(conn.as_mut())
                .await
                .map(|done| done.rows_affected())
                .map_err(|e| SqlBackendError::new(format!("mysql execute failed: {e}")))
        } else {
            sqlx::query_with(sqlx::AssertSqlSafe(sql_owned), args)
                .execute(&self.pool)
                .await
                .map(|done| done.rows_affected())
                .map_err(|e| SqlBackendError::new(format!("mysql execute failed: {e}")))
        }
    }

    async fn begin_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        if guard.is_some() {
            return Err(SqlBackendError::new(
                "mysql begin transaction failed: transaction already active",
            ));
        }
        let mut conn =
            self.pool.acquire().await.map_err(|e| {
                SqlBackendError::new(format!("mysql begin transaction failed: {e}"))
            })?;
        sqlx::raw_sql("BEGIN")
            .execute(conn.as_mut())
            .await
            .map_err(|e| SqlBackendError::new(format!("mysql begin transaction failed: {e}")))?;
        *guard = Some(conn);
        Ok(())
    }

    async fn commit_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        let Some(mut conn) = guard.take() else {
            return Err(SqlBackendError::new(
                "mysql commit failed: no active transaction",
            ));
        };
        sqlx::raw_sql("COMMIT")
            .execute(conn.as_mut())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("mysql commit failed: {e}")))
    }

    async fn rollback_transaction(&self) -> Result<(), SqlBackendError> {
        let mut guard = self.tx_conn.lock().await;
        let Some(mut conn) = guard.take() else {
            return Err(SqlBackendError::new(
                "mysql rollback failed: no active transaction",
            ));
        };
        sqlx::raw_sql("ROLLBACK")
            .execute(conn.as_mut())
            .await
            .map(|_| ())
            .map_err(|e| SqlBackendError::new(format!("mysql rollback failed: {e}")))
    }
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
fn add_pg_argument(args: &mut PgArguments, value: &SqlValue) -> Result<(), SqlBackendError> {
    match value {
        SqlValue::Null => args.add::<Option<i64>>(None),
        SqlValue::Bool(v) => args.add(*v),
        SqlValue::Int(v) => args.add(*v),
        SqlValue::Float(v) => args.add(*v),
        SqlValue::Text(v) => args.add(v.clone()),
        SqlValue::Bytes(v) => args.add(v.clone()),
    }
    .map_err(|e| SqlBackendError::new(format!("postgres argument encode failed: {e}")))?;
    Ok(())
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
fn add_mysql_argument(args: &mut MySqlArguments, value: &SqlValue) -> Result<(), SqlBackendError> {
    match value {
        SqlValue::Null => args.add::<Option<i64>>(None),
        SqlValue::Bool(v) => args.add(*v),
        SqlValue::Int(v) => args.add(*v),
        SqlValue::Float(v) => args.add(*v),
        SqlValue::Text(v) => args.add(v.clone()),
        SqlValue::Bytes(v) => args.add(v.clone()),
    }
    .map_err(|e| SqlBackendError::new(format!("mysql argument encode failed: {e}")))?;
    Ok(())
}

#[cfg(feature = "runtime")]
fn sql_value_to_turso_value(value: &SqlValue) -> turso::Value {
    match value {
        SqlValue::Null => turso::Value::Null,
        SqlValue::Bool(v) => turso::Value::Integer(if *v { 1 } else { 0 }),
        SqlValue::Int(v) => turso::Value::Integer(*v),
        SqlValue::Float(v) => turso::Value::Real(*v),
        SqlValue::Text(v) => turso::Value::Text(v.clone()),
        SqlValue::Bytes(v) => turso::Value::Blob(v.clone()),
    }
}

#[cfg(feature = "runtime")]
fn map_turso_row(row: &turso::Row) -> Result<SqlRow, SqlBackendError> {
    let mut out = Vec::with_capacity(row.column_count());
    for idx in 0..row.column_count() {
        let value = row.get_value(idx).map_err(|e| {
            SqlBackendError::new(format!(
                "libsql row cell decode failed at column {idx}: {e}"
            ))
        })?;
        out.push((format!("col_{idx}"), turso_value_to_sql_value(value)));
    }
    Ok(out)
}

#[cfg(feature = "runtime")]
fn turso_value_to_sql_value(value: turso::Value) -> SqlValue {
    match value {
        turso::Value::Null => SqlValue::Null,
        turso::Value::Integer(v) => SqlValue::Int(v),
        turso::Value::Real(v) => SqlValue::Float(v),
        turso::Value::Text(v) => SqlValue::Text(v),
        turso::Value::Blob(v) => SqlValue::Bytes(v),
    }
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
fn map_postgres_row(row: &sqlx::postgres::PgRow) -> Result<SqlRow, SqlBackendError> {
    let mut out = Vec::with_capacity(row.columns().len());
    for (idx, col) in row.columns().iter().enumerate() {
        out.push((col.name().to_owned(), decode_postgres_cell(row, idx)?));
    }
    Ok(out)
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
fn map_mysql_row(row: &sqlx::mysql::MySqlRow) -> Result<SqlRow, SqlBackendError> {
    let mut out = Vec::with_capacity(row.columns().len());
    for (idx, col) in row.columns().iter().enumerate() {
        out.push((col.name().to_owned(), decode_mysql_cell(row, idx)?));
    }
    Ok(out)
}

#[cfg(all(feature = "runtime", feature = "postgres"))]
fn decode_postgres_cell(
    row: &sqlx::postgres::PgRow,
    idx: usize,
) -> Result<SqlValue, SqlBackendError> {
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Bool));
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i8>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Int));
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Float(f64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Float));
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Text));
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Bytes));
    }
    Err(SqlBackendError::new(format!(
        "unsupported postgres cell type at column index {idx}"
    )))
}

#[cfg(all(feature = "runtime", feature = "mysql"))]
fn decode_mysql_cell(row: &sqlx::mysql::MySqlRow, idx: usize) -> Result<SqlValue, SqlBackendError> {
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i8>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Int));
    }
    if let Ok(v) = row.try_get::<Option<u64>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(n as i64)));
    }
    if let Ok(v) = row.try_get::<Option<u32>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Int(i64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, |n| SqlValue::Float(f64::from(n))));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Float));
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Bool));
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Text));
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return Ok(v.map_or(SqlValue::Null, SqlValue::Bytes));
    }
    Err(SqlBackendError::new(format!(
        "unsupported mysql cell type at column index {idx}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_dialect_shape() {
        let d = SqlDialect::sqlite();
        assert_eq!(d.name, "sqlite");
        assert_eq!(d.placeholder_style, PlaceholderStyle::QuestionMarkNumbered);
        assert_eq!(d.identifier_quote_style, IdentifierQuoteStyle::DoubleQuote);
        assert_eq!(d.upsert_style, UpsertStyle::OnConflict);
    }

    #[test]
    fn postgres_dialect_shape() {
        let d = SqlDialect::postgres();
        assert_eq!(d.name, "postgres");
        assert_eq!(d.placeholder_style, PlaceholderStyle::DollarNumbered);
        assert_eq!(d.return_clause_style, ReturnClauseStyle::Returning);
    }

    #[test]
    fn mysql_dialect_shape() {
        let d = SqlDialect::mysql();
        assert_eq!(d.name, "mysql");
        assert_eq!(d.placeholder_style, PlaceholderStyle::QuestionMark);
        assert_eq!(d.identifier_quote_style, IdentifierQuoteStyle::Backtick);
        assert_eq!(d.upsert_style, UpsertStyle::OnDuplicateKeyUpdate);
    }

    #[test]
    fn backend_kind_from_url_dispatch() {
        #[cfg(feature = "postgres")]
        assert_eq!(
            BackendKind::from_url("postgres://localhost/db").unwrap(),
            BackendKind::Postgres
        );
        #[cfg(feature = "mysql")]
        assert_eq!(
            BackendKind::from_url("mysql://localhost/db").unwrap(),
            BackendKind::MySql
        );
        assert_eq!(
            BackendKind::from_url("libsql://example.db").unwrap(),
            BackendKind::Libsql
        );
        assert!(BackendKind::from_url("mssql://localhost/db").is_err());
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn sql_value_to_turso_roundtrip_scalar_shapes() {
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Null),
            turso::Value::Null
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Bool(true)),
            turso::Value::Integer(1)
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Bool(false)),
            turso::Value::Integer(0)
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Int(7)),
            turso::Value::Integer(7)
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Float(1.5)),
            turso::Value::Real(1.5)
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Text("x".to_owned())),
            turso::Value::Text("x".to_owned())
        );
        assert_eq!(
            sql_value_to_turso_value(&SqlValue::Bytes(vec![1, 2])),
            turso::Value::Blob(vec![1, 2])
        );
    }
}
