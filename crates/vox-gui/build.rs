fn main() {
    vox_build_meta::emit();
    // Must not swallow errors: a missing Windows manifest yields STATUS_ENTRYPOINT_NOT_FOUND at runtime.
    if let Err(err) = tauri_build::try_build(tauri_build::Attributes::new()) {
        panic!("tauri build script failed: {err}");
    }
}
