//! `@table` / VoxDB `schema.ts` generator.

mod from_ast;
mod from_hir;
mod type_maps;

pub use from_ast::generate_voxdb_schema;
pub use from_hir::generate_voxdb_schema_from_hir;
pub use type_maps::type_to_voxdb_validator;
