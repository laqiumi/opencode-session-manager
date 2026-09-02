use rusqlite::Connection;
use std::path::PathBuf;

use crate::{ChatItem, SessionInfo};

// 截断超长文本（按字符数，避免截断 UTF-8 多字节字符）
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max).collect::<String>())
}

pub fn session_messages(path: &str, id: &str) -> Result<Vec<ChatItem>, String> {
    let conn = open_readonly(path)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT m.data, p.data
            FROM message m
            LEFT JOIN part p ON p.message_id = m.id
            WHERE m.session_id = ?1
            ORDER BY m.time_created, m.id, p.time_created, p.id
            "#,
        )
        .map_err(|e| format!("查询失败: {}", e))?;

    let rows = stmt
        .query_map([id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("读取失败: {}", e))?;

    let mut items: Vec<ChatItem> = Vec::new();
    for row in rows {
        let (mraw, praw) = row.map_err(|e| format!("解析失败: {}", e))?;
        let role = serde_json::from_str::<serde_json::Value>(&mraw)
            .ok()
            .and_then(|v| v.get("role")?.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let Some(praw) = praw else { continue };
        let Ok(p) = serde_json::from_str::<serde_json::Value>(&praw) else {
            continue;
        };
        let get = |k: &str| p.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
        match p.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                let text = get("text").unwrap_or_default();
                if !text.trim().is_empty() {
                    items.push(ChatItem {
                        role: role.clone(),
                        kind: "text".into(),
                        text: Some(text),
                        tool: None,
                        input: None,
                        output: None,
                    });
                }
            }
            Some("tool") => {
                let state = p.get("state").cloned().unwrap_or_default();
                let input = state
                    .get("input")
                    .map(|v| {
                        // input 是对象：bash 取 command，其余整体序列化
                        v.get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| truncate_chars(&v.to_string(), 500))
                    });
                let output = state
                    .get("output")
                    .and_then(|o| o.as_str())
                    .map(|s| truncate_chars(s, 2000));
                items.push(ChatItem {
                    role: role.clone(),
                    kind: "tool".into(),
                    text: None,
                    tool: get("tool"),
                    input,
                    output,
                });
            }
            Some("reasoning") => {
                let text = get("text").unwrap_or_default();
                if !text.trim().is_empty() {
                    items.push(ChatItem {
                        role: role.clone(),
                        kind: "reasoning".into(),
                        text: Some(truncate_chars(&text, 4000)),
                        tool: None,
                        input: None,
                        output: None,
                    });
                }
            }
            // step-start/step-finish 等无展示价值，跳过
            _ => {}
        }
    }
    Ok(items)
}

pub fn default_db_path() -> String {
    if let Ok(p) = std::env::var("OPENCODE_DB_PATH") {
        return p;
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
        .to_string_lossy()
        .to_string()
}

fn open_readonly(path: &str) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("无法打开数据库 {}: {}", path, e))
}

fn folder_name(directory: &str) -> String {
    let d = directory.trim_end_matches(['/', '\\']);
    d.rsplit(['/', '\\']).next().unwrap_or(d).to_string()
}

pub fn list_sessions(path: &str) -> Result<Vec<SessionInfo>, String> {
    let conn = open_readonly(path)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                s.id, s.title, s.directory,
                s.model,
                s.time_created, s.time_updated,
                (SELECT COUNT(*) FROM message m WHERE m.session_id = s.id) AS message_count,
                (
                    SELECT pt.data FROM part pt
                    JOIN message pm ON pm.id = pt.message_id
                    WHERE pm.session_id = s.id
                      AND json_extract(pm.data, '$.role') = 'user'
                      AND json_extract(pt.data, '$.type') = 'text'
                      AND json_extract(pt.data, '$.text') IS NOT NULL
                      AND COALESCE(json_extract(pt.data, '$.synthetic'), 0) = 0
                    ORDER BY pt.time_created DESC
                    LIMIT 1
                ) AS last_user_part,
                (
                    SELECT GROUP_CONCAT(DISTINCT json_extract(m.data, '$.modelID'))
                    FROM message m
                    WHERE m.session_id = s.id
                      AND json_extract(m.data, '$.role') = 'assistant'
                      AND json_extract(m.data, '$.modelID') IS NOT NULL
                ) AS models_used
            FROM session s
            ORDER BY s.time_updated DESC
            "#,
        )
        .map_err(|e| format!("查询失败: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            let last_part: Option<String> = row.get(7)?;
            let last_user_message = last_part
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .and_then(|v| {
                    v.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty());

            let directory: String = row.get(2)?;
            let model: Option<String> = row.get(3)?;

            // 优先用消息级聚合的实际模型；无 assistant 消息时回退 session.model 里的 id
            let models_used: Option<String> = row.get(8)?;
            let models: Vec<String> = models_used
                .map(|csv| csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .filter(|v: &Vec<String>| !v.is_empty())
                .or_else(|| {
                    model.as_deref().and_then(|raw| {
                        serde_json::from_str::<serde_json::Value>(raw)
                            .ok()
                            .and_then(|v| v.get("id")?.as_str().map(|s| s.to_string()))
                            .map(|id| vec![id])
                    })
                })
                .unwrap_or_default();

            Ok(SessionInfo {
                source: String::new(),
                id: row.get(0)?,
                title: row.get(1)?,
                directory: directory.clone(),
                folder_name: folder_name(&directory),
                model,
                models,
                time_created: row.get(4)?,
                time_updated: row.get(5)?,
                message_count: row.get(6)?,
                last_user_message,
            })
        })
        .map_err(|e| format!("读取失败: {}", e))?;

    let mut sessions: Vec<SessionInfo> = Vec::new();
    for row in rows {
        let s = row.map_err(|e| format!("解析失败: {}", e))?;
        sessions.push(s);
    }

    Ok(sessions)
}

pub fn delete_session(path: &str, id: &str) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|e| format!("无法打开数据库: {}", e))?;    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| format!("设置外键失败: {}", e))?;

    let affected = conn
        .execute("DELETE FROM session WHERE id = ?1", [id])
        .map_err(|e| format!("删除失败: {}", e))?;

    if affected == 0 {
        return Err(format!("未找到会话 {}", id));
    }

    conn.execute("PRAGMA optimize", [])
        .map_err(|e| format!("优化失败: {}", e))?;

    Ok(())
}

pub fn update_session_directory(path: &str, id: &str, new_dir: &str) -> Result<(), String> {
    let conn = Connection::open(path).map_err(|e| format!("无法打开数据库: {}", e))?;
    let affected = conn
        .execute(
            "UPDATE session SET directory = ?1 WHERE id = ?2",
            rusqlite::params![new_dir, id],
        )
        .map_err(|e| format!("更新目录失败: {}", e))?;
    if affected == 0 {
        return Err(format!("未找到会话 {}", id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_sessions_reads_real_db() {
        let path = default_db_path();
        let sessions = list_sessions(&path).expect("应该能读取真实数据库");
        assert!(!sessions.is_empty(), "真实数据库不应为空");
        assert!(sessions.iter().any(|s| !s.directory.is_empty()));
        assert!(sessions.iter().any(|s| !s.folder_name.is_empty()));
        assert!(sessions.iter().all(|s| !s.id.is_empty()));

        let mut sorted = true;
        for w in sessions.windows(2) {
            if w[0].time_updated < w[1].time_updated {
                sorted = false;
            }
        }
        assert!(sorted, "应按时序倒序排列");
        println!("共 {} 个会话", sessions.len());
        for s in sessions.iter().take(5) {
            println!(
                "  {} | {} | {} | 上次: {:?}",
                s.title, s.folder_name, s.time_updated, s.last_user_message
            );
        }

        let with_last = sessions.iter().filter(|s| s.last_user_message.is_some()).count();
        println!("有最后用户消息的会话: {} / {}", with_last, sessions.len());

        let with_models = sessions.iter().filter(|s| !s.models.is_empty()).count();
        assert!(with_models > 0, "应至少有一个会话能解析出使用的模型");
        println!("有模型信息的会话: {} / {}", with_models, sessions.len());
    }

    #[test]
    fn test_session_messages_reads_real_db() {
        let path = default_db_path();
        let sessions = list_sessions(&path).expect("应该能读取真实数据库");
        let with_msgs = sessions.iter().find(|s| s.message_count > 0).expect("应有带消息的会话");
        let items = session_messages(&path, &with_msgs.id).expect("应该能读取会话消息");
        assert!(!items.is_empty(), "有消息的会话应解析出详情条目");
        assert!(items.iter().any(|i| i.kind == "text"), "应至少有一条文本消息");
    }

    #[test]
    fn test_default_db_path() {
        let p = default_db_path();
        assert!(p.ends_with("opencode.db"), "路径应以 opencode.db 结尾: {}", p);
        assert!(std::path::Path::new(&p).exists(), "数据库文件应存在: {}", p);
    }
}
