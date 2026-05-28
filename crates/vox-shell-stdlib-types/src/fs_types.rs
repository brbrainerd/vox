//! File-system record types shared between interpreter and codegen paths.

/// One directory entry with structured metadata (`std.fs.list_dir_detailed` / `std.fs.stat`).
///
/// Identical layout on both paths — the interpreter creates these directly and the codegen
/// path serialises them through the runtime ABI. Serde derives are needed for the runtime
/// JSON bridge; they are harmless on the interpreter side.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VoxFileRecord {
    pub name: String,
    pub path: String,
    pub size: i64,
    pub modified_ms: i64,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}
