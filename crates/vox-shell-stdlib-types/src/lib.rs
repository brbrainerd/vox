//! Shared data types for the Vox shell stdlib surface (`std.fs.*`, `std.csv`).
//!
//! Both `vox-compiler` (interpreter path) and `vox-actor-runtime` (codegen path)
//! need the same file-record shape. This L0 crate holds the canonical definition
//! so neither side has to duplicate it or create a cycle.

pub mod fs_types;

#[cfg(test)]
mod tests {
    use super::fs_types::VoxFileRecord;

    #[test]
    fn vox_file_record_constructs_and_clones() {
        let rec = VoxFileRecord {
            name: "a.txt".into(),
            path: "/tmp/a.txt".into(),
            size: 7,
            modified_ms: 0,
            is_dir: false,
            is_file: true,
            is_symlink: false,
        };
        assert_eq!(rec, rec.clone());
        assert!(rec.is_file && !rec.is_dir);
    }
}
