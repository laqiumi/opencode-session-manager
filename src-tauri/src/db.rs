use rusqlite::Connection;
use std::path::PathBuf;

use crate::SessionInfo;

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
                s.id, s.title, s.directory, p.name,
                s.model, s.agent,
                s.time_created, s.time_updated, s.time_archived,
                (SELECT COUNT(*) FROM message m WHERE m.session_id = s.id) AS message_count,
                s.cost, s.tokens_input, s.tokens_output
            FROM session s
            LEFT JOIN project p ON p.id = s.project_id
            ORDER BY s.time_updated DESC
            "#,
        )
        .map_err(|e| format!("查询失败: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SessionInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                directory: row.get(2)?,
                project_name: row.get(3)?,
                model: row.get(4)?,
                agent: row.get(5)?,
                time_created: row.get(6)?,
                time_updated: row.get(7)?,
                time_archived: row.get(8)?,
                message_count: row.get(9)?,
                cost: row.get(10)?,
                tokens_input: row.get(11)?,
                tokens_output: row.get(12)?,
                folder_name: String::new(),
            })
        })
        .map_err(|e| format!("读取失败: {}", e))?;

    let mut sessions: Vec<SessionInfo> = Vec::new();
    for row in rows {
        let mut s = row.map_err(|e| format!("解析失败: {}", e))?;
        s.folder_name = folder_name(&s.directory);
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
            println!("  {} | {} | {}", s.title, s.folder_name, s.time_updated);
        }
    }

    #[test]
    fn test_default_db_path() {
        let p = default_db_path();
        assert!(p.ends_with("opencode.db"), "路径应以 opencode.db 结尾: {}", p);
        assert!(std::path::Path::new(&p).exists(), "数据库文件应存在: {}", p);
    }
}
