use serde_json::Value;
use std::path::Path;

use crate::SessionInfo;

pub fn list_sessions(claude_dir: &str) -> Result<Vec<SessionInfo>, String> {
    let projects_dir = Path::new(claude_dir).join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions: Vec<SessionInfo> = Vec::new();

    for entry in std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("读取 claude projects 目录失败: {}", e))?
        .flatten()
    {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let folder_code = entry.file_name().to_string_lossy().to_string();
        let decoded_dir = decode_folder(&folder_code);

        for file in std::fs::read_dir(&dir_path)
            .map_err(|e| format!("读取目录失败: {}", e))?
            .flatten()
        {
            let file_path = file.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let id = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }

            let parsed = parse_jsonl(&file_path);
            let directory = parsed
                .real_path
                .clone()
                .unwrap_or_else(|| decoded_dir.clone());

            sessions.push(SessionInfo {
                source: "claude".to_string(),
                id,
                title: parsed
                    .last_user_message
                    .clone()
                    .unwrap_or_else(|| "Claude 会话".to_string()),
                directory: directory.clone(),
                folder_name: String::new(),
                model: parsed.model.clone(),
                models: parsed.model.clone().map(|m| vec![m]).unwrap_or_default(),
                time_created: parsed.time_created,
                time_updated: parsed.time_updated,
                message_count: parsed.message_count,
                last_user_message: parsed.last_user_message,
            });
        }
    }

    Ok(sessions)
}

pub fn delete_session(claude_dir: &str, id: &str) -> Result<(), String> {
    let projects_dir = Path::new(claude_dir).join("projects");
    if !projects_dir.exists() {
        return Err("claude projects 目录不存在".to_string());
    }

    for entry in std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("读取目录失败: {}", e))?
        .flatten()
    {
        if !entry.path().is_dir() {
            continue;
        }
        let candidate = entry.path().join(format!("{}.jsonl", id));
        if candidate.exists() {
            std::fs::remove_file(&candidate)
                .map_err(|e| format!("删除会话文件失败: {}", e))?;
            return Ok(());
        }
    }

    Err(format!("未找到会话 {}", id))
}

struct ParsedSession {
    last_user_message: Option<String>,
    real_path: Option<String>,
    model: Option<String>,
    time_created: i64,
    time_updated: i64,
    message_count: i64,
}

fn parse_jsonl(path: &Path) -> ParsedSession {
    let mut last_user_message: Option<String> = None;
    let mut real_path: Option<String> = None;
    let mut model: Option<String> = None;
    let mut time_created: i64 = 0;
    let mut time_updated: i64 = 0;
    let mut message_count: i64 = 0;

    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let ts = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| parse_iso_time(s))
                .unwrap_or(0);
            if ts > 0 {
                if time_created == 0 {
                    time_created = ts;
                }
                time_updated = ts;
            }

            let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");

            if t == "user" {
                // 提取真实路径（从附件/消息内容里的 Windows 路径）
                let raw = serde_json::to_string(&v).unwrap_or_default();
                if real_path.is_none() {
                    real_path = extract_windows_path(&raw);
                }

                if let Some(content) = extract_user_text(&v) {
                    message_count += 1;
                    last_user_message = Some(content);
                }
            } else if t == "assistant" {
                message_count += 1;
                if model.is_none() {
                    model = v
                        .get("message")
                        .and_then(|m| m.get("model"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string());
                }
            }
        }
    }

    // title 兜底用最后用户消息
    ParsedSession {
        last_user_message,
        real_path,
        model,
        time_created,
        time_updated,
        message_count,
    }
}

fn extract_user_text(v: &Value) -> Option<String> {
    let content = v.get("message")?.get("content")?;
    match content {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Array(parts) => {
            let mut texts: Vec<String> = Vec::new();
            for part in parts {
                if part.get("type").and_then(|x| x.as_str()) == Some("text") {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            texts.push(text.trim().to_string());
                        }
                    }
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        _ => None,
    }
}

fn extract_windows_path(raw: &str) -> Option<String> {
    // 从工具调用 JSON 里扫描提取 Windows 路径，取最长最合理的那个
    // 匹配模式：盘符冒号 + 斜杠 + 至少一段路径
    let chars: Vec<char> = raw.chars().collect();
    let mut best: Option<String> = None;

    for i in 0..chars.len().saturating_sub(2) {
        let is_drive = chars[i].is_ascii_alphabetic()
            && chars[i + 1] == ':'
            && (chars[i + 2] == '\\' || chars[i + 2] == '/');
        if !is_drive {
            continue;
        }

        // 从 i 开始向两边扩展合法的路径字符
        let mut start = i;
        while start > 0 {
            let c = chars[start - 1];
            let ok = c.is_alphanumeric()
                || matches!(c, '\\' | '/' | '.' | '-' | '_' | ' ' | '（' | '）' | '(' | ')');
            if ok {
                start -= 1;
            } else {
                break;
            }
        }

        let mut end = i + 3;
        while end < chars.len() {
            let c = chars[end];
            let ok = c.is_alphanumeric()
                || matches!(c, '\\' | '/' | '.' | '-' | '_' | ' ' | '（' | '）' | '(' | ')');
            if ok {
                end += 1;
            } else {
                break;
            }
        }

        let candidate: String = chars[start..end].iter().collect();
        // 要求至少包含一层目录分隔
        if candidate.contains('\\') || candidate.contains('/') {
            if best.as_ref().map(|b| candidate.len() > b.len()).unwrap_or(true) {
                best = Some(candidate);
            }
        }
    }

    best
}

fn parse_iso_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis())
        .ok()
}

fn decode_folder(code: &str) -> String {
    // Claude Code 目录名编码：把路径中的 `:` 和 `\` 替换为 `-`
    // 解码时无法还原原始分隔符，这里用最合理的规则：
    // "D--project-real" -> "D:\project\real"
    let s = code.trim_start_matches("-").to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();

    // 规则：开头 `X-` 表示盘符冒号，后续每个 `-` 尝试作为 `\` 分隔
    if chars.len() >= 2 && chars[1] == '-' {
        out.push(chars[0]);
        out.push(':');
        let mut i = 2;
        while i < chars.len() {
            if chars[i] == '-' {
                out.push('\\');
                i += 1;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        return out;
    }

    // 无盘符的情况，直接按 `-` 分隔成路径段
    for c in chars {
        if c == '-' {
            out.push('\\');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_folder() {
        assert_eq!(decode_folder("D--project-real"), r"D:\project\real");
        assert_eq!(decode_folder("C--Users-93530"), r"C:\Users\93530");
    }

    #[test]
    fn test_claude_reads_real_dir() {
        let dir = crate::default_claude_dir();
        let sessions = list_sessions(&dir).expect("应能读取 claude 数据");
        if sessions.is_empty() {
            println!("claude 目录为空或不存在，跳过");
            return;
        }
        assert!(sessions.iter().any(|s| !s.id.is_empty()));
        let with_last = sessions.iter().filter(|s| s.last_user_message.is_some()).count();
        println!("claude 会话: {}，有最后消息: {}", sessions.len(), with_last);
        for s in sessions.iter().take(5) {
            println!("  {} | {} | {:?}", s.title, s.directory, s.last_user_message);
        }
    }
}
