use super::hidden_command;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PNG_BYTES: usize = 256 * 1024;
const TARGET_EDGE: i32 = 128;

struct CachedIcon {
    path: PathBuf,
    modified: Option<SystemTime>,
    data_url: String,
}

static CACHE: OnceLock<Mutex<HashMap<String, CachedIcon>>> = OnceLock::new();

fn icon_cache() -> &'static Mutex<HashMap<String, CachedIcon>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn data_url_for_cursor(app_dir: Option<&Path>) -> Option<String> {
    cached("cursor", app_dir.and_then(find_cursor_icon).as_deref())
}

pub fn data_url_for_claude(install_location: Option<&Path>) -> Option<String> {
    cached("claude", install_location.and_then(find_claude_icon).as_deref())
}

fn cached(app_id: &str, source: Option<&Path>) -> Option<String> {
    let source = source?;
    let modified = fs::metadata(source).ok().and_then(|meta| meta.modified().ok());
    if let Ok(cache) = icon_cache().lock() {
        if let Some(entry) = cache.get(app_id) {
            if entry.path == source && entry.modified == modified {
                return Some(entry.data_url.clone());
            }
        }
    }

    let png = decode_icon_file(source)?;
    if png.len() > MAX_PNG_BYTES || !is_png(&png) {
        return None;
    }
    let data_url = format!("data:image/png;base64,{}", STANDARD.encode(&png));
    if let Ok(mut cache) = icon_cache().lock() {
        cache.insert(
            app_id.to_string(),
            CachedIcon {
                path: source.to_path_buf(),
                modified,
                data_url: data_url.clone(),
            },
        );
    }
    Some(data_url)
}

fn find_cursor_icon(app_dir: &Path) -> Option<PathBuf> {
    let root = install_root_from_app_dir(app_dir).unwrap_or_else(|| app_dir.to_path_buf());
    first_existing(cursor_icon_candidates(&root))
}

fn find_claude_icon(install_location: &Path) -> Option<PathBuf> {
    first_existing(claude_icon_candidates(install_location))
}

fn install_root_from_app_dir(app_dir: &Path) -> Option<PathBuf> {
    if app_dir.extension().is_some_and(|ext| ext == "app") {
        return Some(app_dir.to_path_buf());
    }
    if app_dir.file_name()?.to_string_lossy() != "app" {
        return Some(app_dir.to_path_buf());
    }
    let resources = app_dir.parent()?;
    let contents = resources.parent()?;
    if resources.file_name()?.to_string_lossy() == "Resources"
        && contents.file_name()?.to_string_lossy() == "Contents"
    {
        return contents.parent().map(Path::to_path_buf);
    }
    if resources
        .file_name()?
        .to_string_lossy()
        .eq_ignore_ascii_case("resources")
    {
        return Some(contents.to_path_buf());
    }
    None
}

fn cursor_icon_candidates(root: &Path) -> Vec<PathBuf> {
    let mut candidates = macos_bundle_icon_candidates(root, &["Cursor.icns", "Code.icns", "electron.icns"]);
    candidates.extend([
        root.join("Cursor.icns"),
        root.join("Cursor.ico"),
        root.join("Cursor.exe"),
        root.join("cursor.png"),
        root.join("resources/icon.ico"),
        root.join("resources/app.ico"),
        root.join("resources/app/resources/linux/cursor.png"),
        root.join("resources/app/resources/linux/code.png"),
        PathBuf::from("/usr/share/pixmaps/cursor.png"),
        PathBuf::from("/usr/share/icons/hicolor/128x128/apps/cursor.png"),
        PathBuf::from("/usr/share/icons/hicolor/256x256/apps/cursor.png"),
    ]);
    candidates
}

fn claude_icon_candidates(root: &Path) -> Vec<PathBuf> {
    let mut candidates = macos_bundle_icon_candidates(root, &["Claude.icns", "AppIcon.icns", "electron.icns"]);
    candidates.extend([
        root.join("Claude.icns"),
        root.join("Claude.ico"),
        root.join("Claude.exe"),
        root.join("app.ico"),
        root.join("StoreLogo.png"),
        root.join("Assets/StoreLogo.png"),
        root.join("Assets/Square44x44Logo.png"),
        root.join("Assets/Square150x150Logo.png"),
        root.join("app/resources/icon.png"),
        root.join("resources/icon.png"),
        PathBuf::from("/usr/share/icons/hicolor/128x128/apps/claude-desktop.png"),
        PathBuf::from("/usr/share/icons/hicolor/256x256/apps/claude-desktop.png"),
    ]);
    candidates
}

fn macos_bundle_icon_candidates(root: &Path, names: &[&str]) -> Vec<PathBuf> {
    let bundle = if root.extension().is_some_and(|ext| ext == "app") {
        root.to_path_buf()
    } else {
        return Vec::new();
    };
    let resources = bundle.join("Contents/Resources");
    let mut candidates = Vec::new();
    if let Some(name) = plist_icon_file(&bundle.join("Contents/Info.plist")) {
        let file_name = if name.rsplit('.').next().is_some_and(|ext| {
            matches!(ext, "icns" | "png" | "ico")
        }) {
            name
        } else {
            format!("{name}.icns")
        };
        candidates.push(resources.join(file_name));
    }
    for name in names {
        candidates.push(resources.join(name));
    }
    candidates
}

fn plist_icon_file(info: &Path) -> Option<String> {
    if !info.is_file() {
        return None;
    }
    for key in ["CFBundleIconFile", "CFBundleIconName"] {
        let value = hidden_command("plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(info)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|value| !value.is_empty() && !value.contains('\n'));
        if let Some(value) = value {
            return Some(value);
        }
    }
    None
}

fn first_existing(paths: Vec<PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.is_file())
}

fn decode_icon_file(path: &Path) -> Option<Vec<u8>> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "exe" {
        return extract_associated_icon(path);
    }

    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let data = fs::read(path).ok()?;
    if is_png(&data) {
        return Some(data);
    }
    if let Some(png) = extract_png_from_icns(&data) {
        return Some(png);
    }
    if let Some(png) = extract_png_from_ico(&data) {
        return Some(png);
    }
    if extension == "icns" || extension == "ico" || extension == "app" {
        if let Some(png) = convert_with_sips(path) {
            return Some(png);
        }
    }
    if extension == "ico" {
        return extract_associated_icon(path);
    }
    None
}

fn is_png(data: &[u8]) -> bool {
    data.starts_with(b"\x89PNG\r\n\x1a\n") && data.len() >= 24 && data.len() <= MAX_PNG_BYTES
}

fn png_edge(data: &[u8]) -> Option<i32> {
    if !is_png(data) {
        return None;
    }
    let width = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(data[20..24].try_into().ok()?);
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return None;
    }
    Some(width.max(height) as i32)
}

fn better_png<'a>(current: Option<&'a [u8]>, candidate: &'a [u8]) -> Option<&'a [u8]> {
    let candidate_edge = png_edge(candidate)?;
    let Some(current) = current else {
        return Some(candidate);
    };
    let current_edge = png_edge(current)?;
    let candidate_delta = (candidate_edge - TARGET_EDGE).abs();
    let current_delta = (current_edge - TARGET_EDGE).abs();
    if candidate_delta < current_delta
        || (candidate_delta == current_delta && candidate.len() < current.len())
    {
        Some(candidate)
    } else {
        Some(current)
    }
}

pub(crate) fn extract_png_from_icns(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 8 || &data[..4] != b"icns" {
        return None;
    }
    let declared = u32::from_be_bytes(data[4..8].try_into().ok()?) as usize;
    if declared > data.len() {
        return None;
    }
    let mut offset = 8;
    let mut best: Option<&[u8]> = None;
    while offset + 8 <= declared {
        let size = u32::from_be_bytes(data[offset + 4..offset + 8].try_into().ok()?) as usize;
        if size < 8 || offset + size > declared {
            break;
        }
        let payload = &data[offset + 8..offset + size];
        if is_png(payload) {
            best = better_png(best, payload);
        }
        offset += size;
    }
    best.map(|png| png.to_vec())
}

pub(crate) fn extract_png_from_ico(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 6 {
        return None;
    }
    let reserved = u16::from_le_bytes(data[0..2].try_into().ok()?);
    let kind = u16::from_le_bytes(data[2..4].try_into().ok()?);
    let count = u16::from_le_bytes(data[4..6].try_into().ok()?) as usize;
    if reserved != 0 || kind != 1 || count == 0 || count > 64 {
        return None;
    }
    let mut best: Option<&[u8]> = None;
    for index in 0..count {
        let entry = 6 + index * 16;
        if entry + 16 > data.len() {
            break;
        }
        let bytes = u32::from_le_bytes(data[entry + 8..entry + 12].try_into().ok()?) as usize;
        let offset = u32::from_le_bytes(data[entry + 12..entry + 16].try_into().ok()?) as usize;
        if bytes == 0 || offset.checked_add(bytes).is_none_or(|end| end > data.len()) {
            continue;
        }
        let payload = &data[offset..offset + bytes];
        if is_png(payload) {
            best = better_png(best, payload);
        }
    }
    best.map(|png| png.to_vec())
}

fn convert_with_sips(path: &Path) -> Option<Vec<u8>> {
    if cfg!(not(target_os = "macos")) {
        return None;
    }
    let output = std::env::temp_dir().join(format!(
        "i18n-workbench-icon-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let status = hidden_command("sips")
        .args(["-z", "128", "128", "-s", "format", "png"])
        .arg(path)
        .arg("--out")
        .arg(&output)
        .status()
        .ok();
    let data = fs::read(&output).ok();
    let _ = fs::remove_file(&output);
    if status.is_none_or(|code| !code.success()) {
        return None;
    }
    data.filter(|png| is_png(png))
}

#[cfg(target_os = "windows")]
fn extract_associated_icon(path: &Path) -> Option<Vec<u8>> {
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Drawing; $icon = [System.Drawing.Icon]::ExtractAssociatedIcon('{escaped}'); if ($null -eq $icon) {{ exit 1 }}; $ms = New-Object System.IO.MemoryStream; $icon.ToBitmap().Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); [Convert]::ToBase64String($ms.ToArray())"
    );
    let output = hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let encoded = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if encoded.is_empty() {
        return None;
    }
    let png = STANDARD.decode(encoded.as_bytes()).ok()?;
    is_png(&png).then_some(png)
}

#[cfg(not(target_os = "windows"))]
fn extract_associated_icon(_path: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::{extract_png_from_icns, extract_png_from_ico, install_root_from_app_dir, is_png};
    use std::path::PathBuf;

    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn png_with_size(width: u32, height: u32) -> Vec<u8> {
        let mut png = TINY_PNG.to_vec();
        png[16..20].copy_from_slice(&width.to_be_bytes());
        png[20..24].copy_from_slice(&height.to_be_bytes());
        png
    }

    #[test]
    fn extracts_preferred_png_chunk_from_icns() {
        let small = png_with_size(32, 32);
        let target = png_with_size(128, 128);
        let mut icns = Vec::from(b"icnsXXXX".as_slice());
        let mut append = |kind: &[u8; 4], payload: &[u8]| {
            let size = 8 + payload.len();
            icns.extend(kind);
            icns.extend(&(size as u32).to_be_bytes());
            icns.extend(payload);
        };
        append(b"ic11", &small);
        append(b"ic07", &target);
        let total = icns.len() as u32;
        icns[4..8].copy_from_slice(&total.to_be_bytes());

        let extracted = extract_png_from_icns(&icns).expect("png chunk");
        assert!(is_png(&extracted));
        assert_eq!(&extracted[16..20], &128u32.to_be_bytes());
    }

    #[test]
    fn extracts_embedded_png_from_ico() {
        let png = png_with_size(128, 128);
        let mut ico = Vec::from([0u8, 0, 1, 0, 1, 0]);
        ico.extend([128, 128, 0, 0, 1, 0, 32, 0]);
        ico.extend(&(png.len() as u32).to_le_bytes());
        ico.extend(&22u32.to_le_bytes());
        ico.extend(&png);
        let extracted = extract_png_from_ico(&ico).expect("png payload");
        assert_eq!(extracted, png);
    }

    #[test]
    fn rejects_non_icon_bytes() {
        assert!(extract_png_from_icns(b"not-an-icon").is_none());
        assert!(extract_png_from_ico(b"xxxx").is_none());
    }

    #[test]
    fn resolves_macos_and_windows_install_roots() {
        let mac = PathBuf::from("/Applications/Cursor.app/Contents/Resources/app");
        assert_eq!(
            install_root_from_app_dir(&mac),
            Some(PathBuf::from("/Applications/Cursor.app"))
        );
        let windows = PathBuf::from("C:/Users/li/AppData/Local/Programs/Cursor/resources/app");
        assert_eq!(
            install_root_from_app_dir(&windows),
            Some(PathBuf::from("C:/Users/li/AppData/Local/Programs/Cursor"))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_installed_cursor_icns_when_present() {
        let path = PathBuf::from("/Applications/Cursor.app/Contents/Resources/Cursor.icns");
        if !path.is_file() {
            return;
        }
        let png = super::decode_icon_file(&path).expect("local Cursor.icns should decode");
        assert!(is_png(&png));
        assert!(super::png_edge(&png).unwrap() >= 32);
        assert!(!png.is_empty());
    }
}
