use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::adapters;
use crate::chats::{self, StuckChat};

const POLL_ATTEMPTS: u32 = 8;
const POLL_INTERVAL_MS: u64 = 250;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorProcess {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub role: String,
    pub memory_kb: u64,
    pub command: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSessionOverview {
    pub running: bool,
    pub occupied: bool,
    pub remote_control_running: bool,
    pub main_count: u32,
    pub process_count: u32,
    pub status: String,
    pub detail: String,
    pub launch_path: Option<String>,
    pub processes: Vec<CursorProcess>,
    pub chats: Vec<StuckChat>,
    pub stuck_chat_count: u32,
    pub chat_error: Option<String>,
    pub chat_backup_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionActionRequest {
    pub action: String,
}

pub fn load_cursor_sessions() -> Result<CursorSessionOverview, String> {
    let mut overview = summarize(collect_processes()?, cursor_launch_path());
    let inspection = chats::inspect_stuck_chats();
    overview.chats = inspection.chats;
    overview.stuck_chat_count = overview.chats.iter().filter(|chat| chat.can_detach).count() as u32;
    overview.chat_error = inspection.error;
    if overview.stuck_chat_count > 0 {
        overview.detail = format!(
            "{}. 另有 {} 条对话被标成 Cloud Agent 且已卡住或标错, 可在 Cursor 退出后解除标记改回本地.",
            overview.detail, overview.stuck_chat_count
        );
    } else if overview.chats.iter().any(|chat| chat.cloud_bound) {
        overview.detail = format!(
            "{}. 本机索引里还有对话仍标成 Cloud Agent; 还在跑的云端任务先不要改.",
            overview.detail
        );
    }
    Ok(overview)
}

pub fn manage_cursor_session(request: SessionActionRequest) -> Result<CursorSessionOverview, String> {
    match request.action.as_str() {
        "refresh" => {}
        "quit" => quit_cursor(false)?,
        "kill-tree" => quit_cursor(true)?,
        "kill-remote" => kill_remote_workers()?,
        "start" => start_cursor()?,
        "detach-chats" => return detach_chats(),
        other => return Err(format!("不支持的会话操作: {other}")),
    }
    load_cursor_sessions()
}

fn detach_chats() -> Result<CursorSessionOverview, String> {
    if !collect_processes()?.is_empty() {
        return Err("请先结束全部 Cursor 进程, 再解除 Cloud Agent 标记. 状态库被占用时改写不会生效".to_string());
    }
    let inspection = chats::detach_stuck_chats()?;
    let mut overview = summarize(collect_processes()?, cursor_launch_path());
    overview.chats = inspection.chats;
    overview.stuck_chat_count = overview.chats.iter().filter(|chat| chat.can_detach).count() as u32;
    overview.chat_error = inspection.error;
    overview.chat_backup_path = inspection.backup_path;
    if overview.chat_backup_path.is_some() {
        overview.detail = format!(
            "已把标错或卡住的对话改回本地索引, 并备份状态库到 {}. 请重新打开同一工作区; 原对话可继续, 不会自动恢复成远程控制.",
            overview.chat_backup_path.as_deref().unwrap_or("--")
        );
    }
    Ok(overview)
}

fn summarize(mut processes: Vec<CursorProcess>, launch_path: Option<PathBuf>) -> CursorSessionOverview {
    processes.sort_by(|left, right| {
        role_rank(&left.role)
            .cmp(&role_rank(&right.role))
            .then_with(|| left.pid.cmp(&right.pid))
    });
    let main_count = processes
        .iter()
        .filter(|process| process.role == "main")
        .count() as u32;
    let remote_control_running = processes
        .iter()
        .any(|process| process.role == "remote-control");
    let occupied = remote_control_running || main_count > 1;
    let running = !processes.is_empty();
    let (status, detail) = if remote_control_running {
        (
            "远程控制占用".to_string(),
            "检测到远程控制或后台 Agent 工作进程仍在运行. 同时只允许一个工作区占用远程控制, 可先结束工作进程再重试.".to_string(),
        )
    } else if main_count > 1 {
        (
            "多窗口占用".to_string(),
            "检测到多个 Cursor 主进程. 远程控制会绑到先注册的工作区, 多余窗口容易把会话卡在占用状态.".to_string(),
        )
    } else if running {
        (
            "运行中".to_string(),
            "本机 Cursor 进程正常. 若网页或手机端仍显示占用, 先只打开目标工作区, 再重新开启远程控制.".to_string(),
        )
    } else {
        (
            "未运行".to_string(),
            "未检测到 Cursor 或远程控制工作进程. 可以直接启动 Cursor, 或确认远程控制已在设置中关闭.".to_string(),
        )
    };
    CursorSessionOverview {
        running,
        occupied,
        remote_control_running,
        main_count,
        process_count: processes.len() as u32,
        status,
        detail,
        launch_path: launch_path.map(|path| path.to_string_lossy().into_owned()),
        processes,
        chats: Vec::new(),
        stuck_chat_count: 0,
        chat_error: None,
        chat_backup_path: None,
    }
}

fn role_rank(role: &str) -> u8 {
    match role {
        "remote-control" => 0,
        "main" => 1,
        "helper" => 2,
        "gpu" => 3,
        "renderer" => 4,
        "plugin" => 5,
        _ => 6,
    }
}

fn classify_role(name: &str, command: &str) -> &'static str {
    let hay = format!("{name} {command}").to_ascii_lowercase();
    if hay.contains("remote-control")
        || hay.contains("remote_control")
        || hay.contains("remote control")
        || hay.contains("cursor-agent")
        || hay.contains("cloud-agent")
        || hay.contains("self-hosted")
        || hay.contains("background composer")
        || hay.contains("background-composer")
    {
        return "remote-control";
    }
    if hay.contains("gpu") {
        return "gpu";
    }
    if hay.contains("renderer") {
        return "renderer";
    }
    if hay.contains("plugin") {
        return "plugin";
    }
    if hay.contains("helper") {
        return "helper";
    }
    if process_name_is_main(name) {
        return "main";
    }
    "other"
}

fn process_name_is_main(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    stem.eq_ignore_ascii_case("cursor")
}

fn is_cursor_process(name: &str, command: &str) -> bool {
    let hay = format!("{name} {command}").to_ascii_lowercase();
    hay.contains("cursor") || hay.contains("cursor-agent")
}

fn collect_processes() -> Result<Vec<CursorProcess>, String> {
    collect_processes_platform()
}

#[cfg(target_os = "windows")]
fn collect_processes_platform() -> Result<Vec<CursorProcess>, String> {
    if let Ok(output) = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"Get-CimInstance Win32_Process | Where-Object { $_.Name -match 'Cursor|cursor-agent' } | ForEach-Object { $cmd = [string]($_.CommandLine); $cmd = $cmd -replace '[\t\r\n]+',' '; if ($cmd.Length -gt 240) { $cmd = $cmd.Substring(0, 240) }; '{0}{1}{2}{1}{3}{1}{4}{1}{5}' -f $_.ProcessId, [char]9, $_.ParentProcessId, $_.Name, $_.WorkingSetSize, $cmd }"#,
        ],
    ) {
        let parsed = parse_delimited_processes(&output, true);
        if !parsed.is_empty() || output.trim().is_empty() {
            return Ok(parsed);
        }
    }
    let output = run_hidden("tasklist.exe", &["/FO", "CSV", "/NH"])?;
    Ok(parse_tasklist_processes(&output))
}

#[cfg(not(target_os = "windows"))]
fn collect_processes_platform() -> Result<Vec<CursorProcess>, String> {
    let output = run_hidden("ps", &["-axo", "pid=,ppid=,rss=,command="])?;
    Ok(parse_ps_processes(&output))
}

fn parse_delimited_processes(output: &str, memory_is_bytes: bool) -> Vec<CursorProcess> {
    output
        .lines()
        .filter_map(|line| {
            let columns = line.split('\t').collect::<Vec<_>>();
            if columns.len() < 4 {
                return None;
            }
            let pid = columns[0].trim().parse().ok()?;
            let ppid = columns[1].trim().parse().ok();
            let name = columns[2].trim().to_string();
            let memory_raw = columns[3].trim().parse::<u64>().unwrap_or(0);
            let command = columns.get(4).map(|value| value.trim().to_string()).unwrap_or_default();
            if !is_cursor_process(&name, &command) {
                return None;
            }
            Some(CursorProcess {
                pid,
                ppid,
                role: classify_role(&name, &command).to_string(),
                memory_kb: if memory_is_bytes {
                    memory_raw / 1024
                } else {
                    memory_raw
                },
                name,
                command,
            })
        })
        .collect()
}

fn parse_tasklist_processes(output: &str) -> Vec<CursorProcess> {
    output
        .lines()
        .filter_map(|line| {
            let columns = parse_csv_line(line);
            if columns.len() < 5 {
                return None;
            }
            let name = columns[0].trim_matches('"').to_string();
            if !is_cursor_process(&name, "") {
                return None;
            }
            let pid = columns[1].trim_matches('"').parse().ok()?;
            let memory = columns[4]
                .trim_matches('"')
                .replace(',', "")
                .replace(" K", "")
                .replace(' ', "")
                .parse::<u64>()
                .unwrap_or(0);
            Some(CursorProcess {
                pid,
                ppid: None,
                role: classify_role(&name, "").to_string(),
                memory_kb: memory,
                name,
                command: String::new(),
            })
        })
        .collect()
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                values.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    values.push(current);
    values
}

fn parse_ps_processes(output: &str) -> Vec<CursorProcess> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok();
            let memory_kb = parts.next()?.parse().unwrap_or(0);
            let command = parts.collect::<Vec<_>>().join(" ");
            let name = Path::new(command.split_whitespace().next().unwrap_or("Cursor"))
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Cursor")
                .to_string();
            if !is_cursor_process(&name, &command) {
                return None;
            }
            Some(CursorProcess {
                pid,
                ppid,
                role: classify_role(&name, &command).to_string(),
                memory_kb,
                name,
                command,
            })
        })
        .collect()
}

fn quit_cursor(force: bool) -> Result<(), String> {
    if force {
        force_kill_cursor()?;
    } else {
        graceful_quit_cursor()?;
    }
    wait_until_idle(force)
}

fn kill_remote_workers() -> Result<(), String> {
    let remotes = collect_processes()?
        .into_iter()
        .filter(|process| process.role == "remote-control")
        .collect::<Vec<_>>();
    if remotes.is_empty() {
        return Err("未发现远程控制工作进程. 若会话仍卡住, 可结束全部 Cursor 后只打开一个工作区".to_string());
    }
    for process in remotes {
        kill_pid(process.pid)?;
    }
    wait_until(|processes| processes.iter().all(|process| process.role != "remote-control"))
}

fn start_cursor() -> Result<(), String> {
    if collect_processes()?.iter().any(|process| process.role == "main") {
        return Ok(());
    }
    start_cursor_platform()
}

#[cfg(target_os = "windows")]
fn graceful_quit_cursor() -> Result<(), String> {
    let _ = run_hidden("taskkill.exe", &["/IM", "Cursor.exe"]);
    let _ = run_hidden("taskkill.exe", &["/IM", "cursor-agent.exe"]);
    Ok(())
}

#[cfg(target_os = "windows")]
fn force_kill_cursor() -> Result<(), String> {
    let _ = run_hidden("taskkill.exe", &["/IM", "Cursor.exe", "/T", "/F"]);
    let _ = run_hidden("taskkill.exe", &["/IM", "cursor-agent.exe", "/T", "/F"]);
    Ok(())
}

#[cfg(target_os = "windows")]
fn kill_pid(pid: u32) -> Result<(), String> {
    run_hidden("taskkill.exe", &["/PID", &pid.to_string(), "/T", "/F"]).map(|_| ())
}

#[cfg(target_os = "windows")]
fn start_cursor_platform() -> Result<(), String> {
    let exe = cursor_launch_path().ok_or_else(|| "未找到 Cursor 可执行文件".to_string())?;
    adapters::hidden_command("cmd")
        .args(["/C", "start", "", &exe.to_string_lossy()])
        .status()
        .map_err(|error| format!("无法启动 Cursor: {error}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn graceful_quit_cursor() -> Result<(), String> {
    let _ = run_hidden("osascript", &["-e", "tell application \"Cursor\" to quit"]);
    Ok(())
}

#[cfg(target_os = "macos")]
fn force_kill_cursor() -> Result<(), String> {
    let _ = run_hidden("pkill", &["-9", "-x", "Cursor"]);
    let _ = run_hidden("pkill", &["-9", "-x", "Cursor Helper"]);
    let _ = run_hidden("pkill", &["-9", "-f", "cursor-agent"]);
    Ok(())
}

#[cfg(target_os = "macos")]
fn kill_pid(pid: u32) -> Result<(), String> {
    run_hidden("kill", &["-9", &pid.to_string()]).map(|_| ())
}

#[cfg(target_os = "macos")]
fn start_cursor_platform() -> Result<(), String> {
    let output = adapters::hidden_command("open")
        .args(["-a", "Cursor"])
        .output()
        .map_err(|error| format!("无法启动 Cursor: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "无法启动 Cursor, 请确认已安装应用程序".to_string()
        } else {
            format!("无法启动 Cursor: {detail}")
        })
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn graceful_quit_cursor() -> Result<(), String> {
    force_kill_cursor()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn force_kill_cursor() -> Result<(), String> {
    let _ = run_hidden("pkill", &["-f", "/[Cc]ursor"]);
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn kill_pid(pid: u32) -> Result<(), String> {
    run_hidden("kill", &["-9", &pid.to_string()]).map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn start_cursor_platform() -> Result<(), String> {
    let exe = cursor_launch_path().ok_or_else(|| "未找到 Cursor 可执行文件".to_string())?;
    adapters::hidden_command(&exe.to_string_lossy())
        .spawn()
        .map_err(|error| format!("无法启动 Cursor: {error}"))?;
    Ok(())
}

fn wait_until_idle(force: bool) -> Result<(), String> {
    wait_until(|processes| processes.is_empty()).or_else(|error| {
        if force {
            Err(error)
        } else {
            force_kill_cursor()?;
            wait_until(|processes| processes.is_empty())
        }
    })
}

fn wait_until(idle: impl Fn(&[CursorProcess]) -> bool) -> Result<(), String> {
    for _ in 0..POLL_ATTEMPTS {
        let processes = collect_processes()?;
        if idle(&processes) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
    Err("Cursor 进程仍未完全退出. 可改用结束全部进程, 或重新打开并授权后再试".to_string())
}

fn cursor_launch_path() -> Option<PathBuf> {
    cursor_launch_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn cursor_launch_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        candidates.push(local.join("Programs/Cursor/Cursor.exe"));
    }
    if let Some(program) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
        candidates.push(program.join("Cursor/Cursor.exe"));
    }
    candidates
}

#[cfg(target_os = "macos")]
fn cursor_launch_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("/Applications/Cursor.app/Contents/MacOS/Cursor")];
    if let Some(home) = std::env::var_os("I18N_WORKBENCH_USER_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        candidates.push(home.join("Applications/Cursor.app/Contents/MacOS/Cursor"));
    }
    candidates
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn cursor_launch_candidates() -> Vec<PathBuf> {
    ["/usr/bin/cursor", "/usr/share/cursor/cursor", "/opt/Cursor/cursor"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn run_hidden(program: &str, args: &[&str]) -> Result<String, String> {
    let output = adapters::hidden_command(program)
        .args(args)
        .output()
        .map_err(|error| format!("无法执行 {program}: {error}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_remote_control_and_main_processes() {
        assert_eq!(
            classify_role("node", "/usr/local/bin/cursor-agent --remote-control"),
            "remote-control"
        );
        assert_eq!(classify_role("Cursor Helper (GPU)", ""), "gpu");
        assert_eq!(classify_role("Cursor.exe", "C:\\Users\\li\\AppData\\Local\\Programs\\Cursor\\Cursor.exe"), "main");
    }

    #[test]
    fn summarizes_occupied_remote_control() {
        let overview = summarize(
            vec![
                CursorProcess {
                    pid: 11,
                    ppid: Some(1),
                    name: "Cursor".into(),
                    role: "main".into(),
                    memory_kb: 100,
                    command: "/Applications/Cursor.app/Contents/MacOS/Cursor".into(),
                },
                CursorProcess {
                    pid: 22,
                    ppid: Some(1),
                    name: "cursor-agent".into(),
                    role: "remote-control".into(),
                    memory_kb: 40,
                    command: "cursor-agent --remote-control".into(),
                },
            ],
            None,
        );
        assert!(overview.occupied);
        assert!(overview.remote_control_running);
        assert_eq!(overview.status, "远程控制占用");
        assert_eq!(overview.processes[0].role, "remote-control");
        assert!(overview.chats.is_empty());
    }

    #[test]
    fn parses_ps_and_tasklist_rows() {
        let ps = parse_ps_processes(
            "  100  1  2048 /Applications/Cursor.app/Contents/MacOS/Cursor\n  101  100  512 /Applications/Cursor.app/Contents/Frameworks/Cursor Helper (GPU).app/Contents/MacOS/Cursor Helper (GPU)\n  9  1  10 /usr/bin/unrelated\n",
        );
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].role, "main");
        assert_eq!(ps[1].role, "gpu");

        let tasklist = parse_tasklist_processes(
            "\"Cursor.exe\",\"321\",\"Console\",\"1\",\"120,000 K\"\n\"notepad.exe\",\"8\",\"Console\",\"1\",\"1 K\"\n",
        );
        assert_eq!(tasklist.len(), 1);
        assert_eq!(tasklist[0].pid, 321);
        assert_eq!(tasklist[0].memory_kb, 120000);
    }
}
