use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters;
use crate::usage;

const CLOUD_FIELDS: [&str; 12] = [
    "createdFromBackgroundAgent",
    "backgroundComposerId",
    "bcId",
    "bcID",
    "cloudAgentId",
    "cloudAgentBcId",
    "cloudAgentStatus",
    "isBackgroundAgent",
    "createdFromCloudAgent",
    "runningInCloud",
    "isCloudAgent",
    "backgroundAgentId",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StuckChat {
    pub composer_id: String,
    pub name: String,
    pub workspace: String,
    pub status: String,
    pub kind: String,
    pub reason: String,
    pub archived: bool,
    pub cloud_bound: bool,
    pub can_detach: bool,
}

#[derive(Default)]
pub struct ChatInspection {
    pub chats: Vec<StuckChat>,
    pub error: Option<String>,
    pub backup_path: Option<String>,
}

pub fn inspect_stuck_chats() -> ChatInspection {
    match inspect_stuck_chats_at(&usage::cursor_state_db_path()) {
        Ok(chats) => ChatInspection {
            chats,
            error: None,
            backup_path: None,
        },
        Err(error) => ChatInspection {
            chats: Vec::new(),
            error: Some(error),
            backup_path: None,
        },
    }
}

pub fn detach_stuck_chats() -> Result<ChatInspection, String> {
    let path = usage::cursor_state_db_path();
    if !path.is_file() {
        return Err(format!("未找到 Cursor 状态库: {}", path.display()));
    }
    let backup = backup_state_db(&path)?;
    let chats = detach_stuck_chats_at(&path)?;
    adapters::restore_user_ownership(backup.parent().unwrap_or(&backup));
    Ok(ChatInspection {
        chats,
        error: None,
        backup_path: Some(backup.to_string_lossy().into_owned()),
    })
}

fn inspect_stuck_chats_at(path: &Path) -> Result<Vec<StuckChat>, String> {
    let connection = open_state_db(path, true)?;
    Ok(collect_stuck_chats(&connection)?)
}

fn detach_stuck_chats_at(path: &Path) -> Result<Vec<StuckChat>, String> {
    let connection = open_state_db(path, false)?;
    let stuck = collect_stuck_chats(&connection)?
        .into_iter()
        .filter(|chat| chat.can_detach)
        .collect::<Vec<_>>();
    if stuck.is_empty() {
        return Err("没有发现需要改回本地的对话. 还在跑的云端任务不会改; 若只是标错, 等云端结束后再解除".to_string());
    }
    let ids = stuck
        .iter()
        .map(|chat| chat.composer_id.clone())
        .collect::<HashSet<_>>();
    detach_headers(&connection, "composer.composerHeaders", &ids)?;
    detach_headers(&connection, "composer.composerData", &ids)?;
    if table_exists(&connection, "cursorDiskKV")? {
        for id in &ids {
            detach_composer_data(&connection, id)?;
        }
    }
    collect_stuck_chats(&connection)
}

fn open_state_db(path: &Path, read_only: bool) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("未找到 Cursor 状态库: {}", path.display()));
    }
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
    };
    Connection::open_with_flags(path, flags)
        .map_err(|error| format!("无法打开 Cursor 状态库: {error}"))
}

fn collect_stuck_chats(connection: &Connection) -> Result<Vec<StuckChat>, String> {
    let mut headers = composers_from_item(connection, "composer.composerHeaders")?;
    if headers.is_empty() {
        headers = composers_from_item(connection, "composer.composerData")?;
    }
    let cloud = cloud_repository_index(connection)?;
    let mut by_id = HashMap::<String, (Value, Option<StuckChat>)>::new();
    for header in headers {
        let Some(id) = header.get("composerId").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        let classified = classify_chat(&header, None, &cloud);
        by_id.insert(id, (header, classified));
    }
    if table_exists(connection, "cursorDiskKV")? {
        for (id, (header, classified)) in &mut by_id {
            if classified.is_none() {
                continue;
            }
            if let Some(data) = read_composer_data(connection, id)? {
                let meta = composer_index_fields(&data);
                *classified = classify_chat(header, Some(&meta), &cloud);
            }
        }
    }
    let mut chats = by_id
        .into_values()
        .filter_map(|(_, chat)| chat)
        .collect::<Vec<_>>();
    chats.sort_by(|left, right| {
        right
            .can_detach
            .cmp(&left.can_detach)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.composer_id.cmp(&right.composer_id))
    });
    Ok(chats)
}

fn classify_chat(header: &Value, data: Option<&Value>, cloud: &CloudIndex) -> Option<StuckChat> {
    let composer_id = header
        .get("composerId")
        .and_then(Value::as_str)
        .or_else(|| data.and_then(|value| value.get("composerId")).and_then(Value::as_str))?
        .to_string();
    let archived = truthy(header.get("isArchived")).or_else(|| data.and_then(|value| truthy(value.get("isArchived")))).unwrap_or(false);
    let created_from_background = truthy(header.get("createdFromBackgroundAgent"))
        .or_else(|| data.and_then(|value| truthy(value.get("createdFromBackgroundAgent"))))
        .unwrap_or(false);
    let bc_id = first_text(header, data, &["bcId", "bcID", "backgroundComposerId", "cloudAgentId", "cloudAgentBcId"]);
    let status = first_text(header, data, &["status"]).unwrap_or_else(|| "unknown".to_string());
    let cloud_status = bc_id
        .as_deref()
        .and_then(|id| cloud.status_of(id))
        .or_else(|| cloud.status_of(&composer_id))
        .or_else(|| cloud.status_of(&format!("bc-{composer_id}")));
    let flagged = created_from_background || bc_id.is_some();
    if !flagged {
        return None;
    }
    let stuck_status = is_stuck_status(&status);
    let cloud_stuck = cloud_status.is_some_and(is_stuck_status);
    let finished = is_finished_status(&status);
    let live = is_live_status(&status) || cloud_status.is_some_and(is_live_status);
    let can_detach = !live && (archived || stuck_status || cloud_stuck || finished);
    let cloud_bound = true;
    let (kind, reason) = if !can_detach {
        (
            "cloud-bound",
            "索引仍标成 Cloud Agent / 远程控制会话, 且云端任务还在跑或可继续跟进. 杀进程修不好这条标记; 还在跑时不要改.",
        )
    } else if archived {
        (
            "stuck-archived",
            "远程控制或 Cloud Agent 交接后被归档, 本机仍把它当成云端会话, 继续对话会报 Background composer is archived.",
        )
    } else if stuck_status || cloud_stuck {
        (
            "stuck-status",
            "对话仍绑着云端 Agent, 状态已中断或失败, 本机侧栏会显示 Running in cloud. 杀进程清不掉这个标记.",
        )
    } else {
        (
            "misclassified",
            "本机把这条对话标成 Cloud Agent, 但云端任务已不在跑. 这是本地会话状态, 杀进程修不好, 清掉标记后可当普通对话继续.",
        )
    };
    Some(StuckChat {
        name: first_text(header, data, &["name"]).unwrap_or_else(|| "未命名对话".to_string()),
        workspace: workspace_path(header),
        status,
        kind: kind.to_string(),
        reason: reason.to_string(),
        archived,
        cloud_bound,
        can_detach,
        composer_id,
    })
}

fn composers_from_item(connection: &Connection, key: &str) -> Result<Vec<Value>, String> {
    let Some(raw) = read_item(connection, key)? else {
        return Ok(Vec::new());
    };
    Ok(composers_from_value(&parse_json_value(&raw)))
}

fn composers_from_value(value: &Value) -> Vec<Value> {
    value
        .get("allComposers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("composerId").and_then(Value::as_str).is_some())
        .cloned()
        .collect()
}

#[derive(Default)]
struct CloudIndex {
    status_by_id: HashMap<String, String>,
}

impl CloudIndex {
    fn status_of(&self, id: &str) -> Option<&str> {
        self.status_by_id.get(id).map(String::as_str)
    }

    fn record(&mut self, id: &str, status: Option<&str>) {
        let Some(status) = status.filter(|value| !value.is_empty()) else {
            return;
        };
        self.status_by_id.insert(id.to_string(), status.to_string());
        if let Some(stripped) = id.strip_prefix("bc-") {
            self.status_by_id.insert(stripped.to_string(), status.to_string());
        }
    }
}

fn cloud_repository_index(connection: &Connection) -> Result<CloudIndex, String> {
    let mut statement = connection
        .prepare("SELECT key, value FROM ItemTable WHERE key LIKE 'cloudAgentRepository%'")
        .map_err(|error| format!("读取云端 Agent 仓库失败: {error}"))?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|error| format!("读取云端 Agent 仓库失败: {error}"))?;
    let mut index = CloudIndex::default();
    for row in rows {
        let (_, value) = row.map_err(|error| format!("读取云端 Agent 仓库失败: {error}"))?;
        harvest_cloud_ids(&parse_json_value(&value), &mut index);
    }
    Ok(index)
}

fn harvest_cloud_ids(value: &Value, index: &mut CloudIndex) {
    match value {
        Value::Object(map) => {
            let status = map.get("status").and_then(Value::as_str);
            for key in ["composerId", "bcId", "bcID", "id", "backgroundComposerId"] {
                if let Some(id) = map.get(key).and_then(Value::as_str) {
                    index.record(id, status);
                }
            }
            for nested in map.values() {
                harvest_cloud_ids(nested, index);
            }
        }
        Value::Array(items) => {
            for nested in items {
                harvest_cloud_ids(nested, index);
            }
        }
        _ => {}
    }
}

fn read_item(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut statement = connection
        .prepare("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1")
        .map_err(|error| format!("读取 Cursor 对话索引失败: {error}"))?;
    match statement.query_row([key], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(format!("读取 Cursor 对话索引失败: {error}")),
    }
}

fn composer_index_fields(value: &Value) -> Value {
    let mut fields = serde_json::Map::new();
    for key in [
        "composerId",
        "status",
        "name",
        "isArchived",
        "createdFromBackgroundAgent",
        "backgroundComposerId",
        "bcId",
        "bcID",
        "cloudAgentId",
        "cloudAgentBcId",
        "cloudAgentStatus",
        "isBackgroundAgent",
        "createdFromCloudAgent",
        "runningInCloud",
        "isCloudAgent",
        "backgroundAgentId",
    ] {
        if let Some(field) = value.get(key).cloned() {
            fields.insert(key.to_string(), field);
        }
    }
    Value::Object(fields)
}

fn read_composer_data(connection: &Connection, composer_id: &str) -> Result<Option<Value>, String> {
    let mut statement = connection
        .prepare("SELECT value FROM cursorDiskKV WHERE key = ?1 LIMIT 1")
        .map_err(|error| format!("读取对话状态失败: {error}"))?;
    match statement.query_row([format!("composerData:{composer_id}")], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(Some(parse_json_value(&value))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(format!("读取对话状态失败: {error}")),
    }
}

fn detach_headers(connection: &Connection, key: &str, ids: &HashSet<String>) -> Result<(), String> {
    let Some(raw) = read_item(connection, key)? else {
        return Ok(());
    };
    let mut value = parse_json_value(&raw);
    let Some(composers) = value.get_mut("allComposers").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let mut changed = false;
    for composer in composers {
        let Some(id) = composer.get("composerId").and_then(Value::as_str) else {
            continue;
        };
        if !ids.contains(id) {
            continue;
        }
        if detach_value(composer) {
            changed = true;
        }
    }
    if changed {
        write_item(connection, key, &value)?;
    }
    Ok(())
}

fn detach_composer_data(connection: &Connection, composer_id: &str) -> Result<(), String> {
    let Some(mut value) = read_composer_data(connection, composer_id)? else {
        return Ok(());
    };
    if detach_value(&mut value) {
        write_disk_kv(connection, &format!("composerData:{composer_id}"), &value)?;
    }
    Ok(())
}

fn detach_value(value: &mut Value) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    if object.get("isArchived") != Some(&Value::Bool(false)) && object.contains_key("isArchived") {
        object.insert("isArchived".to_string(), json!(false));
        changed = true;
    }
    if object
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(is_stuck_status)
    {
        object.insert("status".to_string(), json!("completed"));
        changed = true;
    }
    for field in CLOUD_FIELDS {
        if object.remove(field).is_some() {
            changed = true;
        }
    }
    changed
}

fn write_item(connection: &Connection, key: &str, value: &Value) -> Result<(), String> {
    let encoded = serde_json::to_string(value).map_err(|error| format!("无法序列化对话索引: {error}"))?;
    connection
        .execute("UPDATE ItemTable SET value = ?1 WHERE key = ?2", [&encoded, key])
        .map_err(|error| format!("写入对话索引失败: {error}"))?;
    Ok(())
}

fn write_disk_kv(connection: &Connection, key: &str, value: &Value) -> Result<(), String> {
    let encoded = serde_json::to_string(value).map_err(|error| format!("无法序列化对话状态: {error}"))?;
    connection
        .execute("UPDATE cursorDiskKV SET value = ?1 WHERE key = ?2", [&encoded, key])
        .map_err(|error| format!("写入对话状态失败: {error}"))?;
    Ok(())
}

fn backup_state_db(path: &Path) -> Result<PathBuf, String> {
    let root = adapters::local_app_data().join("session-repairs");
    fs::create_dir_all(&root).map_err(|error| format!("无法创建会话修复备份目录: {error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let dest = root.join(format!("state-{stamp}.vscdb"));
    fs::copy(path, &dest).map_err(|error| format!("无法备份 Cursor 状态库: {error}"))?;
    for suffix in ["-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{suffix}", path.display()));
        if source.is_file() {
            let side = root.join(format!("state-{stamp}.vscdb{suffix}"));
            fs::copy(&source, &side).map_err(|error| format!("无法备份 Cursor 状态库附属文件: {error}"))?;
        }
    }
    Ok(dest)
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, String> {
    let mut statement = connection
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1")
        .map_err(|error| format!("检查数据表失败: {error}"))?;
    match statement.query_row([name], |_| Ok(())) {
        Ok(()) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(error) => Err(format!("检查数据表失败: {error}")),
    }
}

fn parse_json_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    serde_json::from_str::<Value>(trimmed)
        .or_else(|_| serde_json::from_str::<String>(trimmed).and_then(|inner| serde_json::from_str(&inner)))
        .unwrap_or(Value::Null)
}

fn first_text(header: &Value, data: Option<&Value>, keys: &[&str]) -> Option<String> {
    for source in [Some(header), data].into_iter().flatten() {
        for key in keys {
            if let Some(text) = source.get(*key).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn workspace_path(header: &Value) -> String {
    header
        .pointer("/workspaceIdentifier/uri/fsPath")
        .or_else(|| header.pointer("/workspaceIdentifier/uri/path"))
        .and_then(Value::as_str)
        .or_else(|| header.pointer("/workspaceIdentifier/id").and_then(Value::as_str))
        .unwrap_or("--")
        .to_string()
}

fn truthy(value: Option<&Value>) -> Option<bool> {
    value.and_then(|value| {
        value.as_bool().or_else(|| match value {
            Value::String(text) => match text.trim() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            },
            Value::Number(number) => number.as_i64().map(|value| value != 0),
            _ => None,
        })
    })
}

fn normalize_status(status: &str) -> String {
    status
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_")
}

fn is_stuck_status(status: &str) -> bool {
    matches!(
        normalize_status(status).as_str(),
        "aborted" | "error" | "failed" | "archived" | "expired"
    )
}

fn is_finished_status(status: &str) -> bool {
    matches!(
        normalize_status(status).as_str(),
        "completed" | "complete" | "done" | "finished" | "success" | "succeeded"
    )
}

fn is_live_status(status: &str) -> bool {
    matches!(
        normalize_status(status).as_str(),
        "running"
            | "idle"
            | "generating"
            | "in_progress"
            | "inprogress"
            | "not_yet_started"
            | "notyetstarted"
            | "waiting_for_background_work"
            | "waiting"
            | "active"
            | "queued"
            | "pending"
            | "starting"
            | "provisioning"
            | "installing"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed(connection: &Connection, headers: Value, data: Value) {
        connection
            .execute_batch(
                "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT);
                 CREATE TABLE cursorDiskKV (key TEXT PRIMARY KEY, value TEXT);",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ItemTable(key, value) VALUES ('composer.composerHeaders', ?1), ('cloudAgentRepository.agents.user', ?2)",
                [
                    headers.to_string(),
                    json!({"agents":[{"composerId":"aaaa-1","bcId":"bc-aaaa-1","status":"ARCHIVED"}]}).to_string(),
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cursorDiskKV(key, value) VALUES (?1, ?2)",
                [
                    "composerData:aaaa-1".to_string(),
                    data.to_string(),
                ],
            )
            .unwrap();
    }

    #[test]
    fn classifies_archived_background_chat_as_stuck() {
        let header = json!({
            "composerId": "aaaa-1",
            "name": "卡住的远程控制",
            "isArchived": true,
            "createdFromBackgroundAgent": true,
            "workspaceIdentifier": { "uri": { "fsPath": "D:/work/demo" } }
        });
        let chat = classify_chat(&header, None, &CloudIndex::default()).unwrap();
        assert!(chat.can_detach);
        assert_eq!(chat.kind, "stuck-archived");
        assert_eq!(chat.workspace, "D:/work/demo");
    }

    #[test]
    fn keeps_live_cloud_chats_visible_but_not_detachable() {
        let header = json!({
            "composerId": "live-1",
            "name": "还在跑的云端任务",
            "createdFromBackgroundAgent": true,
            "isArchived": false,
            "bcId": "bc-live-1"
        });
        let mut cloud = CloudIndex::default();
        cloud.record("bc-live-1", Some("RUNNING"));
        let chat = classify_chat(&header, Some(&json!({"status":"completed"})), &cloud).unwrap();
        assert!(!chat.can_detach);
        assert_eq!(chat.kind, "cloud-bound");
    }

    #[test]
    fn wrongly_marked_finished_chat_is_detachable() {
        let header = json!({
            "composerId": "wrong-1",
            "name": "被标错的对话",
            "createdFromBackgroundAgent": true,
            "isArchived": false,
            "bcId": "bc-wrong-1"
        });
        let chat = classify_chat(&header, Some(&json!({"status":"completed"})), &CloudIndex::default()).unwrap();
        assert!(chat.can_detach);
        assert_eq!(chat.kind, "misclassified");
    }

    #[test]
    fn unknown_status_stays_bound_until_finished() {
        let header = json!({
            "composerId": "maybe-1",
            "name": "状态不明",
            "createdFromBackgroundAgent": true,
            "isArchived": false,
            "bcId": "bc-maybe-1"
        });
        let chat = classify_chat(&header, None, &CloudIndex::default()).unwrap();
        assert!(!chat.can_detach);
        assert_eq!(chat.kind, "cloud-bound");
    }

    #[test]
    fn does_not_scan_unflagged_composer_documents() {
        let connection = Connection::open_in_memory().unwrap();
        seed(
            &connection,
            json!({"allComposers":[{"composerId":"data-1","name":"普通对话"}]}),
            json!({"composerId":"data-1","status":"completed","createdFromBackgroundAgent":true,"conversation":[{"text":"secret"}]}),
        );
        assert!(collect_stuck_chats(&connection).unwrap().is_empty());
    }

    #[test]
    fn ignores_plain_local_chats() {
        let header = json!({
            "composerId": "local-1",
            "name": "普通对话",
            "isArchived": false
        });
        assert!(classify_chat(&header, None, &CloudIndex::default()).is_none());
    }

    #[test]
    fn detaches_archived_cloud_flags_in_temp_db() {
        let connection = Connection::open_in_memory().unwrap();
        seed(
            &connection,
            json!({"allComposers":[{
                "composerId":"aaaa-1",
                "name":"卡住的远程控制",
                "isArchived":true,
                "createdFromBackgroundAgent":true,
                "bcId":"bc-aaaa-1"
            }]}),
            json!({"composerId":"aaaa-1","status":"aborted","createdFromBackgroundAgent":true}),
        );
        let before = collect_stuck_chats(&connection).unwrap();
        assert_eq!(before.len(), 1);
        assert!(before[0].can_detach);

        let ids = HashSet::from(["aaaa-1".to_string()]);
        detach_headers(&connection, "composer.composerHeaders", &ids).unwrap();
        detach_composer_data(&connection, "aaaa-1").unwrap();
        let after = collect_stuck_chats(&connection).unwrap();
        assert!(after.is_empty());

        let headers = read_item(&connection, "composer.composerHeaders").unwrap().unwrap();
        assert!(!headers.contains("createdFromBackgroundAgent"));
        assert!(headers.contains("\"isArchived\":false"));
        let data = read_composer_data(&connection, "aaaa-1").unwrap().unwrap();
        assert_eq!(data.get("status").and_then(Value::as_str), Some("completed"));
    }
}
