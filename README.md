# OpenCode 会话管理器

跨文件夹管理所有 OpenCode 会话的桌面应用。Tauri 2 + React 19 + TypeScript + rusqlite。

## 功能

- 读取全局 `opencode.db`，展示**所有文件夹**的会话（标题、文件夹、创建时间、最后运行时间、消息数、Token 消耗）
- 左侧按文件夹分组统计，可筛选
- 顶部搜索框：按标题 / 路径 / 会话 ID 过滤
- 操作：
  - **▶ 继续**：在新终端窗口以 `opencode -s <id>` 续开会话
  - **📁 文件夹**：打开会话所在文件夹
  - **⧉ ID**：复制会话 ID
  - **🗑 删除**：从数据库删除会话（含关联消息，不可恢复，有确认）

## 开发

```bash
npm install
npm run tauri dev
```

## 测试

```bash
cd src-tauri && cargo test
```

## 数据来源

- 数据库路径：`~/.local/share/opencode/opencode.db`（可用环境变量 `OPENCODE_DB_PATH` 覆盖）
- 删除直接操作 SQLite，外键级联清理关联消息
