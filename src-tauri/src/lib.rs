mod db;

use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub db_path: Mutex<String>,
}

#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub folder_name: String,
    pub project_name: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub time_created: i64,
    pub time_updated: i64,
    pub time_archived: Option<i64>,
    pub message_count: i64,
    pub cost: f64,
    pub tokens_input: i64,
    pub tokens_output: i64,
}

#[tauri::command]
fn get_db_path(state: State<AppState>) -> String {
    state.db_path.lock().unwrap().clone()
}

#[tauri::command]
fn list_sessions(state: State<AppState>) -> Result<Vec<SessionInfo>, String> {
    let path = state.db_path.lock().unwrap().clone();
    db::list_sessions(&path)
}

#[tauri::command]
fn delete_session(state: State<AppState>, id: String) -> Result<(), String> {
    let path = state.db_path.lock().unwrap().clone();
    db::delete_session(&path, &id)
}

#[tauri::command]
fn open_folder(directory: String) -> Result<(), String> {
    open::that(&directory).map_err(|e| format!("无法打开文件夹: {}", e))
}

#[tauri::command]
fn continue_session(
    directory: String,
    id: String,
) -> Result<String, String> {
    let dir = if directory.is_empty() {
        ".".to_string()
    } else {
        directory
    };

    let shell_cmd = format!("opencode -s {}", id);
    spawn_in_terminal(&dir, &shell_cmd)?;
    Ok(format!("已在新终端继续会话 {}", id))
}

#[cfg(windows)]
fn spawn_in_terminal(dir: &str, shell_cmd: &str) -> Result<(), String> {
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

#[cfg(not(windows))]
fn spawn_in_terminal(dir: &str, shell_cmd: &str) -> Result<(), String> {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("cd \"{}\" && {}", dir, shell_cmd))
        .spawn()
        .map_err(|e| format!("无法打开终端: {}", e))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = db::default_db_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db_path: Mutex::new(db_path),
        })
        .invoke_handler(tauri::generate_handler![
            get_db_path,
            list_sessions,
            delete_session,
            open_folder,
            continue_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
