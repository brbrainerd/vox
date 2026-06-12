//! Rust codegen for `@table` / DB surface (projections walk + struct/DDL emission).

mod codegen;
mod projections;

pub use codegen::{
    emit_db_setup, emit_index_ddl, emit_schema_drift_verify, emit_table_ddl, emit_table_struct,
};
pub use projections::{collect_table_select_projections, validate_db_projection_suffixes_unique};

pub(crate) use projections::db_projection_method_suffix;
