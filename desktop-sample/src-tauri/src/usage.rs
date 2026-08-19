use base64::{engine::general_purpose::URL_SAFE, engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ureq::Error;

use crate::network;

const USAGE_SUMMARY_URL: &str = "https://cursor.com/api/usage-summary";
const MODEL_USAGE_URL: &str = "https://api2.cursor.sh/auth/usage";
const FILTERED_EVENTS_URL: &str = "https://cursor.com/api/dashboard/get-filtered-usage-events";
const EVENT_PAGE_SIZE: u64 = 100;
const EVENT_MAX_PAGES: u64 = 8;

struct CursorCredentials {
    token: String,
    user_id: String,
    email: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub name: String,
    pub requests: u64,
    pub request_limit: u64,
    pub tokens: u64,
    pub plan_tokens: u64,
    pub api_tokens: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub plan_requests: u64,
    pub api_requests: u64,
    pub plan_tokens: u64,
    pub api_tokens: u64,
    pub charged_cents: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub timestamp_ms: u64,
    pub model: String,
    pub pool: String,
    pub kind: String,
    pub tokens: u64,
    pub charged_cents: f64,
    pub is_headless: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageOverview {
    pub account_email: Option<String>,
    pub membership_type: String,
    pub plan_used: f64,
    pub plan_limit: f64,
    pub plan_remaining: f64,
    pub total_percent_used: f64,
    pub api_percent_used: f64,
    pub on_demand_enabled: bool,
    pub on_demand_used: f64,
    pub billing_cycle_start: Option<String>,
    pub billing_cycle_end: Option<String>,
    pub request_total: u64,
    pub token_total: u64,
    pub plan_tokens: u64,
    pub api_tokens: u64,
    pub models: Vec<ModelUsage>,
    pub days: Vec<DailyUsage>,
    pub events: Vec<UsageEvent>,
    pub event_total: u64,
    pub event_truncated: bool,
    pub events_error: Option<String>,
    pub refreshed_at_unix: u64,
}

pub fn load_cursor_usage() -> Result<UsageOverview, String> {
    let credentials = read_cursor_credentials(&cursor_state_db_path())?;
    let agent = network::platform_agent(Duration::from_secs(20));

    let cookie = format!(
        "WorkosCursorSessionToken={}::{}",
        credentials.user_id, credentials.token
    );
    let summary = fetch_json(
        agent
            .get(USAGE_SUMMARY_URL)
            .header("Accept", "application/json")
            .header("Cookie", &cookie),
        "套餐用量",
    )?;
    let authorization = format!("Bearer {}", credentials.token);
    let model_usage = fetch_json(
        agent
            .get(MODEL_USAGE_URL)
            .header("Accept", "application/json")
            .header("Authorization", &authorization),
        "模型用量",
    )?;

    let mut overview = parse_usage(credentials.email, &summary, &model_usage)?;
    match load_usage_events(&agent, &cookie, &overview) {
        Ok((events, total)) => attach_events(&mut overview, events, total),
        Err(error) => overview.events_error = Some(error),
    }
    Ok(overview)
}

pub(crate) fn cursor_state_db_path() -> PathBuf {
    cursor_user_data_root()
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

#[cfg(target_os = "windows")]
fn cursor_user_data_root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(target_os = "macos")]
fn cursor_user_data_root() -> PathBuf {
    std::env::var_os("I18N_WORKBENCH_USER_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support"))
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn cursor_user_data_root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
        })
        .unwrap_or_else(std::env::temp_dir)
}

fn read_cursor_credentials(path: &Path) -> Result<CursorCredentials, String> {
    if !path.is_file() {
        return Err(format!(
            "未找到 Cursor 登录数据库: {}. 请先登录 Cursor",
            path.display()
        ));
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("无法只读打开 Cursor 登录数据库: {error}"))?;
    let token = read_state_value(&connection, "cursorAuth/accessToken")?
        .ok_or_else(|| "Cursor 尚未登录或登录令牌不存在".to_string())?;
    let token = normalize_state_value(&token);
    if token.is_empty() {
        return Err("Cursor 登录令牌为空, 请重新登录 Cursor".to_string());
    }
    let email = read_state_value(&connection, "cursorAuth/cachedEmail")?
        .map(|value| normalize_state_value(&value))
        .filter(|value| !value.is_empty());
    let subject = jwt_subject(&token)?;
    let user_id = subject
        .strip_prefix("auth0|")
        .unwrap_or(&subject)
        .to_string();
    if user_id.is_empty() {
        return Err("Cursor 登录令牌缺少用户标识, 请重新登录 Cursor".to_string());
    }
    Ok(CursorCredentials {
        token,
        user_id,
        email,
    })
}

fn read_state_value(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut statement = connection
        .prepare("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1")
        .map_err(|error| format!("读取 Cursor 登录状态失败: {error}"))?;
    match statement.query_row([key], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(format!("读取 Cursor 登录状态失败: {error}")),
    }
}

fn normalize_state_value(value: &str) -> String {
    serde_json::from_str::<String>(value)
        .unwrap_or_else(|_| value.to_string())
        .trim()
        .to_string()
}

fn jwt_subject(token: &str) -> Result<String, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "Cursor 登录令牌格式无效, 请重新登录 Cursor".to_string())?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .map_err(|_| "Cursor 登录令牌载荷无法解析, 请重新登录 Cursor".to_string())?;
    let value: Value = serde_json::from_slice(&decoded)
        .map_err(|_| "Cursor 登录令牌载荷格式无效, 请重新登录 Cursor".to_string())?;
    value
        .get("sub")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Cursor 登录令牌缺少用户标识, 请重新登录 Cursor".to_string())
}

fn fetch_json(
    request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
    label: &str,
) -> Result<Value, String> {
    let mut response = request
        .call()
        .map_err(|error| request_error(label, error))?;
    response
        .body_mut()
        .read_json::<Value>()
        .map_err(|error| format!("Cursor {label}响应格式错误: {error}"))
}

fn fetch_json_post(agent: &ureq::Agent, url: &str, cookie: &str, body: Value, label: &str) -> Result<Value, String> {
    let mut response = agent
        .post(url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Origin", "https://cursor.com")
        .header("Referer", "https://cursor.com/dashboard?tab=usage")
        .header("Cookie", cookie)
        .send_json(body)
        .map_err(|error| request_error(label, error))?;
    response
        .body_mut()
        .read_json::<Value>()
        .map_err(|error| format!("Cursor {label}响应格式错误: {error}"))
}

fn request_error(label: &str, error: Error) -> String {
    match error {
        Error::StatusCode(401 | 403) => {
            format!("Cursor 登录已过期, 无法读取{label}. 请重新登录 Cursor")
        }
        Error::StatusCode(code) => format!("Cursor {label}接口返回 HTTP {code}"),
        other => format!("连接 Cursor {label}接口失败: {other}"),
    }
}

fn parse_usage(
    account_email: Option<String>,
    summary: &Value,
    model_usage: &Value,
) -> Result<UsageOverview, String> {
    let plan = summary
        .pointer("/individualUsage/plan")
        .and_then(Value::as_object)
        .ok_or_else(|| "Cursor 套餐用量响应缺少 individualUsage.plan".to_string())?;
    let on_demand = summary.pointer("/individualUsage/onDemand");
    let mut models = model_usage
        .as_object()
        .into_iter()
        .flat_map(|object| object.iter())
        .filter_map(|(name, value)| {
            let details = value.as_object()?;
            let requests = value_u64(details.get("numRequests"));
            let request_limit = value_u64(details.get("maxRequestUsage"));
            let tokens = value_u64(details.get("numTokens"));
            ((requests + request_limit + tokens) > 0).then(|| ModelUsage {
                name: name.clone(),
                requests,
                request_limit,
                tokens,
                plan_tokens: 0,
                api_tokens: 0,
            })
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        right
            .requests
            .cmp(&left.requests)
            .then_with(|| right.tokens.cmp(&left.tokens))
            .then_with(|| left.name.cmp(&right.name))
    });
    let request_total = models.iter().map(|model| model.requests).sum();
    let token_total = models.iter().map(|model| model.tokens).sum();
    Ok(UsageOverview {
        account_email,
        membership_type: summary
            .get("membershipType")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        plan_used: value_f64(plan.get("used")),
        plan_limit: value_f64(plan.get("limit")),
        plan_remaining: value_f64(plan.get("remaining")),
        total_percent_used: value_f64(plan.get("totalPercentUsed")),
        api_percent_used: value_f64(plan.get("apiPercentUsed")),
        on_demand_enabled: on_demand
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        on_demand_used: value_f64(on_demand.and_then(|value| value.get("used"))),
        billing_cycle_start: summary
            .get("billingCycleStart")
            .and_then(Value::as_str)
            .map(str::to_string),
        billing_cycle_end: summary
            .get("billingCycleEnd")
            .and_then(Value::as_str)
            .map(str::to_string),
        request_total,
        token_total,
        plan_tokens: 0,
        api_tokens: 0,
        models,
        days: Vec::new(),
        events: Vec::new(),
        event_total: 0,
        event_truncated: false,
        events_error: None,
        refreshed_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn load_usage_events(
    agent: &ureq::Agent,
    cookie: &str,
    overview: &UsageOverview,
) -> Result<(Vec<UsageEvent>, u64), String> {
    let start_ms = iso_to_millis(overview.billing_cycle_start.as_deref()).unwrap_or(0);
    let end_ms = iso_to_millis(overview.billing_cycle_end.as_deref())
        .unwrap_or_else(now_millis)
        .max(now_millis());
    let mut events = Vec::new();
    let mut total = 0;
    for page in 1..=EVENT_MAX_PAGES {
        let payload = json!({
            "startDate": start_ms.to_string(),
            "endDate": end_ms.to_string(),
            "page": page,
            "pageSize": EVENT_PAGE_SIZE,
        });
        let response = fetch_json_post(agent, FILTERED_EVENTS_URL, cookie, payload, "请求记录")?;
        total = value_u64(
            response
                .get("totalUsageEventsCount")
                .or_else(|| response.get("totalUsageEvents")),
        );
        let page_events = parse_usage_events(&response);
        let received = page_events.len() as u64;
        events.extend(page_events);
        if received == 0 || events.len() as u64 >= total || received < EVENT_PAGE_SIZE {
            break;
        }
    }
    Ok((events, total))
}

fn attach_events(overview: &mut UsageOverview, mut events: Vec<UsageEvent>, total: u64) {
    events.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| left.model.cmp(&right.model))
    });
    overview.event_total = total.max(events.len() as u64);
    overview.event_truncated = (events.len() as u64) < overview.event_total;
    overview.plan_tokens = events
        .iter()
        .filter(|event| event.pool == "plan")
        .map(|event| event.tokens)
        .sum();
    overview.api_tokens = events
        .iter()
        .filter(|event| event.pool == "api")
        .map(|event| event.tokens)
        .sum();
    if !events.is_empty() {
        overview.request_total = events.len() as u64;
        overview.token_total = overview.plan_tokens + overview.api_tokens;
        overview.models = models_from_events(&events);
    }
    overview.days = days_from_events(&events);
    overview.events = events;
}

fn parse_usage_events(value: &Value) -> Vec<UsageEvent> {
    value
        .get("usageEventsDisplay")
        .or_else(|| value.get("usageEvents"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_usage_event)
        .collect()
}

fn parse_usage_event(value: &Value) -> Option<UsageEvent> {
    let timestamp_ms = value_u64(value.get("timestamp"));
    if timestamp_ms == 0 {
        return None;
    }
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let usage_based = value.get("usageBasedCosts").and_then(Value::as_str);
    let tokens = token_total(value.get("tokenUsage"));
    Some(UsageEvent {
        timestamp_ms,
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        pool: classify_pool(&kind, usage_based).to_string(),
        kind,
        tokens,
        charged_cents: value_f64(value.get("chargedCents")).max(token_cents(value.get("tokenUsage"))),
        is_headless: value
            .get("isHeadless")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn classify_pool(kind: &str, usage_based_costs: Option<&str>) -> &'static str {
    let kind = kind.to_ascii_uppercase();
    if kind.contains("USAGE_BASED") || kind.contains("ON_DEMAND") {
        return "api";
    }
    if kind.contains("INCLUDED") || kind.contains("PLAN") {
        return "plan";
    }
    if usage_based_costs.is_some_and(has_usage_based_charge) {
        return "api";
    }
    "plan"
}

fn has_usage_based_charge(value: &str) -> bool {
    let digits = value
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    digits.parse::<f64>().ok().is_some_and(|amount| amount > 0.0)
}

fn token_total(value: Option<&Value>) -> u64 {
    let Some(details) = value.and_then(Value::as_object) else {
        return 0;
    };
    ["inputTokens", "outputTokens", "cacheWriteTokens", "cacheReadTokens"]
        .into_iter()
        .map(|key| value_u64(details.get(key)))
        .sum()
}

fn token_cents(value: Option<&Value>) -> f64 {
    value_f64(value.and_then(|details| details.get("totalCents")))
}

fn models_from_events(events: &[UsageEvent]) -> Vec<ModelUsage> {
    let mut models = BTreeMap::<String, ModelUsage>::new();
    for event in events {
        let entry = models.entry(event.model.clone()).or_insert_with(|| ModelUsage {
            name: event.model.clone(),
            requests: 0,
            request_limit: 0,
            tokens: 0,
            plan_tokens: 0,
            api_tokens: 0,
        });
        entry.requests += 1;
        entry.tokens += event.tokens;
        if event.pool == "api" {
            entry.api_tokens += event.tokens;
        } else {
            entry.plan_tokens += event.tokens;
        }
    }
    let mut models = models.into_values().collect::<Vec<_>>();
    models.sort_by(|left, right| {
        right
            .requests
            .cmp(&left.requests)
            .then_with(|| right.tokens.cmp(&left.tokens))
            .then_with(|| left.name.cmp(&right.name))
    });
    models
}

fn days_from_events(events: &[UsageEvent]) -> Vec<DailyUsage> {
    let mut days = BTreeMap::<String, DailyUsage>::new();
    for event in events {
        let date = utc_date(event.timestamp_ms);
        let entry = days.entry(date.clone()).or_insert_with(|| DailyUsage {
            date,
            plan_requests: 0,
            api_requests: 0,
            plan_tokens: 0,
            api_tokens: 0,
            charged_cents: 0.0,
        });
        if event.pool == "api" {
            entry.api_requests += 1;
            entry.api_tokens += event.tokens;
        } else {
            entry.plan_requests += 1;
            entry.plan_tokens += event.tokens;
        }
        entry.charged_cents += event.charged_cents;
    }
    days.into_values().rev().collect()
}

fn utc_date(timestamp_ms: u64) -> String {
    let (year, month, day) = utc_ymd((timestamp_ms / 86_400_000) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

fn utc_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

fn iso_to_millis(value: Option<&str>) -> Option<u64> {
    parse_iso_millis(value?)
}

fn parse_iso_millis(value: &str) -> Option<u64> {
    if let Ok(millis) = value.parse::<u64>() {
        return Some(if millis < 1_000_000_000_000 {
            millis * 1000
        } else {
            millis
        });
    }
    let trimmed = value.trim().trim_end_matches('Z');
    let (date, time) = trimmed.split_once('T').unwrap_or((trimmed, "00:00:00"));
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    let time = time.split(['.', '+']).next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let minute = time_parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let second = time_parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let days = days_from_civil(year, month, day)?;
    Some((((days as u64) * 86_400) + hour * 3600 + minute * 60 + second) * 1000)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut year = year as i64;
    let month = month as i64;
    if month <= 2 {
        year -= 1;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let doy = (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    Some(era * 146_097 + doe as i64 - 719_468)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn value_f64(value: Option<&Value>) -> f64 {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
        .unwrap_or(0.0)
}

fn value_u64(value: Option<&Value>) -> u64 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_f64().map(|number| number.max(0.0) as u64))
                .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    #[test]
    fn extracts_cursor_user_id_without_exposing_token() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"auth0|user_123"}"#);
        let token = format!("header.{payload}.signature");
        assert_eq!(jwt_subject(&token).unwrap(), "auth0|user_123");
    }

    #[test]
    fn parses_plan_and_model_usage() {
        let summary = serde_json::json!({
            "membershipType": "pro",
            "billingCycleStart": "2026-07-01T00:00:00Z",
            "billingCycleEnd": "2026-08-01T00:00:00Z",
            "individualUsage": {
                "plan": {
                    "used": 120,
                    "limit": 500,
                    "remaining": 380,
                    "totalPercentUsed": 24,
                    "apiPercentUsed": 3
                },
                "onDemand": { "enabled": true, "used": 45 }
            }
        });
        let models = serde_json::json!({
            "gpt-test": {"numRequests": 12, "maxRequestUsage": 100, "numTokens": 3456},
            "startOfMonth": "2026-07-01"
        });
        let usage = parse_usage(Some("user@example.com".to_string()), &summary, &models).unwrap();
        assert_eq!(usage.membership_type, "pro");
        assert_eq!(usage.request_total, 12);
        assert_eq!(usage.token_total, 3456);
        assert_eq!(usage.models.len(), 1);
        assert!(usage.on_demand_enabled);
        assert_eq!(usage.on_demand_used, 45.0);
    }

    #[test]
    fn classifies_plan_and_api_events_and_buckets_by_day() {
        let payload = serde_json::json!({
            "totalUsageEventsCount": 2,
            "usageEventsDisplay": [
                {
                    "timestamp": "1751414400000",
                    "model": "claude-4-sonnet",
                    "kind": "USAGE_EVENT_KIND_INCLUDED_IN_PRO",
                    "tokenUsage": { "inputTokens": 10, "outputTokens": 20, "cacheReadTokens": 5, "totalCents": 1.5 },
                    "chargedCents": 1.5,
                    "isHeadless": false
                },
                {
                    "timestamp": "1751500800000",
                    "model": "gpt-5",
                    "kind": "USAGE_EVENT_KIND_USAGE_BASED",
                    "usageBasedCosts": "$0.12",
                    "tokenUsage": { "inputTokens": "100", "outputTokens": "200" },
                    "chargedCents": 12.0,
                    "isHeadless": true
                }
            ]
        });
        let events = parse_usage_events(&payload);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].pool, "plan");
        assert_eq!(events[0].tokens, 35);
        assert_eq!(events[1].pool, "api");
        assert_eq!(events[1].tokens, 300);
        assert_eq!(utc_date(1_751_414_400_000), "2025-07-02");
        let days = days_from_events(&events);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].api_requests, 1);
        assert_eq!(days[1].plan_tokens, 35);
    }

    #[test]
    fn parses_iso_timestamps_to_utc_dates() {
        let millis = parse_iso_millis("2026-07-01T00:00:00.000Z").unwrap();
        assert_eq!(utc_date(millis), "2026-07-01");
        assert_eq!(parse_iso_millis("1782864000000"), Some(1_782_864_000_000));
    }
}
