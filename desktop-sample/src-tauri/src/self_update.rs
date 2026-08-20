use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::adapters::local_app_data;
use crate::release::UpdateDownloadResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateResult {
    pub restarting: bool,
    pub path: String,
    pub version: String,
    pub sha256: String,
    pub cached: bool,
    pub fallback: bool,
    pub message: String,
}

pub fn apply_downloaded_update(downloaded: &UpdateDownloadResult) -> InstallUpdateResult {
    let (can, reason) = self_update_capability();
    if !can {
        return fallback_result(downloaded, reason);
    }

    match stage_and_schedule(downloaded) {
        Ok(()) => InstallUpdateResult {
            restarting: true,
            path: downloaded.path.clone(),
            version: downloaded.version.clone(),
            sha256: downloaded.sha256.clone(),
            cached: downloaded.cached,
            fallback: false,
            message: "更新已就绪，即将重启工作台".to_string(),
        },
        Err(error) => fallback_result(
            downloaded,
            format!("{error}。已改为打开更新包所在文件夹"),
        ),
    }
}

pub fn self_update_capability() -> (bool, String) {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        return (
            false,
            "当前系统没有官方安装包，下载后请手动替换".to_string(),
        );
    }

    #[cfg(any(windows, target_os = "macos"))]
    match find_install_root() {
        Ok(root) => {
            if !install_dir_writable(&root) {
                return (
                    false,
                    "安装目录不可写。下载后会打开文件夹，请手动替换，或用管理员权限运行后再更新"
                        .to_string(),
                );
            }
            (true, String::new())
        }
        Err(reason) => (false, reason),
    }
}

fn fallback_result(downloaded: &UpdateDownloadResult, message: String) -> InstallUpdateResult {
    InstallUpdateResult {
        restarting: false,
        path: downloaded.path.clone(),
        version: downloaded.version.clone(),
        sha256: downloaded.sha256.clone(),
        cached: downloaded.cached,
        fallback: true,
        message,
    }
}

fn stage_and_schedule(downloaded: &UpdateDownloadResult) -> Result<(), String> {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = downloaded;
        return Err("当前系统不支持自动安装".to_string());
    }

    #[cfg(any(windows, target_os = "macos"))]
    {
        let install_root = find_install_root()?;
        let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let current_pid = std::process::id();
        let work = update_work_dir(&downloaded.version)?;
        let payload = work.join("payload");
        if payload.exists() {
            fs::remove_dir_all(&payload).map_err(|error| format!("无法清空更新目录：{error}"))?;
        }
        fs::create_dir_all(&payload).map_err(|error| format!("无法创建更新目录：{error}"))?;

        let package = PathBuf::from(&downloaded.path);
        #[cfg(windows)]
        {
            extract_windows_zip(&package, &payload)?;
            reject_payload_symlinks(&payload)?;
            let new_exe = find_windows_payload_exe(&payload)?;
            let new_name = file_name(&new_exe)?;
            let old_name = file_name(&current_exe)?;
            let helper = write_windows_helper(
                &work,
                current_pid,
                &payload,
                &install_root,
                &new_name,
                &old_name,
            )?;
            spawn_windows_helper(&helper)?;
        }
        #[cfg(target_os = "macos")]
        {
            let _ = current_exe;
            extract_macos_dmg(&package, &payload)?;
            let staged_app = find_macos_payload_app(&payload)?;
            reject_payload_symlinks(&staged_app)?;
            let helper = write_macos_helper(&work, current_pid, &staged_app, &install_root)?;
            spawn_unix_helper(&helper)?;
        }
        Ok(())
    }
}

fn update_work_dir(version: &str) -> Result<PathBuf, String> {
    let dir = local_app_data()
        .join("updates")
        .join(format!("v{}", version.trim()));
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建更新目录：{error}"))?;
    Ok(dir)
}

#[cfg(any(windows, target_os = "macos"))]
fn find_install_root() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        let name = file_name(&exe)?;
        if name.eq_ignore_ascii_case("cursor-i18n-desktop-sample.exe") {
            return Err("当前是开发版 exe，不能覆盖安装包。请先用正式版 zip 里的工作台".to_string());
        }
        if !name.to_ascii_lowercase().starts_with("localization-workbench")
            || !name.to_ascii_lowercase().ends_with(".exe")
        {
            return Err("未识别当前工作台安装位置，下载后请手动替换".to_string());
        }
        exe.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "未找到安装目录".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let app = find_app_bundle(&exe)?;
        if app.starts_with("/Volumes/") {
            return Err("请先把「汉化工作台」拖到「应用程序」文件夹，再在应用内更新".to_string());
        }
        Ok(app)
    }
}

#[cfg(target_os = "macos")]
fn find_app_bundle(start: &Path) -> Result<PathBuf, String> {
    let mut current = start;
    loop {
        if current.extension().and_then(|value| value.to_str()) == Some("app") {
            return Ok(current.to_path_buf());
        }
        current = current
            .parent()
            .ok_or_else(|| "未找到 .app 安装位置，下载后请手动替换".to_string())?;
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn install_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(".workbench-update-write-probe");
    match fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| "路径无效".to_string())
}

pub(crate) fn reject_zip_entry(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("更新包含空路径".to_string());
    }
    if trimmed.contains('\0') {
        return Err("更新包含非法路径".to_string());
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with("~/") {
        return Err("更新包含非法路径".to_string());
    }
    if normalized.split('/').any(|part| part == "..") {
        return Err("更新包含越界路径".to_string());
    }
    Ok(())
}

fn reject_payload_symlinks(root: &Path) -> Result<(), String> {
    reject_payload_symlinks_inner(root, root)
}

fn reject_payload_symlinks_inner(root: &Path, current: &Path) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|error| format!("无法读取更新目录：{error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("更新包含符号链接，已拒绝安装".to_string());
        }
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            reject_zip_entry(name)?;
        }
        if let (Ok(root_canon), Ok(path_canon)) = (root.canonicalize(), path.canonicalize()) {
            if !path_canon.starts_with(&root_canon) {
                return Err("更新包含越界路径".to_string());
            }
        }
        if metadata.is_dir() {
            reject_payload_symlinks_inner(root, &path)?;
        }
    }
    Ok(())
}

pub(crate) fn find_windows_payload_exe(payload: &Path) -> Result<PathBuf, String> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(payload).map_err(|error| format!("无法读取更新目录：{error}"))? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            if name.starts_with("localization-workbench") && name.ends_with(".exe") {
                matches.push(path);
            }
        }
    }
    if matches.len() != 1 {
        return Err("更新包里没有唯一的工作台 exe".to_string());
    }
    Ok(matches.remove(0))
}

pub(crate) fn find_macos_payload_app(payload: &Path) -> Result<PathBuf, String> {
    let preferred = payload.join("汉化工作台.app");
    if preferred.is_dir() {
        return Ok(preferred);
    }
    let mut matches = Vec::new();
    for entry in fs::read_dir(payload).map_err(|error| format!("无法读取更新目录：{error}"))? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("app") && path.is_dir() {
            matches.push(path);
        }
    }
    if matches.len() != 1 {
        return Err("更新包里没有唯一的工作台 .app".to_string());
    }
    Ok(matches.remove(0))
}

#[cfg(windows)]
fn extract_windows_zip(zip: &Path, dest: &Path) -> Result<(), String> {
    validate_windows_zip_entries(zip)?;
    let status = crate::adapters::hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
                ps_single_quote(zip),
                ps_single_quote(dest)
            ),
        ])
        .status()
        .map_err(|error| format!("无法解压更新包：{error}"))?;
    if !status.success() {
        return Err("解压更新包失败".to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn validate_windows_zip_entries(zip: &Path) -> Result<(), String> {
    let output = crate::adapters::hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Add-Type -AssemblyName System.IO.Compression.FileSystem; $z = [IO.Compression.ZipFile]::OpenRead({}); try {{ $z.Entries | ForEach-Object {{ $_.FullName }} }} finally {{ $z.Dispose() }}",
                ps_single_quote(zip)
            ),
        ])
        .output()
        .map_err(|error| format!("无法读取更新包目录：{error}"))?;
    if !output.status.success() {
        return Err("无法读取更新包目录".to_string());
    }
    let names = String::from_utf8_lossy(&output.stdout);
    let mut count = 0;
    for name in names.lines().map(str::trim).filter(|name| !name.is_empty()) {
        reject_zip_entry(name)?;
        count += 1;
    }
    if count == 0 {
        return Err("更新包为空".to_string());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn extract_macos_dmg(dmg: &Path, dest: &Path) -> Result<(), String> {
    let mount = dest
        .parent()
        .ok_or_else(|| "更新目录无效".to_string())?
        .join("dmg-mount");
    if mount.exists() {
        let _ = std::process::Command::new("hdiutil")
            .args(["detach", &path_str(&mount), "-quiet", "-force"])
            .status();
        let _ = fs::remove_dir_all(&mount);
    }
    fs::create_dir_all(&mount).map_err(|error| format!("无法创建挂载点：{error}"))?;
    let attached = std::process::Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-mountpoint",
            &path_str(&mount),
            &path_str(dmg),
        ])
        .status()
        .map_err(|error| format!("无法挂载更新包：{error}"))?;
    let copied = (|| {
        if !attached.success() {
            return Err("挂载更新包失败".to_string());
        }
        let app = find_macos_payload_app(&mount)?;
        let staged = dest.join("汉化工作台.app");
        let status = std::process::Command::new("ditto")
            .args([app.as_os_str(), staged.as_os_str()])
            .status()
            .map_err(|error| format!("无法复制应用：{error}"))?;
        if !status.success() {
            return Err("复制应用失败".to_string());
        }
        Ok(())
    })();
    let _ = std::process::Command::new("hdiutil")
        .args(["detach", &path_str(&mount), "-quiet", "-force"])
        .status();
    copied
}

fn ps_single_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn sh_single_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn windows_helper_script(
    pid: u32,
    payload: &Path,
    install_dir: &Path,
    new_exe: &str,
    old_exe: &str,
) -> String {
    let delete_old = if old_exe != new_exe {
        format!(
            "$old = Join-Path $dst {}\nif (Test-Path -LiteralPath $old) {{ Remove-Item -LiteralPath $old -Force }}\n",
            ps_single_quote(Path::new(old_exe))
        )
    } else {
        String::new()
    };
    format!(
        "$appPid = {pid}\n$src = {}\n$dst = {}\n$exe = {}\nStart-Sleep -Seconds 1\n$deadline = (Get-Date).AddSeconds(90)\nwhile ((Get-Process -Id $appPid -ErrorAction SilentlyContinue) -and ((Get-Date) -lt $deadline)) {{ Start-Sleep -Milliseconds 400 }}\nif (Get-Process -Id $appPid -ErrorAction SilentlyContinue) {{ exit 2 }}\n{delete_old}Copy-Item -Path (Join-Path $src '*') -Destination $dst -Recurse -Force\nStart-Process -FilePath (Join-Path $dst $exe)\n",
        ps_single_quote(payload),
        ps_single_quote(install_dir),
        ps_single_quote(Path::new(new_exe)),
    )
}

pub(crate) fn macos_helper_script(pid: u32, staged_app: &Path, install_app: &Path) -> String {
    format!(
        "#!/bin/bash\nset -euo pipefail\npid={pid}\nsrc={}\ndst={}\nsleep 1\nfor _ in $(seq 1 225); do\n  if ! kill -0 \"$pid\" 2>/dev/null; then\n    break\n  fi\n  sleep 0.4\ndone\nif kill -0 \"$pid\" 2>/dev/null; then\n  exit 2\nfi\nditto \"$src\" \"$dst\"\nopen \"$dst\"\n",
        sh_single_quote(staged_app),
        sh_single_quote(install_app),
    )
}

#[cfg(windows)]
fn write_windows_helper(
    work: &Path,
    pid: u32,
    payload: &Path,
    install_dir: &Path,
    new_exe: &str,
    old_exe: &str,
) -> Result<PathBuf, String> {
    let path = work.join("apply-update.ps1");
    fs::write(
        &path,
        windows_helper_script(pid, payload, install_dir, new_exe, old_exe),
    )
    .map_err(|error| format!("无法写入更新脚本：{error}"))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn write_macos_helper(
    work: &Path,
    pid: u32,
    staged_app: &Path,
    install_app: &Path,
) -> Result<PathBuf, String> {
    let path = work.join("apply-update.sh");
    fs::write(&path, macos_helper_script(pid, staged_app, install_app))
        .map_err(|error| format!("无法写入更新脚本：{error}"))?;
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(windows)]
fn spawn_windows_helper(script: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map_err(|error| format!("无法启动更新程序：{error}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_unix_helper(script: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    std::process::Command::new("bash")
        .arg(script)
        .process_group(0)
        .spawn()
        .map_err(|error| format!("无法启动更新程序：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("workbench-self-update-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn rejects_zip_slip_and_absolute_entries() {
        assert!(reject_zip_entry("../evil.exe").is_err());
        assert!(reject_zip_entry("foo/../../evil.exe").is_err());
        assert!(reject_zip_entry("foo\\..\\evil.exe").is_err());
        assert!(reject_zip_entry("/tmp/evil.exe").is_err());
        assert!(reject_zip_entry("localization-workbench.exe").is_ok());
        assert!(reject_zip_entry("dict/00-common.json").is_ok());
    }

    #[test]
    fn finds_flat_windows_payload_exe() {
        let dir = temp_dir();
        fs::write(
            dir.join("localization-workbench-v0.4.8-windows-x64.exe"),
            b"exe",
        )
        .unwrap();
        fs::write(dir.join("cli.js"), b"js").unwrap();
        fs::create_dir_all(dir.join("dict")).unwrap();
        let found = find_windows_payload_exe(&dir).unwrap();
        assert!(file_name(&found)
            .unwrap()
            .starts_with("localization-workbench"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_missing_or_ambiguous_windows_exe() {
        let dir = temp_dir();
        assert!(find_windows_payload_exe(&dir).is_err());
        fs::write(dir.join("localization-workbench-a.exe"), b"a").unwrap();
        fs::write(dir.join("localization-workbench-b.exe"), b"b").unwrap();
        assert!(find_windows_payload_exe(&dir).is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn finds_macos_app_payload() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("汉化工作台.app/Contents")).unwrap();
        let found = find_macos_payload_app(&dir).unwrap();
        assert_eq!(file_name(&found).unwrap(), "汉化工作台.app");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn update_work_dir_reuses_local_app_data() {
        let dir = update_work_dir("9.9.9-self-update-test").unwrap();
        assert_eq!(
            dir,
            local_app_data().join("updates").join("v9.9.9-self-update-test")
        );
        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn linux_falls_back_to_manual_replace() {
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let (can, reason) = self_update_capability();
            assert!(!can);
            assert!(reason.contains("手动替换"));
        }
    }

    #[test]
    fn windows_helper_contains_pid_and_replaces_old_exe() {
        let script = windows_helper_script(
            4242,
            Path::new("C:\\updates\\payload"),
            Path::new("C:\\app"),
            "localization-workbench-v0.4.8-windows-x64.exe",
            "localization-workbench-v0.4.7-windows-x64.exe",
        );
        assert!(script.contains("$appPid = 4242"));
        assert!(script.contains("Remove-Item"));
        assert!(script.contains("localization-workbench-v0.4.8-windows-x64.exe"));
        assert!(script.contains("Copy-Item"));
        assert!(script.contains("Start-Process"));
    }

    #[test]
    fn windows_helper_keeps_same_exe_name() {
        let script = windows_helper_script(
            7,
            Path::new("C:\\updates\\payload"),
            Path::new("C:\\app"),
            "localization-workbench-v0.4.8-windows-x64.exe",
            "localization-workbench-v0.4.8-windows-x64.exe",
        );
        assert!(!script.contains("Remove-Item"));
        assert!(script.contains("Copy-Item"));
    }

    #[test]
    fn macos_helper_waits_then_ditto_and_open() {
        let script = macos_helper_script(
            99,
            Path::new("/tmp/payload/汉化工作台.app"),
            Path::new("/Applications/汉化工作台.app"),
        );
        assert!(script.contains("pid=99"));
        assert!(script.contains("ditto"));
        assert!(script.contains("open"));
        assert!(script.contains("/Applications/汉化工作台.app"));
    }
}
