//! DDL Compiler — convert `@table` AST declarations into SQLite DDL.
//!
//! This is the bridge between the Vox type system and SQLite's physical schema.
//! It generates `CREATE TABLE`, `CREATE INDEX`, and type-safe DDL from the AST.

pub mod activity_result_cache;
mod diff;
mod emit;

pub use diff::{SchemaDiff, describe_diff, diff_schemas, diff_to_sql};
pub use emit::{
    collection_index_to_ddl, collection_info_to_ddl, collection_to_ddl, collections_to_ddl,
    index_info_to_ddl, index_to_ddl, indexes_to_ddl, sqlite_affinity_for_named_vox_type,
    table_info_to_ddl, table_to_ddl, tables_to_ddl, to_snake_case, type_to_sqlite_type,
    vector_index_to_ddl, vox_type_to_sqlite_type,
};
