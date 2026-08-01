import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import type { SessionInfo } from "./types";
import "./App.css";

interface FolderStat {
  name: string;
  count: number;
  lastActive: number;
}

function formatTime(ts: number): string {
  if (!ts) return "—";
  const d = new Date(ts);
  const now = Date.now();
  const diff = now - ts;
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} 天前`;
  return d.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

function formatFullTime(ts: number): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return `${n}`;
}

function App() {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dbPath, setDbPath] = useState("");
  const [search, setSearch] = useState("");
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [appVersion, setAppVersion] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => {});
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [list, path] = await Promise.all([
        invoke<SessionInfo[]>("list_sessions"),
        invoke<string>("get_db_path"),
      ]);
      setSessions(list);
      setDbPath(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const folders = useMemo<FolderStat[]>(() => {
    const map = new Map<string, FolderStat>();
    for (const s of sessions) {
      const cur = map.get(s.folder_name);
      if (!cur) {
        map.set(s.folder_name, {
          name: s.folder_name,
          count: 1,
          lastActive: s.time_updated,
        });
      } else {
        cur.count += 1;
        cur.lastActive = Math.max(cur.lastActive, s.time_updated);
      }
    }
    return [...map.values()].sort((a, b) => b.lastActive - a.lastActive);
  }, [sessions]);

  const filtered = useMemo(() => {
    let list = sessions;
    if (selectedFolder) {
      list = list.filter((s) => s.folder_name === selectedFolder);
    }
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      list = list.filter(
        (s) =>
          s.title.toLowerCase().includes(q) ||
          s.directory.toLowerCase().includes(q) ||
          s.id.toLowerCase().includes(q),
      );
    }
    return list;
  }, [sessions, selectedFolder, search]);

  const handleDelete = useCallback(
    async (s: SessionInfo) => {
      if (!confirm(`确定删除会话「${s.title}」？\n删除后不可恢复。`)) return;
      setDeleting(s.id);
      try {
        await invoke("delete_session", { id: s.id });
        setSessions((prev) => prev.filter((x) => x.id !== s.id));
      } catch (e) {
        alert(`删除失败：${e}`);
      } finally {
        setDeleting(null);
      }
    },
    [],
  );

  const handleContinue = useCallback(
    (s: SessionInfo) => {
      invoke("continue_session", { directory: s.directory, id: s.id }).catch((e) =>
        alert(`无法继续会话：${e}`),
      );
    },
    [],
  );

  const handleOpenFolder = useCallback(
    (s: SessionInfo) => {
      invoke("open_folder", { directory: s.directory }).catch((e) =>
        alert(`无法打开文件夹：${e}`),
      );
    },
    [],
  );

  const handleCopyId = useCallback(
    async (id: string) => {
      await navigator.clipboard.writeText(id).catch(() => {});
    },
    [],
  );

  const totalTokens = useMemo(
    () =>
      sessions.reduce((acc, s) => acc + s.tokens_input + s.tokens_output, 0),
    [sessions],
  );

  const totalMessages = useMemo(
    () => sessions.reduce((acc, s) => acc + s.message_count, 0),
    [sessions],
  );

  const totalCost = useMemo(
    () => sessions.reduce((acc, s) => acc + s.cost, 0),
    [sessions],
  );

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="logo">
            <span className="logo-mark">⌘</span>
          </div>
          <div>
            <h1 className="app-title">OpenCode 会话管理</h1>
            <p className="app-subtitle">
              {sessions.length} 个会话 · {folders.length} 个文件夹
            </p>
          </div>
        </div>

        <nav className="folder-nav">
          <button
            className={`folder-item ${selectedFolder === null ? "active" : ""}`}
            onClick={() => setSelectedFolder(null)}
          >
            <span className="folder-icon">🗂</span>
            <span className="folder-name">全部会话</span>
            <span className="folder-count">{sessions.length}</span>
          </button>
          {folders.map((f) => (
            <button
              key={f.name}
              className={`folder-item ${selectedFolder === f.name ? "active" : ""}`}
              onClick={() => setSelectedFolder(f.name)}
              title={`最后活跃 ${formatTime(f.lastActive)}`}
            >
              <span className="folder-icon">📁</span>
              <span className="folder-name">{f.name}</span>
              <span className="folder-count">{f.count}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <p className="db-path" title={dbPath}>
            {dbPath || "加载中…"}
          </p>
        </div>
      </aside>

      <main className="main">
        <header className="toolbar">
          <div className="search-box">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
              <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="2" />
              <path
                d="m20 20-3.5-3.5"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
            <input
              ref={searchRef}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索标题 / 路径 / 会话 ID…"
              spellCheck={false}
            />
            {search && (
              <button className="search-clear" onClick={() => setSearch("")}>
                ×
              </button>
            )}
          </div>

          <div className="toolbar-stats">
            <span title="总 Token 消耗">
              ≈ {formatTokens(totalTokens)} tokens
            </span>
          </div>

          <button className="refresh-btn" onClick={load} disabled={loading}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <path
                d="M21 12a9 9 0 1 1-2.64-6.36"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
              <path
                d="M21 3v6h-6"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            {loading ? "刷新中…" : "刷新"}
          </button>

          <button className="refresh-btn settings-btn" onClick={() => setSettingsOpen(true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <path
                d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"
                stroke="currentColor"
                strokeWidth="2"
              />
              <path
                d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
            设置
          </button>
        </header>

        <div className="list-header">
          <div className="col-title">会话</div>
          <div className="col-time">最后运行</div>
          <div className="col-actions">操作</div>
        </div>

        <div className="list">
          {loading && (
            <div className="state">
              <div className="spinner" />
              <p>正在读取 opencode 数据库…</p>
            </div>
          )}

          {!loading && error && (
            <div className="state error">
              <p>读取失败</p>
              <code>{error}</code>
              <button className="refresh-btn" onClick={load}>
                重试
              </button>
            </div>
          )}

          {!loading && !error && filtered.length === 0 && (
            <div className="state">
              <p>没有匹配的会话</p>
            </div>
          )}

          {!loading &&
            !error &&
            filtered.map((s) => (
              <div className="session-row" key={s.id}>
                <div className="session-main">
                  <div className="session-title-row">
                    <span className="session-title">{s.title || "(无标题)"}</span>
                    {s.agent && <span className="tag agent">{s.agent}</span>}
                    {s.model && <span className="tag model">{s.model}</span>}
                  </div>
                  {s.last_user_message && (
                    <div className="session-last" title={s.last_user_message}>
                      <span className="last-label">上次：</span>
                      {s.last_user_message}
                    </div>
                  )}
                  <div className="session-meta">
                    <span className="meta-item" title={s.directory}>
                      <span className="meta-icon">📁</span>
                      {s.directory}
                    </span>
                    <span className="meta-item" title={s.id}>
                      <span className="meta-icon">🆔</span>
                      {s.id}
                    </span>
                    <span className="meta-item" title={`${s.message_count} 条消息`}>
                      <span className="meta-icon">💬</span>
                      {s.message_count}
                    </span>
                    <span className="meta-item" title="Token 消耗">
                      <span className="meta-icon">⚡</span>
                      {formatTokens(s.tokens_input + s.tokens_output)}
                    </span>
                  </div>
                </div>

                <div className="session-time">
                  <span className="time-rel" title={`创建于 ${formatFullTime(s.time_created)}`}>
                    {formatTime(s.time_updated)}
                  </span>
                  <span className="time-full">
                    {formatFullTime(s.time_updated)}
                  </span>
                </div>

                <div className="session-actions">
                  <button
                    className="act-btn"
                    onClick={() => handleContinue(s)}
                    title="在 Windows Terminal 中打开该文件夹并 opencode -s 继续此会话"
                  >
                    ▶ 继续
                  </button>
                  <button
                    className="act-btn"
                    onClick={() => handleOpenFolder(s)}
                    title="打开所在文件夹"
                  >
                    📁 文件夹
                  </button>
                  <button
                    className="act-btn"
                    onClick={() => handleCopyId(s.id)}
                    title="复制会话 ID"
                  >
                    ⧉ ID
                  </button>
                  <button
                    className="act-btn danger"
                    onClick={() => handleDelete(s)}
                    disabled={deleting === s.id}
                    title="删除会话"
                  >
                    {deleting === s.id ? "删除中…" : "🗑 删除"}
                  </button>
                </div>
              </div>
            ))}
        </div>
      </main>

      {settingsOpen && (
        <div className="modal-overlay" onClick={() => setSettingsOpen(false)}>
          <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
            <div className="settings-header">
              <h2>设置与关于</h2>
              <button className="search-clear close-btn" onClick={() => setSettingsOpen(false)}>
                ×
              </button>
            </div>

            <div className="settings-body">
              <section className="settings-section">
                <h3>应用信息</h3>
                <div className="info-row">
                  <span className="info-label">名称</span>
                  <span className="info-value">OpenCode 会话管理器</span>
                </div>
                <div className="info-row">
                  <span className="info-label">版本</span>
                  <span className="info-value">
                    v{appVersion || "未知"}
                    {appVersion === "0.1.0" ? (
                      <span className="tag model" style={{ marginLeft: 8 }}>当前版本</span>
                    ) : null}
                  </span>
                </div>
                <div className="info-row">
                  <span className="info-label">技术栈</span>
                  <span className="info-value">Tauri 2 · React 19 · rusqlite</span>
                </div>
              </section>

              <section className="settings-section">
                <h3>数据统计</h3>
                <div className="stats-grid">
                  <div className="stat-card">
                    <div className="stat-num">{sessions.length}</div>
                    <div className="stat-label">会话</div>
                  </div>
                  <div className="stat-card">
                    <div className="stat-num">{folders.length}</div>
                    <div className="stat-label">文件夹</div>
                  </div>
                  <div className="stat-card">
                    <div className="stat-num">{formatTokens(totalTokens)}</div>
                    <div className="stat-label">Tokens</div>
                  </div>
                  <div className="stat-card">
                    <div className="stat-num">{formatTokens(totalMessages)}</div>
                    <div className="stat-label">消息</div>
                  </div>
                </div>
              </section>

              <section className="settings-section">
                <h3>数据库</h3>
                <div className="info-row db-row">
                  <span className="info-label">路径</span>
                  <span className="info-value mono">{dbPath || "加载中…"}</span>
                </div>
                <div className="info-row">
                  <span className="info-label">累计消耗</span>
                  <span className="info-value">
                    {totalCost.toFixed(4)} USD · ≈ {formatTokens(totalTokens)} tokens
                  </span>
                </div>
                <div className="settings-actions">
                  <button
                    className="refresh-btn"
                    onClick={() =>
                      invoke("open_folder", {
                        directory: dbPath.replace(/\\[^\\]+$/, ""),
                      }).catch((e) => alert(`无法打开文件夹：${e}`))
                    }
                    disabled={!dbPath}
                  >
                    📂 打开数据目录
                  </button>
                  <button className="refresh-btn" onClick={() => setSettingsOpen(false)}>
                    刷新列表
                  </button>
                </div>
              </section>

              <section className="settings-section">
                <h3>关于更新</h3>
                <p className="settings-note">
                  更新软件：重新运行 <code>npm run tauri build</code> 生成安装包，覆盖安装即可。版本号在
                  <code> package.json </code> 中修改。
                </p>
              </section>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
