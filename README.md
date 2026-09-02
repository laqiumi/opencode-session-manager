# OpenCode Session Manager · OpenCode 会话管理器

[简体中文](#简体中文) · [English](#english)

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.x-orange)
![React](https://img.shields.io/badge/React-19-61dafb)

---

## 简体中文

### 简介

跨文件夹管理所有 AI 编程会话的桌面应用。聚合 **OpenCode**、**Codex**、**Claude** 三个来源的会话记录，统一检索、查看详情、一键续聊。

技术栈：Tauri 2 · React 19 · TypeScript · rusqlite（SQLite 直读，无外部服务依赖）。

### 功能

| 模块 | 说明 |
|---|---|
| 会话聚合 | OpenCode 读全局 `opencode.db`；Codex 读 `session_index`；Claude 读 `projects/` 目录 |
| 分组筛选 | 左侧按**文件夹**、按**时间**（今天/昨天/近 7 天/近 30 天/更早）分组统计；顶部按标题/路径/会话 ID 搜索 |
| 会话卡片 | 标题、目录、实际使用的模型（按 assistant 消息聚合）、消息数、Token 消耗、最后用户消息 |
| 会话详情 | 弹窗展示完整对话：角色气泡、长文本折叠展开、工具调用简录（可展开 input/output）、思考过程折叠 |
| 一键继续 | 新终端窗口执行 `opencode -s <id>` 续聊；终端可配置：Terminal.app / Ghostty / 自定义命令模板（`{dir}` `{cmd}` 占位符） |
| 目录修复 | 文件夹被移动后，Spotlight 自动搜索候选新路径，确认后写回数据库并自动续聊 |
| 会话管理 | 打开所在文件夹、复制会话 ID、删除会话（外键级联清理关联消息，有确认） |
| 皮肤 | 明亮 / 深色一键切换，选择持久化 |

### 安装步骤（macOS）

**1. 环境依赖**

- Node.js 18+：`brew install node`
- Rust 工具链（任选其一）：
  - `brew install rust`
  - 或 rustup（国内建议清华镜像）：
    ```bash
    curl -fL -o /tmp/rustup-init https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup/dist/aarch64-apple-darwin/rustup-init
    chmod +x /tmp/rustup-init
    RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup \
    RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup \
    /tmp/rustup-init -y --default-toolchain stable --profile minimal
    source "$HOME/.cargo/env"
    ```
- Xcode Command Line Tools：`xcode-select --install`（一般已有）

**2. 构建**

```bash
git clone https://github.com/laqiumi/opencode-session-manager.git
cd opencode-session-manager
npm install
npm run tauri build
```

首次构建约需 20~30 分钟（编译 Rust 依赖），产物位于：

- App：`src-tauri/target/release/bundle/macos/opencode-session-manager.app`
- DMG：`src-tauri/target/release/bundle/dmg/`

**3. 安装**

```bash
cp -R src-tauri/target/release/bundle/macos/opencode-session-manager.app /Applications/
xattr -dr com.apple.quarantine /Applications/opencode-session-manager.app
```

之后从启动台打开即可。

### 使用说明

- 首次点击「▶ 继续」时，macOS 可能请求「允许控制终端」，请点击允许
- `opencode` 命令需在 shell 的 PATH 中（终端新窗口会加载你的 shell 配置）
- 终端偏好：设置 → 终端，支持 Terminal.app / Ghostty / 自定义命令模板

### 开发

```bash
npm install
npm run tauri dev        # 热重载调试
cd src-tauri && cargo test   # 后端测试
```

### 数据来源

- OpenCode：`~/.local/share/opencode/opencode.db`（环境变量 `OPENCODE_DB_PATH` 可覆盖）
- Codex：`~/.codex`（环境变量 `CODEX_DIR` 可覆盖）
- Claude：`~/.claude`（环境变量 `CLAUDE_CONFIG_DIR` 可覆盖）
- 删除会话直接操作 SQLite，外键级联清理关联消息

---

## English

### Introduction

A desktop app to manage all your AI coding sessions across folders. It aggregates sessions from **OpenCode**, **Codex**, and **Claude** into one place for searching, inspecting, and resuming.

Tech stack: Tauri 2 · React 19 · TypeScript · rusqlite (reads SQLite directly, no external services).

### Features

| Module | Description |
|---|---|
| Session aggregation | OpenCode via global `opencode.db`; Codex via `session_index`; Claude via `projects/` directory |
| Grouping & filtering | Sidebar groups by **folder** and by **time** (Today / Yesterday / Last 7 days / Last 30 days / Earlier); search by title, path, or session ID |
| Session cards | Title, directory, models actually used (aggregated from assistant messages), message count, token usage, last user message |
| Session detail | Dialog with the full conversation: role bubbles, collapsible long text, one-line tool call summaries (expandable input/output), folded reasoning |
| One-click resume | Opens a new terminal window running `opencode -s <id>`; configurable terminal: Terminal.app / Ghostty / custom command template (`{dir}` `{cmd}` placeholders) |
| Directory repair | If a folder was moved, Spotlight suggests candidate new locations; confirm to write back to the database and resume automatically |
| Session management | Open containing folder, copy session ID, delete session (cascading cleanup of messages, with confirmation) |
| Themes | Light / dark theme toggle with persistence |

### Installation (macOS)

**1. Prerequisites**

- Node.js 18+: `brew install node`
- Rust toolchain: `brew install rust`, or via [rustup](https://rustup.rs/)
- Xcode Command Line Tools: `xcode-select --install`

**2. Build**

```bash
git clone https://github.com/laqiumi/opencode-session-manager.git
cd opencode-session-manager
npm install
npm run tauri build
```

The first build takes about 20–30 minutes (Rust dependencies). Artifacts:

- App: `src-tauri/target/release/bundle/macos/opencode-session-manager.app`
- DMG: `src-tauri/target/release/bundle/dmg/`

**3. Install**

```bash
cp -R src-tauri/target/release/bundle/macos/opencode-session-manager.app /Applications/
xattr -dr com.apple.quarantine /Applications/opencode-session-manager.app
```

Then launch it from Launchpad.

### Usage Notes

- On the first "Resume" click, macOS may ask for permission to control the terminal — click Allow
- The `opencode` command must be on your shell PATH (new terminal windows load your shell profile)
- Terminal preference: Settings → Terminal — Terminal.app, Ghostty, or a custom command template

### Development

```bash
npm install
npm run tauri dev            # hot-reload debugging
cd src-tauri && cargo test   # backend tests
```

### Data Sources

- OpenCode: `~/.local/share/opencode/opencode.db` (override with `OPENCODE_DB_PATH`)
- Codex: `~/.codex` (override with `CODEX_DIR`)
- Claude: `~/.claude` (override with `CLAUDE_CONFIG_DIR`)
- Deleting a session operates directly on SQLite with cascading foreign-key cleanup
