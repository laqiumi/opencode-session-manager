mod claude;
mod codex;
mod db;

use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    OpenCode,
    Codex,
    Claude,
}

pub struct AppState {
    pub opencode_db: Mutex<String>,
    pub codex_dir: Mutex<String>,
    pub claude_dir: Mutex<String>,
}

#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub source: String,
    pub id: String,
    pub title: String,
    pub directory: String,
    pub folder_name: String,
    pub model: Option<String>,
    pub time_created: i64,
    pub time_updated: i64,
    pub message_count: i64,
    pub last_user_message: Option<String>,
}

pub fn default_codex_dir() -> String {
    if let Ok(p) = std::env::var("CODEX_DIR") {
        return p;
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".codex")
        .to_string_lossy()
        .to_string()
}

pub fn default_claude_dir() -> String {
    if let Ok(p) = std::env::var("CLAUDE_CONFIG_DIR") {
        return p;
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
fn get_db_path(state: State<AppState>) -> String {
    state.opencode_db.lock().unwrap().clone()
}

#[tauri::command]
fn list_sessions(state: State<AppState>, source: Source) -> Result<Vec<SessionInfo>, String> {
    let mut sessions = match source {
        Source::OpenCode => {
            let path = state.opencode_db.lock().unwrap().clone();
            db::list_sessions(&path)?
        }
        Source::Codex => {
            let dir = state.codex_dir.lock().unwrap().clone();
            codex::list_sessions(&dir)?
        }
        Source::Claude => {
            let dir = state.claude_dir.lock().unwrap().clone();
            claude::list_sessions(&dir)?
        }
    };

    for s in sessions.iter_mut() {
        s.source = source_str(source).to_string();
        s.folder_name = folder_name(&s.directory);
    }

    sessions.sort_by(|a, b| b.time_updated.cmp(&a.time_updated));
    Ok(sessions)
}

#[tauri::command]
fn delete_session(state: State<AppState>, source: Source, id: String) -> Result<(), String> {
    match source {
        Source::OpenCode => {
            let path = state.opencode_db.lock().unwrap().clone();
            db::delete_session(&path, &id)
        }
        Source::Codex => {
            let dir = state.codex_dir.lock().unwrap().clone();
            codex::delete_session(&dir, &id)
        }
        Source::Claude => {
            let dir = state.claude_dir.lock().unwrap().clone();
            claude::delete_session(&dir, &id)
        }
    }
}

#[tauri::command]
fn open_folder(directory: String) -> Result<(), String> {
    open::that(&directory).map_err(|e| format!("无法打开文件夹: {}", e))
}

#[tauri::command]
fn continue_session(
    source: Source,
    directory: String,
    id: String,
    preset: Option<String>,
    custom_cmd: Option<String>,
) -> Result<String, String> {
    let dir = if directory.is_empty() {
        ".".to_string()
    } else {
        directory
    };

    let shell_cmd = match source {
        Source::OpenCode => format!("opencode -s {}", id),
        // Codex 是桌面客户端操作，无需在终端继续
        Source::Codex => {
            return Err("Codex 会话请直接在 Codex 客户端中继续".to_string());
        }
        Source::Claude => format!("claude --resume {}", id),
    };

    // 目录可能已被移动/删除，返回带前缀的错误供前端走自助修复流程
    if !std::path::Path::new(&dir).is_dir() {
        return Err(format!("DIR_NOT_FOUND:{}", dir));
    }

    spawn_in_terminal(&dir, &shell_cmd, preset.as_deref(), custom_cmd.as_deref())?;
    Ok(format!("已在新终端继续会话 {}", id))
}

// 用 Spotlight 按文件夹名搜可能迁移到的新位置，省得用户手输路径
#[tauri::command]
fn search_moved_directory(directory: String) -> Vec<String> {
    let name = directory
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return vec![];
    }
    let output = std::process::Command::new("mdfind")
        .args(["-name", &name])
        .output();
    let mut candidates: Vec<String> = output
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|p| {
            !p.is_empty()
                && p != &directory
                && p.rsplit(['/', '\\']).next() == Some(name.as_str())
                && std::path::Path::new(p).is_dir()
        })
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates.truncate(10);
    candidates
}

#[tauri::command]
fn update_session_directory(
    state: State<AppState>,
    source: Source,
    id: String,
    new_dir: String,
) -> Result<(), String> {
    if !std::path::Path::new(&new_dir).is_dir() {
        return Err(format!("目录不存在: {}", new_dir));
    }
    match source {
        Source::OpenCode => {
            let path = state.opencode_db.lock().unwrap().clone();
            db::update_session_directory(&path, &id, &new_dir)
        }
        // Codex/Claude 会话不存在 opencode.db 里，暂不支持改目录
        _ => Err("仅支持修复 OpenCode 会话的目录".to_string()),
    }
}

// shell 单引号转义，供自定义终端模板替换 {dir}/{cmd} 使用
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn source_str(source: Source) -> &'static str {
    match source {
        Source::OpenCode => "opencode",
        Source::Codex => "codex",
        Source::Claude => "claude",
    }
}

fn folder_name(directory: &str) -> String {
    let d = directory.trim_end_matches(['/', '\\']);
    d.rsplit(['/', '\\']).next().unwrap_or(d).to_string()
}

#[cfg(windows)]
fn spawn_in_terminal(
    dir: &str,
    shell_cmd: &str,
    _preset: Option<&str>,
    _custom_cmd: Option<&str>,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    // 优先用 Windows Terminal 开新标签页
    let wt = std::env::var("LOCALAPPDATA")
        .map(|p| std::path::Path::new(&p).join("Microsoft\\WindowsApps\\wt.exe"))
        .ok();

    let dir_arg = format!("/D \"{}\"", dir.replace('/', "\\"));

    if let Some(wt_path) = wt {
        if wt_path.exists() {
            let code = std::process::Command::new(&wt_path)
                .args(["-d", dir, "cmd", "/k", shell_cmd])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW，避免闪现黑框
                .spawn();
            if let Err(e) = code {
                return Err(format!("无法启动 Windows Terminal: {}", e));
            }
            return Ok(());
        }
    }

    // fallback: 传统 cmd 新控制台窗口
    let code = std::process::Command::new("cmd")
        .args(["/c", "start", "", dir_arg.as_str(), "cmd", "/k", shell_cmd])
        .creation_flags(0x08000000)
        .spawn();
    code.map_err(|e| format!("无法打开终端: {}", e))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_in_terminal(
    dir: &str,
    shell_cmd: &str,
    preset: Option<&str>,
    custom_cmd: Option<&str>,
) -> Result<(), String> {
    let full_cmd = format!("cd \"{}\" && {}", dir, shell_cmd);
    match preset.unwrap_or("terminal") {
        "ghostty" => {
            // Ghostty 不支持 AppleScript，macOS 上需用 open --args 传 -e；
            // sh -lc 保证加载用户 shell 配置的 PATH
            if !std::path::Path::new("/Applications/Ghostty.app").exists() {
                return Err("未安装 Ghostty（/Applications/Ghostty.app 不存在）".to_string());
            }
            std::process::Command::new("open")
                .args(["-na", "Ghostty.app", "--args", "-e", "sh", "-lc", &full_cmd])
                .spawn()
                .map_err(|e| format!("无法打开 Ghostty: {}", e))?;
            Ok(())
        }
        "custom" => {
            let tpl = custom_cmd.unwrap_or("").trim();
            if tpl.is_empty() {
                return Err("未配置自定义终端命令".to_string());
            }
            // {dir}/{cmd} 已做单引号转义，用户模板里直接裸写占位符即可
            let cmdline = tpl
                .replace("{dir}", &shell_quote(dir))
                .replace("{cmd}", &shell_quote(&full_cmd));
            std::process::Command::new("sh")
                .args(["-c", &cmdline])
                .spawn()
                .map_err(|e| format!("无法执行自定义终端命令: {}", e))?;
            Ok(())
        }
        _ => {
            // TUI 程序必须有 tty，直接 spawn sh 进程会在后台静默退出；
            // 用 AppleScript 让 Terminal.app 新开窗口执行，同时复用用户 shell 的 PATH
            let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                "tell application \"Terminal\" to do script \"{}\"",
                esc(&full_cmd)
            );
            std::process::Command::new("osascript")
                .args([
                    "-e",
                    &script,
                    "-e",
                    "tell application \"Terminal\" to activate",
                ])
                .spawn()
                .map_err(|e| format!("无法打开终端: {}", e))?;
            Ok(())
        }
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn spawn_in_terminal(
    dir: &str,
    shell_cmd: &str,
    _preset: Option<&str>,
    _custom_cmd: Option<&str>,
) -> Result<(), String> {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("cd \"{}\" && {}", dir, shell_cmd))
        .spawn()
        .map_err(|e| format!("无法打开终端: {}", e))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            opencode_db: Mutex::new(db::default_db_path()),
            codex_dir: Mutex::new(default_codex_dir()),
            claude_dir: Mutex::new(default_claude_dir()),
        })
        .invoke_handler(tauri::generate_handler![
            get_db_path,
            list_sessions,
            delete_session,
            open_folder,
            continue_session,
            search_moved_directory,
            update_session_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
