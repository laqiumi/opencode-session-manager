use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::SessionInfo;

const INDEX_FILE: &str = "session_index.jsonl";

pub fn list_sessions(codex_dir: &str) -> Result<Vec<SessionInfo>, String> {
    let index_path = Path::new(codex_dir).join(INDEX_FILE);
    if !index_path.exists() {
        return Ok(Vec::new());
    }

    let index: Vec<IndexEntry> = read_jsonl(&index_path)?
        .into_iter()
        .filter_map(|v| {
            let id = v.get("id")?.as_str()?.to_string();
            let title = v
                .get("thread_name")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let updated = parse_iso_time(v.get("updated_at")?.as_str()?);
            Some(IndexEntry { id, title, updated })
        })
        .collect();

    let sessions_dir = Path::new(codex_dir).join("sessions");
    let mut sessions: Vec<SessionInfo> = Vec::new();

    for entry in &index {
        if let Some(file) = find_rollout(&sessions_dir, &entry.id) {
            let meta = read_rollout_meta(&file);
            let directory = meta
                .cwd
                .unwrap_or_else(|| "未知目录".to_string());
            let last_msg = meta.last_user_message;
            let created = meta.created.unwrap_or(entry.updated);

            sessions.push(SessionInfo {
                source: "codex".to_string(),
                id: entry.id.clone(),
                title: entry.title.clone(),
                directory: directory.clone(),
                folder_name: String::new(),
                model: meta.model,
                time_created: created,
                time_updated: entry.updated,
                message_count: meta.message_count,
                last_user_message: last_msg,
            });
        }
    }

    // 有的 session 在索引里但 rollout 找不到（已归档），补上索引里的信息
    for entry in &index {
        if !sessions.iter().any(|s| s.id == entry.id) {
            sessions.push(SessionInfo {
                source: "codex".to_string(),
                id: entry.id.clone(),
                title: entry.title.clone(),
                directory: "未知目录".to_string(),
                folder_name: String::new(),
                model: None,
                time_created: entry.updated,
                time_updated: entry.updated,
                message_count: 0,
                last_user_message: None,
            });
        }
    }

    Ok(sessions)
}

pub fn delete_session(codex_dir: &str, id: &str) -> Result<(), String> {
    let sessions_dir = Path::new(codex_dir).join("sessions");
    if let Some(file) = find_rollout(&sessions_dir, id) {
        std::fs::remove_file(&file)
            .map_err(|e| format!("删除会话文件失败: {}", e))?;
    }

    // 同时从索引中移除
    let index_path = Path::new(codex_dir).join(INDEX_FILE);
    if index_path.exists() {
        let lines: Vec<String> = std::fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引失败: {}", e))?
            .lines()
            .filter(|line| {
                !line.contains(&format!("\"id\":\"{}\"", id))
                    && !line.contains(&format!("\"id\": \"{}\"", id))
            })
            .map(|l| l.to_string())
            .collect();
        std::fs::write(&index_path, lines.join("\n"))
            .map_err(|e| format!("更新索引失败: {}", e))?;
    }

    Ok(())
}

struct IndexEntry {
    id: String,
    title: String,
    updated: i64,
}

struct RolloutMeta {
    cwd: Option<String>,
    last_user_message: Option<String>,
    created: Option<i64>,
    model: Option<String>,
    message_count: i64,
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("读取失败: {}", e))?;
    Ok(content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect())
}

fn parse_iso_time(s: &str) -> i64 {
    // ISO8601 格式 "2026-07-31T15:57:50.0357993Z"，直接解析为毫秒时间戳
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

fn find_rollout(sessions_dir: &Path, id: &str) -> Option<PathBuf> {
    // rollout 文件名格式: rollout-YYYY-MM-DDTHH-MM-SS-<id>.jsonl
    // 遍历 sessions 目录（可能按 年/月/日 组织）
    let mut best: Option<PathBuf> = None;
    walk(sessions_dir, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
            && path.file_name().map(|n| n.to_string_lossy().contains(id)).unwrap_or(false)
        {
            best = Some(path.to_path_buf());
        }
    });
    best
}

fn walk(dir: &Path, f: &mut impl FnMut(&Path)) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, f);
            } else {
                f(&path);
            }
        }
    }
}

fn read_rollout_meta(file: &Path) -> RolloutMeta {
    let mut cwd: Option<String> = None;
    let mut last_user_message: Option<String> = None;
    let mut created: Option<i64> = None;
    let mut model: Option<String> = None;
    let mut message_count: i64 = 0;

    if let Ok(content) = std::fs::read_to_string(file) {
        for line in content.lines() {
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let t = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

            if t == "session_meta" {
                let payload = &v["payload"];
                cwd = payload.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());
                created = payload
                    .get("timestamp")
                    .and_then(|ts| ts.as_str())
                    .and_then(|s| Some(parse_iso_time(s)))
                    .or(created);
                model = payload.get("model_provider").and_then(|m| m.as_str()).map(|s| s.to_string());
            } else if t == "response_item" {
                let payload = &v["payload"];
                if payload.get("type").and_then(|x| x.as_str()) == Some("message") {
                    let role = payload.get("role").and_then(|r| r.as_str());
                    message_count += 1;
                    if role == Some("user") {
                        // 真正的用户消息带 internal_chat_message 标记
                        let is_real_user = payload
                            .get("internal_chat_message")
                            .is_some()
                            || payload
                                .get("internal_chat_message_metadata_passthrough")
                                .is_some();
                        if is_real_user {
                            if let Some(text) = extract_text(payload) {
                                last_user_message = Some(text);
                            }
                        }
                    }
                }
            }
        }
    }

    RolloutMeta {
        cwd,
        last_user_message,
        created,
        model,
        message_count,
    }
}

fn extract_text(payload: &Value) -> Option<String> {
    let content = payload.get("content")?.as_array()?;
    let mut texts: Vec<String> = Vec::new();
    for part in content {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            if !text.trim().is_empty()
                && !text.starts_with("<environment_context>")
                && !text.starts_with("<permissions")
            {
                texts.push(text.trim().to_string());
            }
        }
    }
    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_reads_real_dir() {
        let dir = crate::default_codex_dir();
        let sessions = list_sessions(&dir).expect("应能读取 codex 数据");
        if sessions.is_empty() {
            println!("codex 目录为空或不存在，跳过");
            return;
        }
        assert!(sessions.iter().any(|s| !s.id.is_empty()));
        assert!(sessions.iter().all(|s| s.time_updated > 0));
        let with_last = sessions.iter().filter(|s| s.last_user_message.is_some()).count();
        println!("codex 会话: {}，有最后消息: {}", sessions.len(), with_last);
        for s in sessions.iter().take(3) {
            println!("  {} | {} | {:?}", s.title, s.directory, s.last_user_message);
        }
    }
}
