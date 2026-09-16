fn main() {
    // tauri-build only tracks tauri.conf.json / capabilities, so icon-only
    // changes would not re-run this script and the Windows resource (exe icon)
    // and embedded window icon would stay stale. Watch the icon files we ship.
    for icon in [
        "icons/icon.ico",
        "icons/icon.png",
        "icons/icon.icns",
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
    ] {
        println!("cargo:rerun-if-changed={icon}");
    }

    tauri_build::build();
}
