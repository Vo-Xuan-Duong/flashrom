use std::{env, fs, path::PathBuf};

const TRANSPARENT_PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00,
    0x00, 0x05, 0x00, 0x01, 0x7a, 0x5e, 0xab, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

fn ensure_fallback_png() {
    let icon_path = PathBuf::from("icons/icon.png");
    if icon_path.is_file() {
        return;
    }

    if let Some(parent) = icon_path.parent() {
        fs::create_dir_all(parent).expect("failed to create fallback icon directory");
    }
    fs::write(&icon_path, TRANSPARENT_PNG_1X1).expect("failed to write fallback Tauri PNG icon");
}

fn write_fallback_icon() -> PathBuf {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let icon_path = out_dir.join("flashrom-fallback.ico");

    let mut ico = Vec::with_capacity(22 + TRANSPARENT_PNG_1X1.len());
    ico.extend_from_slice(&[
        0x00, 0x00, // reserved
        0x01, 0x00, // icon type
        0x01, 0x00, // one image
        0x01, // width
        0x01, // height
        0x00, // color count
        0x00, // reserved
        0x01, 0x00, // color planes
        0x20, 0x00, // 32 bits per pixel
    ]);
    ico.extend_from_slice(&(TRANSPARENT_PNG_1X1.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22_u32.to_le_bytes());
    ico.extend_from_slice(TRANSPARENT_PNG_1X1);

    fs::write(&icon_path, ico).expect("failed to write fallback Windows icon");
    icon_path
}

fn main() {
    ensure_fallback_png();

    if PathBuf::from("icons/icon.ico").is_file() {
        tauri_build::build();
        return;
    }

    let windows = tauri_build::WindowsAttributes::new().window_icon_path(write_fallback_icon());
    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attributes).expect("failed to run Tauri build script");
}
