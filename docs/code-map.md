# 代码地图

这份文档的用途只有一个：在动代码之前先定位到正确的文件和符号，尽量不要靠仓库级别的盲搜。

使用规则：
- 任何代码相关的新增、删除、修改、排查，都先读这里。
- 如果本次变更影响了文件、函数、回调、结构体、组件或跨文件依赖，顺手更新这份文档。
- 这份文档是“定位导航”，不是源码替代品；需要实现细节时，再去看对应文件。

## 1. 入口与主链路

1. `src/main.rs` 初始化日志，然后直接进入 `app::run()`
2. `src/app/mod.rs::run()` 是整棵 UI 状态树的总入口，负责：
   - 读取配置
   - 创建 Slint 窗口
   - 建立 sessions / tabs / terminals 等模型
   - 启动本地系统采样
   - 绑定所有 UI 回调；大部分 glue 已拆到 `src/app/events.rs`、`src/app/sidebar.rs`、`src/app/sftp_panel.rs`、`src/app/layout.rs`、`src/app/sessions.rs`、`src/app/tabs.rs`、`src/app/transfer.rs`、`src/app/tunnels.rs` 和 `src/app/terminal_input.rs`
3. `ui/app.slint` 定义顶层窗口 `AppWindow`、全部回调名、以及 Rust 侧要喂进去的模型字段
4. `src/ssh.rs` 和 `src/sftp.rs` 分别负责 SSH 终端会话和 SFTP 子系统，两者都在 Tokio 任务里跑；可选出站代理逻辑在 `src/proxy.rs`
5. `src/system.rs` 提供本机侧资源采样，`src/i18n.rs` 负责运行时语言切换
6. `build.rs` 负责编译 Slint UI、打包翻译文件，并在 Windows 上嵌入图标
7. `src/app/state.rs` 保存少量跨组件 UI 布局状态，当前只覆盖侧边栏、底部面板显示和底部面板页签
8. `src/connection.rs` 保存每个终端 tab 的连接运行态，统一包装 SSH session 的连接、断开、重连和状态
9. `src/terminal/mod.rs` 是终端核心模块入口；`src/terminal/types.rs` / `src/terminal/engine.rs` 定义终端渲染数据、引擎模式和最小终端引擎 trait，`src/terminal/legacy.rs` 提供 legacy vt100 实现，`src/terminal/alacritty.rs` 提供可选实验 alacritty 引擎
10. `src/file_transfer.rs`、`src/app/transfer.rs` 和 `ui/transfer_window.slint` 提供独立文件传输窗口第一版；远程侧复用 `src/sftp.rs` worker
11. `src/tunnel.rs`、`src/app/tunnels.rs` 和 `ui/tunnel_panel.slint` 提供当前 session 关联的 Local Forward 隧道规则、持久化和后台转发任务

## 2. 先看哪个文件

- 改顶部工具栏、侧边栏/底部面板显隐、底部面板页签状态：先看 `src/app/layout.rs`、`src/app/state.rs`、`src/app/mod.rs`、`ui/app.slint` 和 `ui/terminal_view.slint`
- 改终端渲染数据或引擎边界：先看 `src/terminal/types.rs`、`src/terminal/engine.rs`、`src/terminal/alacritty.rs` 和 `src/terminal/legacy.rs`
- 改终端按键、鼠标上报、resize、复制/粘贴、选区生命周期：先看 `src/app/terminal_input.rs` 和 `src/app/mod.rs`，再看 `ui/terminal_view.slint`
- 改终端显示、选区、搜索、高亮、Tab 切换、回调绑定：先看 `src/app/terminal_render.rs`、`src/app/events.rs`、`src/app/models.rs`、`src/app/tabs.rs` 和 `src/app/mod.rs`，再看 `ui/app.slint` 和 `ui/terminal_view.slint`
- 改会话事件泵、终端事件到 UI 的映射、连接成功后自动启动隧道：先看 `src/app/events.rs`、`src/app/tunnels.rs` 和 `src/ssh.rs`
- 改会话列表模型、会话对话框、导入 `~/.ssh/config`、连接入口：先看 `src/app/sessions.rs`、`src/config.rs` 和 `ui/app.slint`
- 改 tab 关闭/新建、断开/重连当前 tab：先看 `src/app/tabs.rs`、`src/app/mod.rs` 和 `ui/app.slint`
- 改 SSH 连接运行态、断开、重连、连接状态入口：先看 `src/connection.rs`、`src/app/tabs.rs`、`src/app/mod.rs` 和 `src/ssh.rs`
- 改 SSH 认证、远端监控、OSC7 路径解析、出站代理：先看 `src/ssh.rs` 和 `src/proxy.rs`
- 改 SFTP 面板、树形目录、下载 / 上传 / 删除 / 打开文件、拖拽上传：先看 `src/app/sftp_panel.rs`、`src/sftp.rs` 和 `src/proxy.rs`，再看 `ui/sftp_panel.slint`
- 改独立文件传输窗口、本地目录浏览、双栏上传/下载：先看 `src/file_transfer.rs`、`src/app/transfer.rs`，再看 `ui/transfer_window.slint`、`ui/local_file_panel.slint`、`ui/remote_file_panel.slint`
- 改隧道 Local Forward、规则持久化、自动启停、端口占用状态：先看 `src/tunnel.rs`、`src/app/tunnels.rs`，再看 `ui/tunnel_panel.slint`
- 改 app 内部状态别名、TabStatus、TransferWindowState、NetHistory：先看 `src/app/types.rs`
- 改窗口居中和鼠标位置：先看 `src/app/platform.rs`
- 改会话持久化、密码字段、代理字段、下载目录、语言配置：先看 `src/app/sessions.rs`、`src/config.rs`
- 改本机 CPU / 内存 / 网络 / 磁盘侧边栏：先看 `src/app/sidebar.rs`、`src/system.rs` 和 `ui/sidebar.slint`
- 改语言、翻译、`@tr(...)` 文案：先看 `src/i18n.rs`、`build.rs`、`lang/*`、`ui/*.slint`
- 改导入 `~/.ssh/config`：先看 `src/ssh_config.rs`
- 改依赖、feature、构建脚本、打包行为、GitHub Release workflow：先看 `Cargo.toml`、`build.rs` 和 `.github/workflows/release.yml`

## 3. Rust 源码地图

### `src/app/mod.rs`
职责：
- 顶层 UI 状态机和 glue code
- 初始化 `src/app/state.rs` 里的 `AppState`，并把默认布局状态同步到 Slint 窗口属性
- 通过 `ConnectionManager` 管理每个终端 tab 的 SSH runtime
- 持有当前终端 wrapper，默认走 legacy vt100，引擎模式为 `MEATSHELL_TERMINAL_ENGINE=alacritty` 时委托到实验 alacritty 引擎
- 维护 tabs / terminals / SFTP 状态
- 把 Slint 回调路由到 SSH / SFTP / 配置 / 系统采样模块
- 基础状态布局、平台 helper、layout / events / sidebar / session / tab / sftp panel / transfer / tunnel / terminal-input / terminal-render / model 的 UI glue 分别拆到 `src/app/state.rs`、`src/app/types.rs`、`src/app/platform.rs`、`src/app/layout.rs`、`src/app/events.rs`、`src/app/sidebar.rs`、`src/app/sessions.rs`、`src/app/tabs.rs`、`src/app/sftp_panel.rs`、`src/app/transfer.rs`、`src/app/tunnels.rs`、`src/app/terminal_input.rs`、`src/app/terminal_render.rs` 和 `src/app/models.rs`；legacy vt100 引擎核心放在 `src/terminal/legacy.rs`

关键符号：
- `run()`
定位提示：
- 任何 callback 签名变动，通常都要同时改这里和 `ui/app.slint`
- 终端显示问题，优先查 `src/terminal/legacy.rs` 和 `src/app/events.rs`
- 选区 / 搜索问题，优先查 `src/app/terminal_render.rs`，再看 `compute_find_matches(...)`、`selection_rects(...)`、`extract_selection(...)`

### `src/terminal/mod.rs`
职责：
- 终端核心子模块入口
- 统一导出 `alacritty`、`engine`、`legacy` 和 `types`，供 app 层按 `crate::terminal::*` 引用

关键符号：
- `mod alacritty`
- `mod engine`
- `mod legacy`
- `mod types`

### `src/terminal/legacy.rs`
职责：
- 保存 legacy vt100 终端引擎实现，以及它专用的行构建、滚动检测和颜色映射 helper
- 只负责本地终端渲染和历史缓冲，不接触 SSH / SFTP / UI wiring

关键符号：
- `LegacyTerminalEngine`
- `CsiState`
- `MAX_HISTORY`
- `ANSI16`
- `cell_attrs(...)`
- `build_row(...)`
- `detect_scroll(...)`
- `impl LegacyTerminalEngine`
- `impl TerminalEngine for LegacyTerminalEngine`
- `vt_color_to_slint(...)`
- `vt_bg_to_slint(...)`
- `idx_to_rgb(...)`

### `src/app/models.rs`
职责：
- 保存终端显示用的 Slint model 适配器
- 负责把 Rust 侧的渲染结果和终端状态写回 `TerminalState`，避免这类轻量转换继续留在 `src/app/mod.rs`

关键符号：
- `term_spans_model(...)`
- `set_terminal_row(...)`

### `src/app/terminal_render.rs`
职责：
- 保存终端显示的搜索、选区和重建逻辑
- 负责从 `TermBuffers` 重新计算当前 tab 的 spans、cursor、find 高亮和 selection 高亮，再交给 `src/app/models.rs`

关键符号：
- `compute_find_matches(...)`
- `norm_sel(...)`
- `selection_rects(...)`
- `extract_selection(...)`
- `rebuild_tab_display(...)`

### `src/app/terminal_input.rs`
职责：
- 保存终端按键、鼠标上报、resize、剪贴板和选区交互的 UI glue
- 只负责把 Slint 事件转成 PTY 字节、终端窗口尺寸、剪贴板操作和重新绘制调用，不触碰 SSH / SFTP / tunnel / render 引擎本体

关键符号：
- `wire_key_input(...)`
- `key_to_pty_bytes(...)`
- `sgr_mouse_sequence(...)`
- `is_vk_back_down(...)`
- `c0_letter_key_down(...)`

### `src/app/layout.rs`
职责：
- 保存窗口布局状态和底部面板的 UI glue
- 把 `src/app/state.rs` 里的 `AppState` 同步到 Slint 窗口，并绑定侧边栏 / 底部面板的切换回调

关键符号：
- `sync_app_state_to_window(...)`
- `wire_layout_callbacks(...)`

### `src/app/events.rs`
职责：
- 保存 session / SFTP 事件泵，以及把 `SessionEvent` 映射到 Slint UI 模型的 glue
- 连接成功时继续触发隧道自动启动，连接断开时继续停止对应隧道

关键符号：
- `spawn_shell_event_pump(...)`
- `spawn_sftp_event_pump(...)`
- `apply_session_event_to_window(...)`

### `src/app/sidebar.rs`
职责：
- 保存侧边栏和底部网络图的计算逻辑
- 负责把系统采样、tab 状态和网络历史转成 Slint model / 属性

关键符号：
- `push_ring(...)`
- `normalized_model(...)`
- `disk_model(...)`
- `selected_iface(...)`
- `refresh_sidebar(...)`

### `src/app/sessions.rs`
职责：
- 保存会话列表模型同步和会话对话框 / 连接入口的 UI glue
- 负责把 `ConfigStore`、会话模型和连接启动逻辑接到 Slint 回调

关键符号：
- `sync_sessions_to_model(...)`
- `wire_session_callbacks(...)`

### `src/app/tabs.rs`
职责：
- 保存 tab 列表和当前连接工具栏的 UI glue
- 负责 tab 关闭 / 新建，以及断开 / 重连当前 tab 的回调接线

关键符号：
- `wire_connection_toolbar_callbacks(...)`
- `wire_tab_callbacks(...)`

### `src/app/sftp_panel.rs`
职责：
- 保存 SFTP 面板的 UI glue，以及拖拽上传的命中检测
- 负责把 SFTP 面板回调和当前 tab 的远程路径定位接到 Slint

关键符号：
- `wire_sftp_callbacks(...)`
- `active_sftp_path(...)`
- `handle_file_drop(...)`
- `parent_path(...)`

### `src/app/transfer.rs`
职责：
- 保存独立文件传输窗口的 UI glue
- 负责把 `src/file_transfer.rs` 的本地目录 helper 和 `src/sftp.rs` 的 worker 接到 `ui/transfer_window.slint`

关键符号：
- `open_transfer_window(...)`
- `wire_transfer_window_callbacks(...)`
- `refresh_transfer_local(...)`
- `local_entries_model(...)`
- `remote_entries_model(...)`
- `spawn_transfer_sftp_event_pump(...)`
- `apply_transfer_event_to_window(...)`

### `src/app/tunnels.rs`
职责：
- 保存隧道面板的 UI glue
- 负责把 `src/tunnel.rs` 的规则管理和事件泵接到 `ui/tunnel_panel.slint`

关键符号：
- `spawn_tunnel_event_pump(...)`
- `wire_tunnel_callbacks(...)`
- `refresh_tunnel_panel(...)`
- `tunnel_view_to_info(...)`

### `src/app/types.rs`
职责：
- 保存 app 作用域内的状态别名和轻量结构体，供 `src/app/mod.rs` 和后续 app 子模块复用
- 现在只放连接 / SFTP / 隧道 / 终端缓冲区相关的别名、tab 状态、传输窗口状态和网卡历史长度常量

关键符号：
- `TermBuffers`
- `SftpHandles`
- `ConnectionStore`
- `TunnelStore`
- `TermBuffer`
- `SftpManualNav`
- `TabStatus`
- `TabStatuses`
- `LocalSnap`
- `TransferWindowState`
- `TransferWindows`
- `NetHist`
- `NET_HISTORY_LEN`

### `src/app/platform.rs`
职责：
- 放置少量平台相关 helper，当前只承载窗口居中和鼠标位置查询
- 这些函数只为 `src/app/mod.rs` 提供本地调用点，不扩展成通用平台层

关键符号：
- `center_window(...)`
- `cursor_pos(...)`

### `src/app/state.rs`
职责：
- 保存少量全局 UI 布局状态，避免继续把阶段 2 的工具栏/底部面板状态直接散落在 `src/app/mod.rs`
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

### `src/file_transfer.rs`
职责：
- 独立文件传输窗口的本地文件系统 helper
- 用 `std::fs::read_dir` 列本地目录，并排序为目录优先
- 解析本地上级目录和默认本地目录；远程传输仍由 `src/sftp.rs` 负责

关键符号：
- `LocalFileEntry`
- `default_local_dir(...)`
- `resolve_local_path(...)`
- `list_local_dir(...)`

### `src/tunnel.rs`
职责：
- 保存和读取独立的 `tunnels.json`，不把隧道规则塞进 `Session`
- 定义当前 session 关联的 Local Forward 规则、运行状态和后台任务句柄
- 为 enabled 规则启动独立 SSH 连接，通过 `direct-tcpip` 转发本地 TCP 流量
- 提供 stop/cancel 入口，session 断开、重连或 tab 关闭时释放本地监听端口
- 通过 `TunnelEvent` 把 Starting / Running / Reconnecting / Failed / Stopped 状态推回 UI

关键符号：
- `TunnelRule`
- `TunnelStatus`
- `TunnelView`
- `TunnelEvent`
- `TunnelHandle`
- `TunnelManager`
- `run_tunnel(...)`
- `connect_and_serve(...)`
- `connect_ssh(...)`
- `authenticate(...)`

定位提示：
- 端口占用、认证失败、自动重连/backoff、停止后端口释放问题，先看这里
- UI 规则保存/启用/删除入口在 `src/app/mod.rs::wire_tunnel_callbacks(...)`
- session 连接成功后自动启动和断开后停止，在 `src/app/mod.rs::spawn_shell_event_pump(...)`

### `src/terminal/types.rs`
职责：
- 保存终端渲染数据类型，避免把纯渲染模型继续定义在 `src/app/mod.rs`
- `BuiltScreen` 是泛型快照，除渲染 run 和 cursor 外也携带当前是否启用 SGR mouse reporting
- `RenderSpan` 是 Rust 侧中立渲染 run，`src/terminal/legacy.rs`、`src/terminal/alacritty.rs` 和 `src/app/models.rs` 共同使用它再转换成 Slint 的 `TermSpan`

关键符号：
- `BuiltScreen`
- `RenderSpan`
- `HistSpan`
- `Line`

### `src/terminal/engine.rs`
职责：
- 定义最小 `TerminalEngine` trait
- 定义 `TerminalEngineMode`，当前通过启动前环境变量 `MEATSHELL_TERMINAL_ENGINE=alacritty` 选择实验引擎，否则默认 legacy
- trait 由 `src/terminal/legacy.rs` 中的 `LegacyTerminalEngine` wrapper 和 `src/terminal/alacritty.rs` 中的实验引擎实现

关键符号：
- `TerminalEngineMode`
- `TerminalEngine`

### `src/terminal/alacritty.rs`
职责：
- 包装 `alacritty_terminal`，实现实验终端引擎
- 接收 SSH 输出 bytes，交给 alacritty parser 更新终端状态
- 把 alacritty grid/cell 转换为 `BuiltScreen<RenderSpan>`，不把 alacritty 内部类型泄漏到 `app/mod.rs` 或 Slint
- 支持基础 resize，并从 alacritty `TermMode` 暴露 SGR mouse reporting 状态给 UI

关键符号：
- `AlacrittyTerminalEngine`
- `AlacrittyDimensions`

定位提示：
- 实验引擎显示、颜色、宽字符或 resize 问题，先看这里
- app 侧切换逻辑只看 `src/terminal/legacy.rs::LegacyTerminalEngine::new(...)` 和 `TerminalEngine` impl

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
- 暴露当前 active session 的 `tunnel-rules` 模型和隧道规则增删改/启用回调给 Rust

关键符号：
- `AppWindow`
- `TransferInfo`
- `TunnelRuleInfo`
- `TransferWindow`
- `TerminalState`
- `toggle-sidebar`
- `toggle-bottom-panel`
- `select-bottom-panel-tab`
- `disconnect-active-tab`
- `reconnect-active-tab`
- `open-transfer-window`
- `tunnel-add-rule`
- `tunnel-update-rule`
- `tunnel-toggle-rule`
- `tunnel-delete-rule`
- `terminal-mouse`
- `dialog-proxy`
- 导出类型：`SessionInfo`、`SessionDraft`、`TabInfo`、`SftpEntry`、`SftpTreeNode`、`TunnelRuleInfo`

定位提示：
- Rust 回调名、属性名、模型字段改动时，先改这里
- 会话弹窗字段（例如 `dialog-proxy`）改动时，先对照 `ui/session_dialog.slint` 和 `src/app/mod.rs`
- `src/app/mod.rs` 的 wiring 代码和这里必须一一对应

### `ui/terminal_view.slint`
职责：
- 终端格子渲染
- 隐藏 IME 输入
- 搜索高亮、拖拽选区、右键菜单、滚轮滚动
- 当终端引擎报告 SGR mouse mode 已开启时，把左键按下/释放和滚轮转发给 Rust，避免普通文本选择退化
- 底部 `BottomPanel` 承载
- 根据 `bottom-panel-visible` / `bottom-panel-tab` 决定当前底部文件面板是否显示
- 把 `tunnel-rules` 和隧道规则回调继续传给底部 `TunnelPanel`

关键符号：
- `TermSpan`
- `TermMatch`
- `MenuItem`
- `TerminalView`
- `terminal-mouse`

定位提示：
- 终端交互问题，先看这里，再看 `src/app/mod.rs` 的键盘和渲染代码

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
- `Tunnels` 页承载 `TunnelPanel` 规则管理面板

关键符号：
- `BottomPanel`
- `PanelTab`

### `ui/tunnel_panel.slint`
职责：
- 显示当前 session 的 Local Forward 规则列表
- 支持新增规则、保存本地/远端地址端口、启用/禁用、删除规则
- 显示 Stopped / Starting / Running / Reconnecting / Failed 状态

关键符号：
- `TunnelPanel`
- `TunnelRuleInfo`

### `ui/transfer_window.slint`
职责：
- 独立文件传输窗口外壳
- 左侧承载 `LocalFilePanel`，右侧承载 `RemoteFilePanel`
- 暴露本地/远程导航、刷新、上传、下载、关闭回调给 `src/app/mod.rs`

关键符号：
- `TransferWindow`

### `ui/local_file_panel.slint`
职责：
- 文件传输窗口左侧本机目录列表
- 支持进入目录、返回上级、刷新，以及上传本地文件到当前远程目录

关键符号：
- `LocalFilePanel`

### `ui/remote_file_panel.slint`
职责：
- 文件传输窗口右侧远程目录列表
- 支持进入目录、返回上级、刷新，以及下载远程文件到当前本地目录

关键符号：
- `RemoteFilePanel`

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
- `tunnels.json`：运行时生成的用户配置文件，保存 Local Forward 隧道规则（位于系统配置目录，不在仓库内）
- `lang/zh/LC_MESSAGES/meatshell.po` 和 `lang/en/LC_MESSAGES/meatshell.po`：翻译资源
- `.github/workflows/release.yml`：打 tag / 手动发布的构建与上传 workflow
- `assets/meatshell.ico`：Windows 程序图标
- `assets/meatshell.desktop`：Linux 桌面文件
- `assets/install-linux.sh`、`assets/make_icon.py`：打包 / 资源辅助脚本
- `docs/screenshots/*`：截图参考，不是运行时逻辑

## 6. 常见定位路径

- SSH 连接 / 认证 / 代理：`src/ssh.rs` -> `src/proxy.rs` -> `src/config.rs`
- SFTP 列表 / 下载 / 上传 / 删除 / 代理：`src/sftp.rs` -> `src/proxy.rs` -> `ui/sftp_panel.slint`
- 独立文件传输窗口：`src/app/mod.rs` -> `src/file_transfer.rs` / `src/sftp.rs` -> `ui/transfer_window.slint`
- 隧道 Local Forward：`src/app/mod.rs` -> `src/tunnel.rs` -> `ui/tunnel_panel.slint`
- 终端显示 / 搜索 / 选区：`src/app/mod.rs` -> `ui/terminal_view.slint`
- 侧边栏资源数据：`src/system.rs`、`src/ssh.rs` -> `ui/sidebar.slint`
- 会话导入 / 编辑：`src/ssh_config.rs`、`src/config.rs` -> `ui/session_dialog.slint`、`ui/welcome.slint`
- 翻译 / 语言切换：`src/i18n.rs`、`build.rs`、`lang/*`、`ui/*.slint`

## 7. 维护规则

- 只要变更涉及文件、函数、回调、结构体、组件或跨文件依赖，就要同步更新这份地图
- 每次动代码前，先读这份地图，再开始查找或修改
