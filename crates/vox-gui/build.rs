fn main() {
    vox_build_meta::emit();
    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(""));
    tauri_build::try_build(attrs).ok();
}
