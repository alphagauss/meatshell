# 代码地图

这份文档的用途只有一个：在动代码之前先定位到正确的文件和符号，尽量不要靠仓库级别的盲搜。

使用规则：
- 任何代码相关的新增、删除、修改、排查，都先读这里。
- 如果本次变更影响了文件、函数、回调、结构体、组件或跨文件依赖，顺手更新这份文档。
- 这份文档是“定位导航”，不是源码替代品；需要实现细节时，再去看对应文件。

## 1. 入口与主链路

1. `src/main.rs` 初始化日志，然后直接进入 `app::run()`
2. `src/app.rs::run()` 是整棵 UI 状态树的总入口，负责：
   - 读取配置
   - 创建 Slint 窗口
   - 建立 sessions / tabs / terminals 等模型
   - 启动本地系统采样
   - 绑定所有 UI 回调
3. `ui/app.slint` 定义顶层窗口 `AppWindow`、全部回调名、以及 Rust 侧要喂进去的模型字段
4. `src/ssh.rs` 和 `src/sftp.rs` 分别负责 SSH 终端会话和 SFTP 子系统，两者都在 Tokio 任务里跑；可选出站代理逻辑在 `src/proxy.rs`
5. `src/system.rs` 提供本机侧资源采样，`src/i18n.rs` 负责运行时语言切换
6. `build.rs` 负责编译 Slint UI、打包翻译文件，并在 Windows 上嵌入图标
7. `src/app_state.rs` 保存少量跨组件 UI 布局状态，当前只覆盖侧边栏、底部面板显示和底部面板页签
8. `src/connection.rs` 保存每个终端 tab 的连接运行态，统一包装 SSH session 的连接、断开、重连和状态
9. `src/terminal_types.rs` / `src/terminal_engine.rs` 定义终端渲染数据和最小终端引擎 trait，当前由 legacy vt100 引擎实现

## 2. 先看哪个文件

- 改顶部工具栏、侧边栏/底部面板显隐、底部面板页签状态：先看 `src/app_state.rs`、`src/app.rs`、`ui/app.slint` 和 `ui/terminal_view.slint`
- 改终端渲染数据或引擎边界：先看 `src/terminal_types.rs`、`src/terminal_engine.rs`，再看 `src/app.rs` 的 `LegacyTerminalEngine`
- 改终端显示、选区、搜索、高亮、拖拽上传、Tab 切换、回调绑定：先看 `src/app.rs`，再看 `ui/app.slint` 和 `ui/terminal_view.slint`；终端缩窗后的回滚保留也在 `src/app.rs` 的 `wire_key_input(...)`
- 改 SSH 连接运行态、断开、重连、连接状态入口：先看 `src/connection.rs`，再看 `src/app.rs` 和 `src/ssh.rs`
- 改 SSH 认证、远端监控、OSC7 路径解析、出站代理：先看 `src/ssh.rs` 和 `src/proxy.rs`
- 改 SFTP 列表、树形目录、下载 / 上传 / 删除 / 打开文件、出站代理：先看 `src/sftp.rs` 和 `src/proxy.rs`，再看 `ui/sftp_panel.slint`
- 改会话持久化、密码字段、代理字段、下载目录、语言配置：先看 `src/config.rs`
- 改本机 CPU / 内存 / 网络 / 磁盘侧边栏：先看 `src/system.rs` 和 `ui/sidebar.slint`
- 改语言、翻译、`@tr(...)` 文案：先看 `src/i18n.rs`、`build.rs`、`lang/*`、`ui/*.slint`
- 改导入 `~/.ssh/config`：先看 `src/ssh_config.rs`
- 改依赖、feature、构建脚本、打包行为、GitHub Release workflow：先看 `Cargo.toml`、`build.rs` 和 `.github/workflows/release.yml`

## 3. Rust 源码地图

### `src/app.rs`
职责：
- 顶层 UI 状态机和 glue code
- 初始化 `AppState`，并把默认布局状态同步到 Slint 窗口属性
- 通过 `ConnectionManager` 管理每个终端 tab 的 SSH runtime
- 持有当前 legacy vt100 终端引擎，并通过 `TerminalEngine` trait 调用 ingest/render/resize
- 维护 tabs / terminals / SFTP 状态
- 处理终端渲染、搜索、选区、拖拽、侧边栏刷新
- 把 Slint 回调路由到 SSH / SFTP / 配置 / 系统采样模块

关键符号：
- `run()`
- `sync_app_state_to_window(...)`
- `wire_app_state_callbacks(...)`
- `spawn_shell_event_pump(...)`
- `spawn_sftp_event_pump(...)`
- `wire_session_callbacks(...)`
- `wire_tab_callbacks(...)`
- `wire_sftp_callbacks(...)`
- `wire_key_input(...)`
- `apply_session_event_to_window(...)`
- `refresh_sidebar(...)`
- `rebuild_tab_display(...)`
- `sync_sessions_to_model(...)`
- `set_terminal_row(...)`
- `key_to_pty_bytes(...)`
- `handle_file_drop(...)`
- `active_sftp_path(...)`
- `center_window(...)`
- `push_ring(...)`
- `normalized_model(...)`
- `disk_model(...)`
- `compute_find_matches(...)`
- `norm_sel(...)`
- `selection_rects(...)`
- `extract_selection(...)`
- `selected_iface(...)`
- `vt_color_to_slint(...)`
- `vt_bg_to_slint(...)`
- `idx_to_rgb(...)`
- `parent_path(...)`
- `LegacyTerminalEngine`
- `TermBuffer`（legacy 引擎别名，降低阶段 4 改动面）
- `CsiState`
- `TabStatus`
- `TermBuffers`
- `SftpHandles`
- `SftpManualNav`
- `TabStatuses`
- `LocalSnap`
- `NetHist`

`LegacyTerminalEngine` 里最重要的内部逻辑：
- `ingest(...)`
- `rewrite_hvp(...)`
- `ingest_chunk(...)`
- `render(...)`

定位提示：
- 任何 callback 签名变动，通常都要同时改这里和 `ui/app.slint`
- 终端显示问题，优先查 `LegacyTerminalEngine` 和 `apply_session_event_to_window(...)`
- 选区 / 搜索问题，优先查 `compute_find_matches(...)`、`selection_rects(...)`、`extract_selection(...)`

### `src/app_state.rs`
职责：
- 保存少量全局 UI 布局状态，避免继续把阶段 2 的工具栏/底部面板状态直接散落在 `src/app.rs`
- 当前只包含 `sidebar_visible`、`bottom_panel_visible`、`bottom_panel_tab`，以及对应的 toggle / tab select 方法

关键符号：
- `AppState`
- `BottomPanelTab`

### `src/connection.rs`
职责：
- 统一管理每个终端 tab 的 SSH 连接运行态
- 包装现有 `ssh::spawn_session(...)` 和 `SessionHandle`
- 提供连接、断开、重连、PTY 输入/resize 转发和状态更新入口
- 用 generation 过滤重连后旧 worker 迟到事件

关键符号：
- `ConnectionStatus`
- `SessionRuntime`
- `SessionLaunch`
- `ConnectionManager`

### `src/terminal_types.rs`
职责：
- 保存终端渲染数据类型，避免把纯渲染模型继续定义在 `src/app.rs`
- `BuiltScreen` 是泛型快照，`app.rs` 当前使用 `BuiltScreen<TermSpan>` 适配 Slint

关键符号：
- `BuiltScreen`
- `HistSpan`
- `Line`

### `src/terminal_engine.rs`
职责：
- 定义最小 `TerminalEngine` trait
- 当前 trait 由 `src/app.rs` 中的 `LegacyTerminalEngine` 实现，后续 alacritty 实验引擎复用这个边界

关键符号：
- `TerminalEngine`

### `src/ssh.rs`
职责：
- 单个 SSH shell 会话的生命周期管理
- PTY 创建、认证、收发 shell 数据
- 远端资源监控采样
- 解析 OSC7 当前目录信息，并在需要时经 `src/proxy.rs` 建连

关键符号：
- `RemoteEntry`
- `RemoteTreeNode`
- `SessionCommand`
- `SessionEvent`
- `SessionHandle`
- `format_size(...)`
- `format_mtime(...)`
- `extract_osc7_path(...)`
- `spawn_session(...)`
- `run_session(...)`
- `parse_monitor_block(...)`
- `parse_df_line(...)`
- `parse_meminfo_kib(...)`
- `parse_net_dev_line(...)`
- `ClientHandler`

定位提示：
- SSH 认证和连接失败，先看 `spawn_session(...)` / `run_session(...)` / `src/proxy.rs`
- 远端状态栏、CPU / 内存 / 网络 / 磁盘数据，先看 `parse_monitor_block(...)`
- 终端当前目录同步，先看 `extract_osc7_path(...)`

### `src/proxy.rs`
职责：
- SSH / SFTP 出站代理支持
- 解析会话代理配置和 `ALL_PROXY` / `all_proxy`
- 把 TCP 连接透过 SOCKS5 或 HTTP CONNECT 建成透明隧道

关键符号：
- `ProxyConfig`
- `ProxyKind`
- `resolve(...)`
- `describe(...)`
- `connect(...)`
- `parse(...)`
- `connect_socks5(...)`
- `connect_http(...)`

定位提示：
- 代理 URL 解析、scheme 判定、认证参数解析先看 `resolve(...)` / `parse(...)`
- SOCKS5 和 HTTP CONNECT 的具体握手分别看 `connect_socks5(...)` / `connect_http(...)`

### `src/sftp.rs`
职责：
- 独立的 SFTP 工作线程
- 远端目录树、目录列表、下载、上传、删除、临时打开、编辑后自动重传
- 可选沿用 SSH 会话的同一套出站代理

关键符号：
- `SftpCommand`
- `SftpHandle`
- `spawn_sftp(...)`
- `run_sftp(...)`
- `build_tree_nodes(...)`
- `list_dir_impl(...)`
- `list_dirs_only_impl(...)`
- `download_impl(...)`
- `upload_pipelined(...)`
- `spawn_edit_watcher(...)`
- `emit_transfer(...)`
- `base_name(...)`
- `parent_dir(...)`
- `sanitize_filename(...)`
- `open_with_os(...)`
- `SftpClientHandler`

定位提示：
- SFTP 列表和树不一致，先看 `build_tree_nodes(...)` 和 `list_dir_impl(...)`
- SFTP 连接和代理问题，先看 `run_sftp(...)` / `src/proxy.rs`
- 下载 / 上传进度条问题，先看 `emit_transfer(...)`、`download_impl(...)`、`upload_pipelined(...)`
- “查看 / 编辑” 的临时文件安全问题，先看 `sanitize_filename(...)` 和 `open_with_os(...)`

### `src/config.rs`
职责：
- 会话配置落盘与读取
- 凭据包装
- 下载目录、UI 语言，以及每个 Session 的可选出站代理持久化

关键符号：
- `Secret`
- `AuthMethod`
- `Session`
- `ConfigFile`
- `ConfigStore`
- `ConfigStore::load(...)`
- `ConfigStore::save(...)`
- `ConfigStore::upsert(...)`
- `ConfigStore::remove(...)`
- `ConfigStore::get(...)`
- `ConfigStore::download_dir(...)`
- `ConfigStore::set_download_dir(...)`
- `ConfigStore::language(...)`
- `ConfigStore::set_language(...)`

定位提示：
- 任何 session 字段新增 / 删除，先看这里，再看 `ui/session_dialog.slint`、`ui/welcome.slint` 和 `src/proxy.rs`
- `Secret` 的 zeroize 行为不要轻易改

### `src/system.rs`
职责：
- 本地机器资源采样
- 侧边栏底部那组本机网络 / 磁盘数据的来源

关键符号：
- `SystemSnapshot`
- `SystemSampler`
- `SystemSampler::new(...)`
- `SystemSampler::recommended_interval(...)`
- `SystemSampler::sample(...)`
- `format_bytes_per_sec(...)`

### `src/ssh_config.rs`
职责：
- 从 `~/.ssh/config` 导入主机条目

关键符号：
- `ImportedHost`
- `parse_default(...)`
- `parse_str(...)`
- `home_dir(...)`
- `split_kv(...)`
- `expand_tilde(...)`
- `is_concrete(...)`
- `parses_basic_blocks`（测试）

定位提示：
- 导入行为变化时，优先补这个文件里的测试

### `src/i18n.rs`
职责：
- 运行时语言选择
- Rust 动态字符串和 Slint 静态翻译同步

关键符号：
- `set_language(...)`
- `apply_to_slint(...)`
- `current_code(...)`
- `is_en(...)`
- `t(...)`

### `src/main.rs`
职责：
- 程序入口

关键符号：
- `main()`

### `build.rs`
职责：
- 编译 `ui/app.slint`
- 打包 `lang/` 下的翻译
- Windows 图标资源嵌入

关键符号：
- `main()`

## 4. Slint UI 地图

### `ui/app.slint`
职责：
- 顶层窗口 `AppWindow`
- 定义 Rust 侧需要的全部回调和模型字段
- 组装左侧栏、Tab 栏、顶部工具栏、欢迎页、终端页、底部面板、会话对话框
- 暴露 `sidebar-visible`、`bottom-panel-visible`、`bottom-panel-tab` 布局状态给 Rust 侧 `AppState`

关键符号：
- `AppWindow`
- `TransferInfo`
- `TerminalState`
- `toggle-sidebar`
- `toggle-bottom-panel`
- `select-bottom-panel-tab`
- `disconnect-active-tab`
- `reconnect-active-tab`
- `open-transfer-window`
- `dialog-proxy`
- 导出类型：`SessionInfo`、`SessionDraft`、`TabInfo`、`SftpEntry`、`SftpTreeNode`

定位提示：
- Rust 回调名、属性名、模型字段改动时，先改这里
- 会话弹窗字段（例如 `dialog-proxy`）改动时，先对照 `ui/session_dialog.slint` 和 `src/app.rs`
- `src/app.rs` 的 wiring 代码和这里必须一一对应

### `ui/terminal_view.slint`
职责：
- 终端格子渲染
- 隐藏 IME 输入
- 搜索高亮、拖拽选区、右键菜单、滚轮滚动
- 底部 `BottomPanel` 承载
- 根据 `bottom-panel-visible` / `bottom-panel-tab` 决定当前底部文件面板是否显示

关键符号：
- `TermSpan`
- `TermMatch`
- `MenuItem`
- `TerminalView`

定位提示：
- 终端交互问题，先看这里，再看 `src/app.rs` 的键盘和渲染代码

### `ui/top_action_bar.slint`
职责：
- 标签页下方的固定工具栏
- 提供侧边栏显隐、底部面板显隐、断开、重连、新建文件传输按钮

关键符号：
- `TopActionBar`

### `ui/bottom_panel.slint`
职责：
- 底部“文件 / 隧道”页签外壳
- `Files` 页继续承载现有 `SftpPanel`
- `Tunnels` 页承载 `TunnelPanel` 空状态

关键符号：
- `BottomPanel`
- `PanelTab`

### `ui/tunnel_panel.slint`
职责：
- 隧道页签第一版空状态，真实规则管理在后续阶段实现

关键符号：
- `TunnelPanel`

### `ui/sidebar.slint`
职责：
- 左侧状态栏
- CPU / 内存 / Swap
- 双网络图
- 磁盘列表

关键符号：
- `DiskInfo`
- `StatRow`
- `NetGraph`
- `Sidebar`

### `ui/sftp_panel.slint`
职责：
- 远端目录树
- 文件列表
- 右键菜单

关键符号：
- `SftpEntry`
- `SftpTreeNode`
- `SftpMenuItem`
- `SftpPanel`

### `ui/tabs.slint`
职责：
- Tab 栏与新建 / 关闭按钮

关键符号：
- `TabInfo`
- `SingleTab`
- `TabBar`

### `ui/welcome.slint`
职责：
- 欢迎页
- 快速连接
- 会话列表

关键符号：
- `SessionInfo`
- `SessionRow`
- `Welcome`

### `ui/session_dialog.slint`
职责：
- 新建 / 编辑 SSH 会话弹窗，包含可选出站代理输入

关键符号：
- `SessionDraft`
- `draft-proxy`
- `SessionDialog`

### `ui/widgets.slint`
职责：
- 通用 UI 组件

关键符号：
- `IconButton`
- `PrimaryButton`
- `GhostButton`
- `LabeledInput`
- `Sparkline`

### `ui/theme.slint`
职责：
- 颜色、字号、圆角、间距等设计 token；`text-secondary` / `text-muted` 控制弱文字对比度

关键符号：
- `Theme`
- `text-secondary`
- `text-muted`

## 5. 资源与数据目录

- `plan.md`：阶段执行计划和完成状态；每完成一个阶段后更新对应勾选项
- `lang/zh/LC_MESSAGES/meatshell.po` 和 `lang/en/LC_MESSAGES/meatshell.po`：翻译资源
- `.github/workflows/release.yml`：打 tag / 手动发布的构建与上传 workflow
- `assets/meatshell.ico`：Windows 程序图标
- `assets/meatshell.desktop`：Linux 桌面文件
- `assets/install-linux.sh`、`assets/make_icon.py`：打包 / 资源辅助脚本
- `docs/screenshots/*`：截图参考，不是运行时逻辑

## 6. 常见定位路径

- SSH 连接 / 认证 / 代理：`src/ssh.rs` -> `src/proxy.rs` -> `src/config.rs`
- SFTP 列表 / 下载 / 上传 / 删除 / 代理：`src/sftp.rs` -> `src/proxy.rs` -> `ui/sftp_panel.slint`
- 终端显示 / 搜索 / 选区：`src/app.rs` -> `ui/terminal_view.slint`
- 侧边栏资源数据：`src/system.rs`、`src/ssh.rs` -> `ui/sidebar.slint`
- 会话导入 / 编辑：`src/ssh_config.rs`、`src/config.rs` -> `ui/session_dialog.slint`、`ui/welcome.slint`
- 翻译 / 语言切换：`src/i18n.rs`、`build.rs`、`lang/*`、`ui/*.slint`

## 7. 维护规则

- 只要变更涉及文件、函数、回调、结构体、组件或跨文件依赖，就要同步更新这份地图
- 每次动代码前，先读这份地图，再开始查找或修改
