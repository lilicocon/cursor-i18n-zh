pub mod claude;
pub mod cursor;
mod icons;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const FIRST_RUN_CONSENT_FILE: &str = "first-run-consent";
const FIRST_RUN_CONSENT_VALUE: &str = "accepted";

#[cfg(any(target_os = "macos", test))]
const MACOS_RESIGN_SCRIPT: &str = include_str!("../../../resources/macos/resign-app.sh");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleOption {
    pub id: &'static str,
    pub label: &'static str,
    pub tag: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub installed: bool,
    pub ready: bool,
    pub path: Option<String>,
    pub icon_data_url: Option<String>,
    pub version: Option<String>,
    pub state: String,
    pub state_tone: &'static str,
    pub adapter_version: &'static str,
    pub backup_available: bool,
    pub backup_path: Option<String>,
    pub backup_files: usize,
    pub backup_message: String,
    pub localized: bool,
    pub auto_compatible: bool,
    pub compatibility_message: String,
    pub reason: Option<String>,
    pub locales: Vec<LocaleOption>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequest {
    pub app_id: String,
    pub action: String,
    pub locale: String,
    pub backup_version: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub app_id: String,
    pub app_name: String,
    pub version: String,
    pub created_at_iso: Option<String>,
    pub created_at_unix: Option<u64>,
    pub path: String,
    pub files: usize,
    pub valid: bool,
    pub current: bool,
    pub can_restore: bool,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub app_id: String,
    pub action: String,
    pub percent: u8,
    pub level: &'static str,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub app_id: String,
    pub action: String,
    pub title: String,
    pub message: String,
    pub files_changed: usize,
    pub replacements: usize,
    pub backup_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRuntimeStatus {
    pub installed: bool,
    pub compatible: bool,
    pub version: Option<String>,
    pub executable: Option<String>,
    pub required_version: &'static str,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub platform: &'static str,
    pub is_admin: bool,
    pub data_dir: String,
    pub cursor_engine_path: Option<String>,
    pub node_runtime: NodeRuntimeStatus,
}

#[derive(Clone)]
pub struct ProgressSink {
    app: AppHandle,
    app_id: String,
    action: String,
}

impl ProgressSink {
    pub fn new(app: AppHandle, request: &ActionRequest) -> Self {
        Self {
            app,
            app_id: request.app_id.clone(),
            action: request.action.clone(),
        }
    }

    pub fn emit(&self, percent: u8, level: &'static str, message: impl Into<String>) {
        let _ = self.app.emit(
            "operation-progress",
            ProgressEvent {
                app_id: self.app_id.clone(),
                action: self.action.clone(),
                percent,
                level,
                message: message.into(),
            },
        );
    }
}

pub fn detect_all() -> Vec<AppStatus> {
    vec![cursor::detect(), claude::detect()]
}

pub fn list_backups() -> Vec<BackupRecord> {
    let mut records = cursor::list_backups();
    records.extend(claude::list_backups());
    records.sort_by(|left, right| {
        right
            .created_at_unix
            .cmp(&left.created_at_unix)
            .then_with(|| right.created_at_iso.cmp(&left.created_at_iso))
            .then_with(|| left.app_name.cmp(&right.app_name))
            .then_with(|| right.version.cmp(&left.version))
    });
    records
}

pub fn run_action(request: ActionRequest, sink: ProgressSink) -> Result<OperationResult, String> {
    let app_id = request.app_id.clone();
    let result = match request.app_id.as_str() {
        "cursor" => cursor::run(&request, sink),
        "claude" => claude::run(&request, sink),
        other => Err(format!("不支持的应用适配器: {other}")),
    };
    restore_user_ownership(&local_app_data());
    if app_id == "cursor" {
        if let Some(home) = std::env::var_os("I18N_WORKBENCH_USER_HOME").map(PathBuf::from) {
            restore_user_ownership(&home.join(".cursor"));
            restore_user_ownership(&home.join("Library/Application Support/Cursor"));
        }
    }
    result
}

pub fn environment_status() -> EnvironmentStatus {
    EnvironmentStatus {
        platform: std::env::consts::OS,
        is_admin: is_elevated(),
        data_dir: claude::data_root().to_string_lossy().into_owned(),
        cursor_engine_path: cursor::engine_root().map(|path| path.to_string_lossy().into_owned()),
        node_runtime: cursor::node_runtime_status(),
    }
}

pub fn is_elevated() -> bool {
    is_elevated_platform()
}

#[cfg(target_os = "windows")]
fn is_elevated_platform() -> bool {
    let output = hidden_command("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
        ])
        .output();
    output
        .ok()
        .filter(|result| result.status.success())
        .map(|result| {
            String::from_utf8_lossy(&result.stdout)
                .trim()
                .eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_elevated_platform() -> bool {
    hidden_command("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|result| result.status.success())
        .is_some_and(|result| String::from_utf8_lossy(&result.stdout).trim() == "0")
}

pub fn hidden_command(program: &str) -> std::process::Command {
    let command = std::process::Command::new(program);
    hide_window(command)
}

#[cfg(any(target_os = "macos", test))]
pub fn resign_macos_app(
    app: &std::path::Path,
    label: &str,
    rewrite_team_entitlements: bool,
) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let script = std::env::temp_dir().join(format!(
        "i18n-workbench-resign-{}-{stamp}.sh",
        std::process::id()
    ));
    std::fs::write(&script, MACOS_RESIGN_SCRIPT)
        .map_err(|error| format!("无法准备 {label} 重签名脚本: {error}"))?;
    let output = hidden_command("/bin/bash")
        .arg(&script)
        .arg(app)
        .arg(if rewrite_team_entitlements {
            "true"
        } else {
            "false"
        })
        .arg(label)
        .output();
    let _ = std::fs::remove_file(&script);
    let output = output.map_err(|error| format!("无法启动 {label} 重签名: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = [stdout, stderr]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("; ");
        Err(format!(
            "{label} 重签名失败, exit {:?}: {}",
            output.status.code(),
            if detail.is_empty() {
                "未返回错误详情"
            } else {
                detail.as_str()
            }
        ))
    }
}

#[cfg(windows)]
fn hide_window(mut command: std::process::Command) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
    command
}

#[cfg(not(windows))]
fn hide_window(command: std::process::Command) -> std::process::Command {
    command
}

pub fn local_app_data() -> PathBuf {
    local_app_data_platform()
}

fn first_run_consent_path(dir: &Path) -> PathBuf {
    dir.join(FIRST_RUN_CONSENT_FILE)
}

fn has_first_run_consent_in(dir: &Path) -> bool {
    fs::read_to_string(first_run_consent_path(dir))
        .ok()
        .is_some_and(|value| value.trim() == FIRST_RUN_CONSENT_VALUE)
}

fn accept_first_run_consent_in(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("无法保存首次启动确认: {error}"))?;
    fs::write(first_run_consent_path(dir), FIRST_RUN_CONSENT_VALUE)
        .map_err(|error| format!("无法保存首次启动确认: {error}"))?;
    restore_user_ownership(dir);
    Ok(())
}

pub fn has_first_run_consent() -> bool {
    has_first_run_consent_in(&local_app_data())
}

pub fn accept_first_run_consent() -> Result<(), String> {
    accept_first_run_consent_in(&local_app_data())
}

pub fn require_first_run_consent() -> Result<(), String> {
    if has_first_run_consent() {
        Ok(())
    } else {
        Err("请先阅读并同意软件声明与隐私说明".to_string())
    }
}

#[cfg(target_os = "windows")]
fn local_app_data_platform() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("I18nWorkbench"))
}

#[cfg(target_os = "macos")]
pub fn restore_user_ownership(path: &std::path::Path) {
    if !path.exists() || !is_elevated() {
        return;
    }
    let Some(uid) = std::env::var_os("I18N_WORKBENCH_USER_UID") else {
        return;
    };
    let Some(gid) = std::env::var_os("I18N_WORKBENCH_USER_GID") else {
        return;
    };
    let owner = format!("{}:{}", uid.to_string_lossy(), gid.to_string_lossy());
    let _ = hidden_command("chown")
        .args(["-R", &owner])
        .arg(path)
        .status();
}

#[cfg(not(target_os = "macos"))]
pub fn restore_user_ownership(_path: &std::path::Path) {}

#[cfg(target_os = "macos")]
fn local_app_data_platform() -> PathBuf {
    std::env::var_os("I18N_WORKBENCH_USER_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support/I18nWorkbench"))
        .unwrap_or_else(|| std::env::temp_dir().join("I18nWorkbench"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn local_app_data_platform() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/share"))
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("I18nWorkbench")
}

#[cfg(test)]
mod tests {
    use super::{
        accept_first_run_consent_in, has_first_run_consent_in, resign_macos_app, MACOS_RESIGN_SCRIPT,
    };

    #[test]
    fn first_run_consent_file_is_required_before_local_privileges() {
        let root = std::env::temp_dir().join(format!(
            "i18n-workbench-consent-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        assert!(!has_first_run_consent_in(&root));
        accept_first_run_consent_in(&root).unwrap();
        assert!(has_first_run_consent_in(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn macos_resign_script_preserves_nested_code_and_critical_entitlements() {
        let _signer: fn(&std::path::Path, &str, bool) -> Result<(), String> = resign_macos_app;
        assert!(MACOS_RESIGN_SCRIPT.contains("find \"$CONTENTS\" -depth -type f"));
        assert!(MACOS_RESIGN_SCRIPT.contains("grep -q 'Mach-O'"));
        assert!(MACOS_RESIGN_SCRIPT.contains("*.framework"));
        assert!(MACOS_RESIGN_SCRIPT.contains("com.apple.security.virtualization"));
        assert!(MACOS_RESIGN_SCRIPT.contains("com.apple.security.cs.disable-library-validation"));
        assert!(MACOS_RESIGN_SCRIPT.contains("codesign --verify --deep --strict"));
    }
}
