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

## 安装指南（macOS，从源码构建）

### 1. 环境依赖

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

### 2. 构建

```bash
git clone https://github.com/laqiumi/opencode-session-manager.git
cd opencode-session-manager
npm install
npm run tauri build
```

首次构建约需 20~30 分钟（编译 Rust 依赖），产物在：

- App：`src-tauri/target/release/bundle/macos/opencode-session-manager.app`
- DMG：`src-tauri/target/release/bundle/dmg/`

### 3. 安装到系统

```bash
cp -R src-tauri/target/release/bundle/macos/opencode-session-manager.app /Applications/
xattr -dr com.apple.quarantine /Applications/opencode-session-manager.app
```

之后从启动台打开即可。

### 4. 使用注意

- 「▶ 继续」会在 **Terminal.app** 新开窗口执行 `opencode -s <id>`，首次点击时 macOS 会请求「允许控制 Terminal」，需点允许
- 需要 `opencode` 命令在 shell 的 PATH 中（Terminal 新窗口会加载你的 shell 配置）

## 操作指南（日常维护）

本仓库本地路径约定为 `~/opencode-session-manager`，远端为 `origin`（main 分支）。

### 提交与推送

```bash
git add <改动文件>
git commit -m "feat: xxx"   # 或 fix: 开头，不超过 20 字
git push
```

说明：

- GitHub 访问走本机代理（已配置 `http.https://github.com.proxy=http://127.0.0.1:7897`）
- 凭据用 osxkeychain 保存：首次 `git push` 提示输入时，用户名填 GitHub 账号，密码填 PAT（Settings → Developer settings → Tokens (classic)，勾 `repo`），之后免输

### 构建发布版并安装

```bash
npm run tauri build
cp -R src-tauri/target/release/bundle/macos/opencode-session-manager.app /Applications/
```

### 同步上游（原项目更新时）

```bash
git remote add upstream https://github.com/fingertipschen/opencode-session-manager.git  # 只需加一次
git fetch upstream && git merge upstream/master
```

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
