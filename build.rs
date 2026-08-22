fn main() {
    // Ensure the bundled-Python resource directory exists so Tauri's build-time
    // resource validation passes even when `pnpm bundle:python` has not been
    // run (fresh clones, `tauri dev`, CI without the runtime). For release
    // installers, `pnpm bundle:python` populates this path with a real Python
    // + mineru runtime that gets bundled automatically. When the directory is
    // empty, the app simply falls back to downloading the runtime on first launch.
    let bundle_python = std::path::Path::new("bundle_extras/python");
    if !bundle_python.exists() {
        let _ = std::fs::create_dir_all(bundle_python);
    }
    tauri_build::build();
}
