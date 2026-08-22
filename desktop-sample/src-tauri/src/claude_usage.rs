use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::usage::{parse_iso_millis, utc_date, value_f64, value_u64};

const EVENT_LIMIT: usize = 200;
const WALK_DEPTH: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Desktop,
    Cli,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Cli => "cli",
        }
    }
}

struct TokenHit {
    timestamp_ms: u64,
    model: String,
    source: Source,
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
}

impl TokenHit {
    fn tokens(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeModelUsage {
    pub name: String,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub tokens: u64,
    pub desktop_tokens: u64,
    pub cli_tokens: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDailyUsage {
    pub date: String,
    pub desktop_requests: u64,
    pub cli_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tokens: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageEvent {
    pub timestamp_ms: u64,
    pub model: String,
    pub source: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tokens: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageOverview {
    pub available: bool,
    pub account_email: Option<String>,
    pub five_hour_percent: Option<f64>,
    pub seven_day_percent: Option<f64>,
    pub five_hour_reset: Option<String>,
    pub seven_day_reset: Option<String>,
    pub request_total: u64,
    pub token_total: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub desktop_sessions: u64,
    pub cli_sessions: u64,
    pub desktop_tokens: u64,
    pub cli_tokens: u64,
    pub models: Vec<ClaudeModelUsage>,
    pub days: Vec<ClaudeDailyUsage>,
    pub events: Vec<ClaudeUsageEvent>,
    pub event_total: u64,
    pub event_truncated: bool,
    pub notes: Vec<String>,
    pub refreshed_at_unix: u64,
}

pub fn load_claude_usage() -> Result<ClaudeUsageOverview, String> {
    Ok(collect_from_roots(
        &claude_config_roots(),
        &desktop_data_roots(),
        &claude_settings_files(),
    ))
}

fn collect_from_roots(
    config_roots: &[PathBuf],
    desktop_roots: &[PathBuf],
    settings_files: &[PathBuf],
) -> ClaudeUsageOverview {
    let desktop_ids = index_desktop_sessions(desktop_roots);
    let mut seen_files = HashSet::new();
    let mut seen_turns = HashSet::new();
    let mut hits = Vec::new();
    let mut desktop_sessions = HashSet::new();
    let mut cli_sessions = HashSet::new();

    for root in config_roots {
        for path in walk_files(&root.join("projects"), WALK_DEPTH, is_jsonl) {
            collect_jsonl(
                &path,
                session_source(&path, &desktop_ids),
                &mut seen_files,
                &mut seen_turns,
                &mut hits,
                &mut desktop_sessions,
                &mut cli_sessions,
            );
        }
    }
    for root in desktop_roots {
        for folder in ["claude-code-sessions", "local-agent-mode-sessions"] {
            for path in walk_files(&root.join(folder), WALK_DEPTH, is_jsonl) {
                collect_jsonl(
                    &path,
                    Source::Desktop,
                    &mut seen_files,
                    &mut seen_turns,
                    &mut hits,
                    &mut desktop_sessions,
                    &mut cli_sessions,
                );
            }
        }
    }

    let (account_email, five_hour_percent, seven_day_percent, five_hour_reset, seven_day_reset) =
        read_plan_snapshot(settings_files);
    let models = models_from_hits(&hits);
    let days = days_from_hits(&hits);
    let event_total = hits.len() as u64;
    let mut events = events_from_hits(&hits);
    let event_truncated = events.len() < hits.len();
    events.truncate(EVENT_LIMIT);

    let request_total = hits.len() as u64;
    let input_tokens = hits.iter().map(|hit| hit.input).sum();
    let output_tokens = hits.iter().map(|hit| hit.output).sum();
    let cache_write_tokens = hits.iter().map(|hit| hit.cache_write).sum();
    let cache_read_tokens = hits.iter().map(|hit| hit.cache_read).sum();
    let desktop_tokens = hits
        .iter()
        .filter(|hit| hit.source == Source::Desktop)
        .map(TokenHit::tokens)
        .sum();
    let cli_tokens = hits
        .iter()
        .filter(|hit| hit.source == Source::Cli)
        .map(TokenHit::tokens)
        .sum();
    let available = request_total > 0
        || five_hour_percent.is_some()
        || seven_day_percent.is_some()
        || !desktop_ids.is_empty();

    ClaudeUsageOverview {
        available,
        account_email,
        five_hour_percent,
        seven_day_percent,
        five_hour_reset,
        seven_day_reset,
        request_total,
        token_total: input_tokens + output_tokens + cache_write_tokens + cache_read_tokens,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        desktop_sessions: desktop_sessions.len() as u64,
        cli_sessions: cli_sessions.len() as u64,
        desktop_tokens,
        cli_tokens,
        models,
        days,
        events,
        event_total,
        event_truncated,
        notes: usage_notes(available, request_total > 0, !desktop_ids.is_empty()),
        refreshed_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

fn usage_notes(available: bool, has_hits: bool, has_desktop_index: bool) -> Vec<String> {
    let mut notes = vec![
        "中转台账（NPCR / CC / UC）只统计经过它的请求. 官方 Claude 桌面客户端默认走 Anthropic, 那些台账看不到桌面用量.".to_string(),
        "这里只读官方客户端写下的本机记录, 不估算 Token, 也不把会话正文送出 Rust.".to_string(),
    ];
    if !available {
        notes.push(
            "未找到 Claude 本机用量. 先在官方桌面 Code 标签或 Claude Code 里产生会话, 或查看 Claude 设置里的官方用量页.".to_string(),
        );
    } else if !has_hits {
        notes.push(
            "找到了桌面会话索引或额度快照, 但 JSONL 里还没有 usage 字段. Chat / Cowork 常常不落本地 Token.".to_string(),
        );
    } else if has_desktop_index {
        notes.push(
            "桌面 Code 会话通过 claude-code-sessions 的 cliSessionId 对到 ~/.claude/projects JSONL.".to_string(),
        );
    }
    notes.push(
        "Chat / Cowork 若没落本地 usage, 这里没有条数. 官方 JSONL 的 output_tokens 有时是流式占位.".to_string(),
    );
    notes
}

fn collect_jsonl(
    path: &Path,
    source: Source,
    seen_files: &mut HashSet<PathBuf>,
    seen_turns: &mut HashSet<String>,
    hits: &mut Vec<TokenHit>,
    desktop_sessions: &mut HashSet<String>,
    cli_sessions: &mut HashSet<String>,
) {
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen_files.insert(key) {
        return;
    }
    let session_id = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let before = hits.len();
    parse_jsonl(path, source, seen_turns, hits);
    if hits.len() > before && !session_id.is_empty() {
        match source {
            Source::Desktop => {
                desktop_sessions.insert(session_id);
            }
            Source::Cli => {
                cli_sessions.insert(session_id);
            }
        }
    }
}

fn parse_jsonl(path: &Path, source: Source, seen_turns: &mut HashSet<String>, hits: &mut Vec<TokenHit>) {
    let Ok(file) = File::open(path) else {
        return;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(hit) = hit_from_value(&value, source, seen_turns) {
            hits.push(hit);
        }
    }
}

fn hit_from_value(value: &Value, source: Source, seen_turns: &mut HashSet<String>) -> Option<TokenHit> {
    if value.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let usage = value
        .pointer("/message/usage")
        .or_else(|| value.get("usage"))
        .filter(|value| value.is_object())?;
    let input = value_u64(usage.get("input_tokens"));
    let output = value_u64(usage.get("output_tokens"));
    let cache_write = cache_write_tokens(usage);
    let cache_read = value_u64(usage.get("cache_read_input_tokens"));
    if input + output + cache_write + cache_read == 0 {
        return None;
    }
    let model = value
        .pointer("/message/model")
        .or_else(|| value.get("model"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if model == "<synthetic>" {
        return None;
    }
    let message_id = value.pointer("/message/id").and_then(Value::as_str).unwrap_or("");
    let request_id = value
        .get("requestId")
        .or_else(|| value.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let dedup_key = if !message_id.is_empty() {
        format!("m:{message_id}")
    } else if !request_id.is_empty() {
        format!("r:{request_id}")
    } else {
        String::new()
    };
    if !dedup_key.is_empty() && !seen_turns.insert(dedup_key) {
        return None;
    }
    Some(TokenHit {
        timestamp_ms: timestamp_ms(value),
        model: model.to_string(),
        source,
        input,
        output,
        cache_write,
        cache_read,
    })
}

fn cache_write_tokens(usage: &Value) -> u64 {
    let flat = value_u64(usage.get("cache_creation_input_tokens"));
    if flat > 0 {
        return flat;
    }
    usage
        .get("cache_creation")
        .map(|value| {
            value_u64(value.get("ephemeral_5m_input_tokens"))
                + value_u64(value.get("ephemeral_1h_input_tokens"))
        })
        .unwrap_or(0)
}

fn timestamp_ms(value: &Value) -> u64 {
    if let Some(text) = value.get("timestamp").and_then(Value::as_str) {
        if let Some(millis) = parse_iso_millis(text) {
            return millis;
        }
    }
    value
        .get("timestamp")
        .and_then(Value::as_u64)
        .map(|value| {
            if value < 1_000_000_000_000 {
                value * 1000
            } else {
                value
            }
        })
        .unwrap_or(0)
}

fn session_source(path: &Path, desktop_ids: &HashSet<String>) -> Source {
    path.file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|id| desktop_ids.contains(id))
        .then_some(Source::Desktop)
        .unwrap_or(Source::Cli)
}

fn index_desktop_sessions(desktop_roots: &[PathBuf]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for root in desktop_roots {
        for folder in ["claude-code-sessions", "local-agent-mode-sessions"] {
            for path in walk_files(&root.join(folder), WALK_DEPTH, is_local_session_index) {
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(id) = value.get("cliSessionId").and_then(Value::as_str) {
                    if !id.is_empty() {
                        ids.insert(id.to_string());
                    }
                }
            }
        }
    }
    ids
}

fn read_plan_snapshot(
    settings_files: &[PathBuf],
) -> (
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<String>,
    Option<String>,
) {
    for path in settings_files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let email = value
            .pointer("/oauthAccount/emailAddress")
            .or_else(|| value.pointer("/oauthAccount/email"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|email| !email.is_empty());
        let cached = value.get("cachedUsageUtilization");
        let utilization = cached
            .and_then(|value| value.get("utilization"))
            .or(cached);
        let five = utilization.and_then(|value| value.get("five_hour"));
        let seven = utilization.and_then(|value| value.get("seven_day"));
        let five_hour_percent = utilization_percent(five);
        let seven_day_percent = utilization_percent(seven);
        if email.is_some() || five_hour_percent.is_some() || seven_day_percent.is_some() {
            return (
                email,
                five_hour_percent,
                seven_day_percent,
                reset_at(five),
                reset_at(seven),
            );
        }
    }
    (None, None, None, None, None)
}

fn utilization_percent(node: Option<&Value>) -> Option<f64> {
    let value = node?;
    let percent = value_f64(value.get("utilization").or_else(|| value.get("percent")));
    if value.get("utilization").is_some() || value.get("percent").is_some() {
        Some(percent.clamp(0.0, 100.0))
    } else {
        None
    }
}

fn reset_at(node: Option<&Value>) -> Option<String> {
    node.and_then(|value| value.get("resets_at"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn models_from_hits(hits: &[TokenHit]) -> Vec<ClaudeModelUsage> {
    let mut models = BTreeMap::<String, ClaudeModelUsage>::new();
    for hit in hits {
        let entry = models
            .entry(hit.model.clone())
            .or_insert_with(|| ClaudeModelUsage {
                name: hit.model.clone(),
                requests: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                tokens: 0,
                desktop_tokens: 0,
                cli_tokens: 0,
            });
        entry.requests += 1;
        entry.input_tokens += hit.input;
        entry.output_tokens += hit.output;
        entry.cache_write_tokens += hit.cache_write;
        entry.cache_read_tokens += hit.cache_read;
        entry.tokens += hit.tokens();
        match hit.source {
            Source::Desktop => entry.desktop_tokens += hit.tokens(),
            Source::Cli => entry.cli_tokens += hit.tokens(),
        }
    }
    let mut models = models.into_values().collect::<Vec<_>>();
    models.sort_by(|left, right| {
        right
            .tokens
            .cmp(&left.tokens)
            .then_with(|| right.requests.cmp(&left.requests))
            .then_with(|| left.name.cmp(&right.name))
    });
    models
}

fn days_from_hits(hits: &[TokenHit]) -> Vec<ClaudeDailyUsage> {
    let mut days = BTreeMap::<String, ClaudeDailyUsage>::new();
    for hit in hits {
        let date = utc_date(hit.timestamp_ms);
        let entry = days.entry(date.clone()).or_insert_with(|| ClaudeDailyUsage {
            date,
            desktop_requests: 0,
            cli_requests: 0,
            input_tokens: 0,
            output_tokens: 0,
            tokens: 0,
        });
        match hit.source {
            Source::Desktop => entry.desktop_requests += 1,
            Source::Cli => entry.cli_requests += 1,
        }
        entry.input_tokens += hit.input;
        entry.output_tokens += hit.output;
        entry.tokens += hit.tokens();
    }
    days.into_values().rev().collect()
}

fn events_from_hits(hits: &[TokenHit]) -> Vec<ClaudeUsageEvent> {
    let mut events = hits
        .iter()
        .map(|hit| ClaudeUsageEvent {
            timestamp_ms: hit.timestamp_ms,
            model: hit.model.clone(),
            source: hit.source.as_str().to_string(),
            input_tokens: hit.input,
            output_tokens: hit.output,
            tokens: hit.tokens(),
        })
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| left.model.cmp(&right.model))
    });
    events
}

fn is_jsonl(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
}

fn is_local_session_index(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("local_") && name.ends_with(".json") && !name.ends_with(".tmp"))
}

fn walk_files(root: &Path, max_depth: usize, pred: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push((path, depth + 1));
            } else if meta.is_file() && pred(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn user_home() -> PathBuf {
    std::env::var_os("I18N_WORKBENCH_USER_HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn claude_config_roots() -> Vec<PathBuf> {
    let home = user_home();
    let mut roots = vec![home.join(".claude"), home.join(".config").join("claude")];
    if let Ok(value) = std::env::var("CLAUDE_CONFIG_DIR") {
        for part in value.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                roots.push(PathBuf::from(part));
            }
        }
    }
    unique_paths(roots)
}

fn claude_settings_files() -> Vec<PathBuf> {
    let home = user_home();
    let mut files = vec![home.join(".claude.json")];
    for root in claude_config_roots() {
        files.push(root.join(".claude.json"));
        if let Some(parent) = root.parent() {
            files.push(parent.join(".claude.json"));
        }
    }
    unique_paths(files)
}

#[cfg(target_os = "windows")]
fn desktop_data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        roots.push(appdata.join("Claude"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        roots.push(local.join("Claude"));
        roots.push(local.join("AnthropicClaude"));
        if let Ok(entries) = fs::read_dir(local.join("Packages")) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("Claude_") {
                    roots.push(entry.path().join("LocalCache").join("Roaming").join("Claude"));
                }
            }
        }
    }
    unique_paths(roots)
}

#[cfg(target_os = "macos")]
fn desktop_data_roots() -> Vec<PathBuf> {
    unique_paths(vec![user_home().join("Library/Application Support/Claude")])
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn desktop_data_roots() -> Vec<PathBuf> {
    let home = user_home();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    unique_paths(vec![
        config.join("Claude"),
        config.join("claude-desktop"),
        home.join(".config").join("Claude"),
    ])
}

fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("i18n-claude-usage-{name}-{nanos}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn aggregates_desktop_and_cli_jsonl_without_leaking_transcript() {
        let root = temp_root("mix");
        let projects = root.join("claude/projects/demo");
        write(
            &projects.join("cli-session.jsonl"),
            r#"{"type":"assistant","timestamp":"2026-08-20T10:00:00Z","message":{"id":"msg_cli","model":"claude-opus-4","content":[{"type":"text","text":"secret-cli"}],"usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":4,"cache_read_input_tokens":6}}}
{"type":"user","message":{"content":[{"type":"text","text":"do-not-export"}]}}
"#,
        );
        write(
            &projects.join("desk-session.jsonl"),
            r#"{"type":"assistant","timestamp":"2026-08-21T10:00:00Z","message":{"id":"msg_desk","model":"claude-sonnet-4","content":[{"type":"text","text":"secret-desk"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":8}}}
{"type":"assistant","timestamp":"2026-08-21T10:01:00Z","requestId":"req_dup","message":{"id":"msg_desk","model":"claude-sonnet-4","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":8}}}
"#,
        );
        write(
            &root.join("desktop/claude-code-sessions/acc/org/local_1.json"),
            r#"{"sessionId":"local_1","cliSessionId":"desk-session","title":"private-title","cwd":"C:\\secret\\repo"}"#,
        );
        write(
            &root.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"li@example.com"},"cachedUsageUtilization":{"utilization":{"five_hour":{"utilization":23,"resets_at":"2026-08-22T18:00:00Z"},"seven_day":{"utilization":41,"resets_at":"2026-08-25T09:00:00Z"}}},"projects":{"/tmp/x":{"lastModelUsage":{"claude-opus-4":{"costUSD":9.9}}}}}"#,
        );

        let usage = collect_from_roots(
            &[root.join("claude")],
            &[root.join("desktop")],
            &[root.join(".claude.json")],
        );
        assert!(usage.available);
        assert_eq!(usage.account_email.as_deref(), Some("li@example.com"));
        assert_eq!(usage.five_hour_percent, Some(23.0));
        assert_eq!(usage.seven_day_percent, Some(41.0));
        assert_eq!(usage.request_total, 2);
        assert_eq!(usage.input_tokens, 110);
        assert_eq!(usage.output_tokens, 70);
        assert_eq!(usage.cache_write_tokens, 4);
        assert_eq!(usage.cache_read_tokens, 14);
        assert_eq!(usage.desktop_sessions, 1);
        assert_eq!(usage.cli_sessions, 1);
        assert_eq!(usage.desktop_tokens, 158);
        assert_eq!(usage.cli_tokens, 40);
        assert_eq!(usage.models.len(), 2);
        assert_eq!(usage.days.len(), 2);
        assert_eq!(usage.days[0].date, "2026-08-21");
        assert_eq!(usage.days[0].desktop_requests, 1);
        assert_eq!(usage.days[1].cli_requests, 1);
        assert_eq!(usage.events[0].source, "desktop");
        assert_eq!(usage.events[1].source, "cli");

        let payload = serde_json::to_string(&usage).unwrap();
        assert!(!payload.contains("secret-cli"));
        assert!(!payload.contains("secret-desk"));
        assert!(!payload.contains("do-not-export"));
        assert!(!payload.contains("private-title"));
        assert!(!payload.contains("costUSD"));
        assert!(payload.contains("中转台账"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_error_and_synthetic_rows() {
        let mut seen = HashSet::new();
        assert!(hit_from_value(
            &json!({"isApiErrorMessage":true,"message":{"usage":{"input_tokens":9,"output_tokens":1}}}),
            Source::Cli,
            &mut seen,
        )
        .is_none());
        assert!(hit_from_value(
            &json!({"message":{"model":"<synthetic>","usage":{"input_tokens":9,"output_tokens":1}}}),
            Source::Cli,
            &mut seen,
        )
        .is_none());
        let hit = hit_from_value(
            &json!({
                "timestamp": 1_782_864_000_u64,
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 2,
                    "cache_creation": {
                        "ephemeral_5m_input_tokens": 3,
                        "ephemeral_1h_input_tokens": 4
                    }
                }
            }),
            Source::Desktop,
            &mut seen,
        )
        .unwrap();
        assert_eq!(hit.cache_write, 7);
        assert_eq!(hit.tokens(), 10);
        assert_eq!(hit.timestamp_ms, 1_782_864_000_000);
    }

    #[test]
    fn empty_roots_return_honest_empty_snapshot() {
        let root = temp_root("empty");
        let usage = collect_from_roots(
            &[root.join("missing-config")],
            &[root.join("missing-desktop")],
            &[root.join("missing.json")],
        );
        assert!(!usage.available);
        assert_eq!(usage.request_total, 0);
        assert!(usage.notes.iter().any(|note| note.contains("未找到")));
        let _ = fs::remove_dir_all(&root);
    }
}
