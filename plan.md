# 终端管理器重构与功能开发计划

## 0. 背景与目标

本计划面向当前 Rust + Slint 终端管理器项目。项目后续将从 fork 维护转为独立开发维护，不再以兼容上游为主要目标，但仍要求保持重构克制，避免为了“架构漂亮”引入过度抽象。

目标是把项目逐步演进为一个更接近 Xshell + Xftp 使用体验的自用终端管理器，重点新增能力包括：

1. 完整 VT/ANSI 终端模拟，后续接入 `alacritty_terminal`，支持主流 TUI 应用、PowerShell、鼠标交互等。
2. 终端标签页上方新增工具栏按钮：侧边栏展开/折叠、下方文件栏展开/折叠、断开当前连接、重连当前连接、新建文件传输。
3. 下方文件栏改为 tab 面板，至少包含“文件”和“隧道”两个页签。
4. 连接生命周期统一管理，为终端、SFTP、文件传输、隧道提供统一的连接状态和断开/重连入口。
5. 新建文件传输窗口，类似 Xftp：左侧本机，右侧远程，远程后续支持多个 tab。
6. 隧道规则支持和当前终端会话关联，连接后自动启动 enabled 规则，第一版先支持 Local Forward。

---

## 执行状态

- [x] 阶段 0：同步独立维护规则
- [x] 阶段 1：基线整理与最小 App 分层
- [x] 阶段 2：顶部工具栏与底部面板骨架
- [x] 阶段 3：连接生命周期统一入口
- [x] 阶段 4：终端类型与 Legacy 引擎边界
- [x] 阶段 5：接入 `alacritty_terminal` 的实验终端引擎
- [x] 阶段 6：终端鼠标与 TUI 交互增强
- [ ] 阶段 7：独立文件传输窗口第一版
- [ ] 阶段 8：隧道规则与本地端口转发第一版
- [ ] 阶段 9：配置、错误提示、文档与体验收尾

---

## 1. 总体原则

### 1.1 独立维护原则

项目后续按独立项目维护，不再优先考虑 upstream merge。

要求：

- 不再为了兼容上游而保留不必要的补丁式结构。
- 可以重构，但重构必须服务于当前阶段目标或已明确规划的后续功能。
- 不做推倒重写。
- 不一次性重构所有模块。
- 每个阶段只解决一个主要问题。
- 能用明确结构解决的问题，不引入复杂插件系统。
- 不为了未来可能存在的功能提前设计大框架。
- 每个阶段完成后，项目都应能正常编译和运行。

### 1.2 重构边界

本计划中的重构只服务于以下能力：

- 终端引擎替换。
- 顶部工具栏和底部面板。
- 连接生命周期统一。
- 独立文件传输窗口。
- 隧道规则和本地端口转发。
- 终端鼠标与 TUI 交互。

不要借这些阶段顺手重写主题系统、配置系统、全部 UI 布局或整个 SSH/SFTP 实现。

### 1.3 Codex 执行原则

每个阶段交给 Codex 执行时，应要求：

- 先阅读 `docs/code-map.md`。
- 先确认当前文件位置和关键符号，再修改。
- 修改涉及文件、函数、回调、结构体、枚举、trait、模块、Slint 组件、UI 属性或跨文件依赖时，同步更新 `docs/code-map.md`。
- 不顺手重构无关代码。
- 不删除已有功能。
- 不改动与当前阶段无关的 UI 样式和业务逻辑。
- 不提前实现后续阶段功能。
- 每个阶段完成后运行：
  - `cargo fmt`
  - `cargo check`
  - 如已有测试或本阶段影响已有测试，则运行 `cargo test`

每个阶段完成后，Codex 应输出：

1. 改了哪些文件。
2. 为什么改这些文件。
3. 哪些功能故意没有做。
4. `cargo fmt` / `cargo check` / `cargo test` 结果。
5. 必要的手动测试建议。

### 1.4 阶段划分方式

本计划分为 10 个阶段：

- 阶段 0：同步独立维护规则
- 阶段 1：基线整理与最小 App 分层
- 阶段 2：顶部工具栏与底部面板骨架
- 阶段 3：连接生命周期统一入口
- 阶段 4：终端类型与 Legacy 引擎边界
- 阶段 5：接入 `alacritty_terminal` 的实验终端引擎
- 阶段 6：终端鼠标与 TUI 交互增强
- 阶段 7：独立文件传输窗口第一版
- 阶段 8：隧道规则与本地端口转发第一版
- 阶段 9：配置、错误提示、文档与体验收尾

---

# 阶段 0：同步独立维护规则

## 目标

把仓库内原先的 fork/upstream sync 约束改为独立维护约束，避免后续 Codex 执行计划时仍被“兼容上游”目标误导。

## 不做什么

本阶段不做：

- 不改业务代码。
- 不改 UI。
- 不改 SSH/SFTP/终端逻辑。
- 不新增功能。

## 建议修改文件

```text
AGENTS.md
```

## 具体任务

# 阶段 1：基线整理与最小 App 分层

## 目标

降低 `src/app.rs` 的继续膨胀风险，但不进行大规模架构重写。

当前 `src/app.rs` 同时承担 UI 状态机、tabs、terminals、SFTP 状态、终端渲染、搜索、选区、拖拽、侧边栏刷新和 Slint 回调路由等职责。第一阶段只做“最小分层入口”，不改变现有功能行为。

## 不做什么

本阶段不做：

- 不接入 `alacritty_terminal`。
- 不改 SSH 连接模型。
- 不改 SFTP 行为。
- 不新增顶部工具栏。
- 不新增文件传输窗口。
- 不改终端渲染逻辑。
- 不抽复杂 action/reducer 系统。
- 不把 sessions、tabs、terminals 全部塞进新的 AppState。

## 建议新增文件

```text
src/app_state.rs
```

如后续确实需要，可再增加：

```text
src/app_actions.rs
src/app_layout.rs
```

但第一版不建议一次性引入多个空抽象文件。

## 具体任务

### 任务 1.1：抽出最小 AppState

把 `src/app.rs` 中与阶段 2 直接相关的简单 UI 状态抽成 `AppState`。

建议只抽：

```rust
pub struct AppState {
    pub sidebar_visible: bool,
    pub bottom_panel_visible: bool,
    pub bottom_panel_tab: BottomPanelTab,
}

pub enum BottomPanelTab {
    Files,
    Tunnels,
}
```

### 任务 1.2：暂不强行抽复杂 AppAction

如果阶段 1 没有真实 action 分发逻辑，可以暂不新增 `AppAction`。

如需要提前为阶段 2 铺路，只允许定义最小动作：

```rust
pub enum AppAction {
    ToggleSidebar,
    ToggleBottomPanel,
    SelectBottomPanelTab(BottomPanelTab),
}
```

不要提前加入连接、文件传输、隧道相关 action。

### 任务 1.3：保持现有功能不变

本阶段只允许小范围替换状态读写位置，不允许改变现有 UI 行为。

## 验收标准

- `cargo fmt` 通过。
- `cargo check` 通过。
- 原有 SSH 连接、新建 tab、关闭 tab、SFTP 面板、终端输入输出仍可使用。
- `docs/code-map.md` 已更新新增文件和职责。

## Codex 提示词建议

```text
请执行阶段 1：基线整理与最小 App 分层。
要求：
1. 先阅读 docs/code-map.md。
2. 只抽出最小 AppState，字段限于 sidebar_visible、bottom_panel_visible、bottom_panel_tab。
3. 不要大规模搬迁 app.rs。
4. 不改变现有终端、SSH、SFTP 行为。
5. 不提前实现工具栏、连接管理、终端引擎重构。
6. 完成后更新 docs/code-map.md。
7. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 2：顶部工具栏与底部面板骨架

## 目标

先完成可见 UI 外壳：

- 标签页上方新增工具栏。
- 工具栏包含第一批固定按钮。
- 下方文件栏改成 tab 骨架，包含“文件”和“隧道”。
- “文件”页签继续承载现有 SFTP 面板。
- “隧道”页签先只显示空状态。

## 不做什么

本阶段不做：

- 不实现隧道转发。
- 不实现文件传输独立窗口功能。
- 不接入 `alacritty_terminal`。
- 不大改 SFTP 内部逻辑。
- 不为了重连按钮临时实现复杂重连流程。

## 建议新增文件

```text
ui/top_action_bar.slint
ui/bottom_panel.slint
ui/tunnel_panel.slint
```

## 具体任务

### 任务 2.1：新增顶部工具栏 UI

工具栏按钮固定为：

1. 展开/折叠侧边栏
2. 展开/折叠下方文件栏
3. 断开当前连接
4. 重连当前连接
5. 新建文件传输

按钮先使用现有 `IconButton` 或 `GhostButton` 风格，不新增复杂设计系统。

### 任务 2.2：绑定工具栏回调

在 `ui/app.slint` 中新增必要 callback，在 `src/app.rs` 中绑定：

```text
toggle-sidebar
toggle-bottom-panel
disconnect-active-tab
reconnect-active-tab
open-transfer-window
```

第一版行为：

- 侧边栏展开/折叠：真实生效。
- 底部面板展开/折叠：真实生效。
- 断开当前连接：优先复用现有断开逻辑。
- 重连当前连接：如果已有明确重连逻辑则复用；如果没有，先显示占位提示或 disabled，不为它临时引入复杂连接生命周期。
- 新建文件传输：先显示占位提示或日志，真实功能放到阶段 7。

### 任务 2.3：底部面板 tab 化

新增 `BottomPanel`：

```text
BottomPanel
  - Files tab: existing SftpPanel
  - Tunnels tab: TunnelPanel empty state
```

要求现有 SFTP 功能不退化。

## 验收标准

- 顶部工具栏显示在 tab 栏下方或终端区域上方。
- 侧边栏按钮可切换显示状态。
- 底部面板按钮可切换显示状态。
- 底部面板有“文件”和“隧道”两个页签。
- “文件”页签保留现有 SFTP 功能。
- “隧道”页签显示占位提示。
- “重连”和“新建文件传输”未完整实现时，有明确占位提示或 disabled 状态。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 2：顶部工具栏与底部面板骨架。
要求：
1. 先阅读 docs/code-map.md。
2. 新增 top_action_bar.slint、bottom_panel.slint、tunnel_panel.slint。
3. 工具栏按钮固定，不要设计可配置工具栏。
4. 底部面板只做“文件/隧道”两个 tab。
5. 文件 tab 继续使用现有 SFTP 面板。
6. 不实现真实隧道和文件传输窗口。
7. 如果没有现成重连逻辑，重连按钮先做占位提示或 disabled，不要临时大改连接逻辑。
8. 更新 docs/code-map.md。
9. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 3：连接生命周期统一入口

## 目标

在不大改 SSH 实现细节的前提下，引入一个最小的连接管理入口，为后续断开/重连、文件传输、隧道状态展示做准备。

本阶段不是要把所有 SSH channel 全部重写，也不是实现复杂连接池，而是先把“连接、断开、重连、状态”收敛到一个位置。

## 不做什么

本阶段不做：

- 不强制 SFTP 立即复用终端 SSH 连接。
- 不实现隧道。
- 不实现复杂连接池。
- 不做多路复用大重构。
- 不改变认证方式。
- 不引入泛型化连接框架。

## 建议新增文件

```text
src/connection.rs
```

## 具体任务

### 任务 3.1：定义 ConnectionStatus

定义简单状态：

```rust
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}
```

这个状态后续给 tab、顶部工具栏、文件传输窗口、隧道面板复用。

### 任务 3.2：定义最小 SessionRuntime / ConnectionManager

建议第一版使用较直接的结构，例如：

```rust
pub struct SessionRuntime {
    pub session_id: String,
    pub status: ConnectionStatus,
    // 包装或记录现有 ssh::SessionHandle 等运行态信息
}

pub struct ConnectionManager {
    // 管理 session_id -> SessionRuntime
}
```

第一版只包装现有 `ssh::spawn_session(...)` 和已有 handle，不重写底层 SSH。

### 任务 3.3：统一断开 / 重连入口

把阶段 2 工具栏中的断开/重连行为接到 `ConnectionManager`。

如果重连需要依赖原始 session 配置，则只用现有配置重新打开，不引入复杂恢复系统。

## 验收标准

- 原有 SSH 连接可用。
- 顶部工具栏断开行为可用。
- 顶部工具栏重连行为有统一入口；如果功能尚不完善，应清晰显示失败或占位，而不是静默无效。
- tab 状态没有明显退化。
- 没有引入复杂连接池或泛型抽象。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 3：连接生命周期统一入口。
要求：
1. 先阅读 docs/code-map.md。
2. 新增最小 connection.rs。
3. 定义 ConnectionStatus。
4. 定义最小 SessionRuntime / ConnectionManager，只包装现有 ssh::spawn_session 和 handle。
5. 只改断开、重连、状态入口。
6. 不重写 SSH 底层，不实现复杂连接池，不强制 SFTP 复用终端连接。
7. 更新 docs/code-map.md。
8. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 4：终端类型与 Legacy 引擎边界

## 目标

为后续接入 `alacritty_terminal` 做准备，但本阶段不改变终端行为。

本阶段把现有终端相关数据类型和 Legacy 终端解析逻辑从 `src/app.rs` 中逐步抽出，让 UI 层和终端解析逻辑之间有明确边界。

## 不做什么

本阶段不做：

- 不引入 `alacritty_terminal` 依赖。
- 不改变终端渲染模型字段的语义。
- 不重写搜索、选区、复制逻辑。
- 不重写键盘输入映射。
- 不改变终端显示行为。

## 建议新增文件

```text
src/terminal_types.rs
src/terminal_engine.rs
```

如一次性抽两个文件改动过大，可以先新增 `src/terminal_types.rs`，再新增 `src/terminal_engine.rs`。

## 具体任务

### 任务 4.1：抽出终端纯数据类型

将 `BuiltScreen`、`Line`、`HistSpan` 等终端渲染数据类型从 `src/app.rs` 抽到 `src/terminal_types.rs`。

要求：

- 类型语义不变。
- UI 渲染结果不变。
- 搜索、选区、复制逻辑暂时可继续留在 `app.rs`。

### 任务 4.2：抽出 LegacyTerminalEngine

新增最小终端引擎边界：

```rust
pub trait TerminalEngine {
    fn ingest(&mut self, bytes: &[u8]);
    fn render(&self) -> BuiltScreen;
    fn resize(&mut self, rows: usize, cols: usize);
}
```

用现有 `TermBuffer` 包装：

```rust
pub struct LegacyTerminalEngine {
    buffer: TermBuffer,
}
```

如果 `TermBuffer` 与 app.rs 耦合过深，本阶段允许先把 `TermBuffer` 移到 `terminal_engine.rs`，再逐步包装，不强行一次性完成所有 trait 化。

### 任务 4.3：替换最小使用点

只把直接持有 `TermBuffer` 的地方替换为 `LegacyTerminalEngine` 或轻量 enum。

不要一口气迁移所有搜索、选区、复制逻辑。如果这些逻辑强依赖 `TermBuffer` 内部结构，本阶段允许保留原结构，先只完成 ingest/render 边界。

## 验收标准

- 终端显示行为与改动前一致。
- SSH 输出仍能正常显示。
- 输入、复制、搜索、选区不退化。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 4：终端类型与 Legacy 引擎边界。
要求：
1. 先阅读 docs/code-map.md。
2. 先抽 terminal_types，再抽 terminal_engine。
3. 用 LegacyTerminalEngine 包装现有 TermBuffer。
4. 不接入 alacritty_terminal。
5. 不改变终端现有功能行为。
6. 不重写搜索、选区、复制。
7. 如果 TermBuffer 耦合较深，允许分步迁移，不要硬改成复杂 trait 架构。
8. 更新 docs/code-map.md。
9. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 5：接入 alacritty_terminal 的实验终端引擎

## 目标

新增一个实验性的 `AlacrittyTerminalEngine`，初步使用 `alacritty_terminal` 处理 VT/ANSI 解析和终端状态。

本阶段目标是“能跑、能显示、可切换、可回退”，不是一次性达到完美终端模拟。

## 不做什么

本阶段不做：

- 不删除 LegacyTerminalEngine。
- 不默认强制启用 alacritty 引擎。
- 不要求鼠标 TUI 完整可用。
- 不要求所有颜色、选区、搜索完全一致。
- 不重写 SSH 层。
- 不把 alacritty 内部类型泄漏到 `app.rs` 或 Slint UI 模型。

## 建议新增或修改文件

```text
Cargo.toml
src/terminal_engine.rs
```

如果阶段 4 后文件已经较大，可拆为：

```text
src/terminal/mod.rs
src/terminal/types.rs
src/terminal/legacy.rs
src/terminal/alacritty.rs
src/terminal/adapter.rs
```

但只有当单文件已经明显不易维护时才拆目录。

## 具体任务

### 任务 5.1：添加依赖和特性开关

在 `Cargo.toml` 中添加 `alacritty_terminal`。

增加引擎模式：

```rust
pub enum TerminalEngineMode {
    Legacy,
    AlacrittyExperimental,
}
```

初期可以通过常量、环境变量或配置项控制，不要做复杂 UI 设置页。

### 任务 5.2：实现 AlacrittyTerminalEngine 的基本 ingest/render

至少支持：

- 接收 SSH 输出 bytes。
- 解析 ANSI/VT 序列。
- 转换为当前 Slint 可用的 `BuiltScreen` 或等价渲染模型。
- 支持 resize。

建议单独保留转换层，例如：

```rust
AlacrittyScreenAdapter
```

不要让 UI 层直接依赖 alacritty 内部 grid/cell 类型。

### 任务 5.3：保留回退能力

如果 alacritty 引擎出错，应可以切回 legacy 引擎。

第一版不要求运行时无缝切换，可以在启动前选择。

## 验收标准

- Legacy 引擎仍然可用。
- Alacritty 实验引擎能显示普通 shell 输出。
- bash、zsh、PowerShell 基本输出不乱码。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

建议手测：

```text
普通输出：
- echo hello
- ls / dir
- clear
- 长行自动换行
- 中文 / emoji / 宽字符

颜色：
- 彩色 ls
- 256 色
- bold / underline / inverse

屏幕控制：
- top / htop
- vim / nano
- less
- tmux

Shell：
- bash
- zsh
- PowerShell

Resize：
- 改变窗口大小后不崩溃
- TUI 应用 resize 后刷新正常

回退：
- 环境变量或配置切回 Legacy 后功能正常
```

## Codex 提示词建议

```text
请执行阶段 5：接入 alacritty_terminal 的实验终端引擎。
要求：
1. 先阅读 docs/code-map.md。
2. 保留 LegacyTerminalEngine，不要删除旧实现。
3. 新增 AlacrittyTerminalEngine。
4. 新增或保持清晰的 AlacrittyScreenAdapter，不要把 alacritty 内部类型泄漏到 app.rs / Slint UI。
5. 初期只要求基本显示、resize、普通输入输出可用。
6. 不实现完整鼠标交互。
7. 不大改 SSH/SFTP 模块。
8. 更新 docs/code-map.md。
9. 运行 cargo fmt 和 cargo check。
10. 给出 echo、ls/dir、clear、中文、PowerShell、top/htop、vim/nano、resize 的手测建议。
```

---

# 阶段 6：终端鼠标与 TUI 交互增强

## 目标

在阶段 5 的 `AlacrittyTerminalEngine` 基础上增强鼠标交互，使 `htop`、`vim`、`tmux`、`less`、`nano` 等 TUI 应用体验更接近主流终端。

## 不做什么

本阶段不做：

- 不重写文件传输。
- 不改隧道逻辑。
- 不新增复杂主题系统。
- 不要求一次性覆盖所有 xterm mouse mode。
- 不破坏普通文本选择。

## 建议修改文件

```text
src/terminal_engine.rs
ui/terminal_view.slint
src/app.rs
```

如果已有 `src/terminal/` 目录，则相应修改：

```text
src/terminal/mouse.rs
src/terminal/input.rs
```

## 具体任务

### 任务 6.1：把鼠标坐标映射为终端 row/col

在 `TerminalView` 中把鼠标位置转换为：

```text
terminal_col
terminal_row
```

注意行列从 1 还是从 0 开始，最终发送到远端时要符合 xterm mouse 协议。

### 任务 6.2：支持基础 SGR Mouse Mode

优先支持：

- 左键按下
- 左键释放
- 滚轮上
- 滚轮下

输出类似：

```text
ESC [ < button ; col ; row M
ESC [ < button ; col ; row m
```

### 任务 6.3：处理应用是否开启鼠标模式

只有远端程序开启 mouse reporting mode 时才发送鼠标序列。

如果当前引擎可以识别 mouse mode，则按状态发送；如果暂时识别困难，可以先做实验开关，但默认不能破坏普通文本选择。

## 验收标准

- 普通终端文本选择不明显退化。
- `htop` 中鼠标点击或滚轮基本可用。
- `vim` / `tmux` 中鼠标行为不造成明显乱码。
- `less` / `nano` 中鼠标或滚轮不造成明显异常。
- Legacy 引擎仍可使用。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 6：终端鼠标与 TUI 交互增强。
要求：
1. 先阅读 docs/code-map.md。
2. 基于阶段 5 的 AlacrittyTerminalEngine 增强鼠标输入。
3. 优先支持 SGR Mouse Mode 的左键、释放、滚轮。
4. 只有应用开启 mouse reporting mode 时才发送鼠标序列；无法判断时默认不要破坏普通文本选择。
5. Legacy 引擎必须仍可用。
6. 更新 docs/code-map.md。
7. 运行 cargo fmt 和 cargo check。
8. 给出 htop、vim、tmux、less、nano 的手测建议。
```

---

# 阶段 7：独立文件传输窗口第一版

## 目标

实现“新建文件传输”按钮的真实功能：打开独立窗口，左侧显示本机文件，右侧显示当前远程会话文件列表。

第一版重点是可用，不追求完整 Xftp。

## 不做什么

本阶段不做：

- 不实现断点续传。
- 不实现复杂传输队列。
- 不实现本地收藏夹。
- 不实现远程站点管理。
- 不实现远程多会话批量管理。
- 不强制和当前终端共享同一条 SSH transport。
- 不让文件传输窗口关闭影响主终端。

## 建议新增文件

```text
src/file_transfer.rs
ui/transfer_window.slint
ui/local_file_panel.slint
ui/remote_file_panel.slint
```

## 具体任务

### 任务 7.1：新增 TransferWindow

窗口布局：

```text
TransferWindow
  左侧：LocalFilePanel
  右侧：RemoteFilePanel / Remote tab
```

第一版可以只支持一个远程 tab，但数据结构应允许后续多个 remote tab。

### 任务 7.2：实现本机文件列表

用 `std::fs::read_dir` 实现最小本机目录浏览：

- 显示文件名。
- 显示目录/文件类型。
- 支持进入目录。
- 支持返回上级目录。

### 任务 7.3：包装或复用现有 SFTP 能力

右侧远程文件面板优先复用现有 `src/sftp.rs` 的列表、上传、下载、删除能力。

如果 `sftp.rs` 与主窗口 SFTP 面板耦合较深，先增加一个薄 wrapper，例如：

```rust
pub struct TransferSftpClient {
    // wraps existing SftpHandle or equivalent command channel
}
```

不要把主窗口 SFTP 面板状态和文件传输窗口状态揉在一起。

第一版可只实现：

- 列目录。
- 进入目录。
- 返回上级目录。
- 下载远程文件到当前本地目录。
- 上传本地文件到当前远程目录。

### 任务 7.4：主窗口按钮打开文件传输窗口

点击顶部工具栏“新建文件传输”：

- 如果当前有 active terminal session，则打开 TransferWindow 并连接当前远程。
- 如果没有 active session，则显示提示，不崩溃。

## 验收标准

- 点击“新建文件传输”能打开独立窗口。
- 左侧能浏览本机目录。
- 右侧能浏览当前 SSH session 的远程目录。
- 至少支持基本上传和下载。
- 关闭文件传输窗口不影响主终端。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 7：独立文件传输窗口第一版。
要求：
1. 先阅读 docs/code-map.md。
2. 新增 transfer_window.slint、local_file_panel.slint、remote_file_panel.slint、src/file_transfer.rs。
3. 左侧本地文件用 std::fs::read_dir 实现。
4. 右侧优先复用现有 sftp.rs；如果耦合较深，先做 TransferSftpClient 薄封装。
5. 第一版只做基本浏览、上传、下载。
6. 不实现复杂传输队列和断点续传。
7. 关闭文件传输窗口不能影响主终端。
8. 更新 docs/code-map.md。
9. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 8：隧道规则与本地端口转发第一版

## 目标

实现下方“隧道”页签的第一版可用功能：

- 支持 Local Forward。
- 规则和当前 session 关联。
- 连接 session 后自动启动 enabled 的规则。
- 隧道失败不影响主终端连接。

本阶段建议分为两个子阶段：

- 阶段 8A：Tunnel Core
- 阶段 8B：TunnelPanel + 持久化 + 自动启动

## 不做什么

本阶段不做：

- 不做 Remote Forward。
- 不做 Dynamic SOCKS。
- 不做复杂跳板链。
- 不做全局规则模板。
- 不强制复用当前终端 SSH transport。
- 不做复杂隧道编排系统。

## 建议新增文件

```text
src/tunnel.rs
ui/tunnel_rule_dialog.slint
```

如果阶段 2 的 `ui/tunnel_panel.slint` 已存在，本阶段扩展它。

## 阶段 8A：Tunnel Core

### 任务 8A.1：定义 TunnelRule

第一版结构：

```rust
pub struct TunnelRule {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub enabled: bool,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}
```

不要一开始就抽象 `TunnelKind`，除非实现时确实需要。第一版只有 Local Forward。

### 任务 8A.2：定义 TunnelStatus / TunnelHandle

建议状态：

```rust
pub enum TunnelStatus {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Failed(String),
}
```

要求每条隧道都有明确 stop/cancel 入口。

### 任务 8A.3：实现 Local Forward Core

行为：

```text
监听 local_host:local_port
收到本地 TCP 连接
通过 SSH 打开 direct-tcpip 到 remote_host:remote_port
双向转发数据
```

第一版可以每条规则独立 SSH 连接，但必须包含：

- keepalive
- 自动重连
- backoff
- 状态显示
- stop/cancel handle
- 端口占用错误提示

## 阶段 8B：TunnelPanel + 持久化 + 自动启动

### 任务 8B.1：规则持久化

规则保存到单独配置文件，例如：

```text
tunnels.json
```

不要第一版就把规则塞进原有 Session 结构，避免牵连会话编辑 UI。

### 任务 8B.2：UI 管理规则

`TunnelPanel` 支持：

- 显示规则列表。
- 显示状态：Stopped / Starting / Running / Reconnecting / Failed。
- 新增规则。
- 启用/禁用规则。
- 删除规则。

### 任务 8B.3：session 关联和自动启动

行为：

- session 连接后，自动启动当前 session 下 enabled 的 Local Forward 规则。
- session 断开后，对应规则停止或进入断开状态。
- 隧道规则失败不影响主终端。

## 验收标准

- 能新增 Local Forward 规则。
- 能保存和读取规则。
- session 连接后 enabled 规则自动启动。
- session 断开后对应规则停止或进入断开状态。
- 本地端口访问能转发到远程地址。
- 本地端口被占用时能显示明确错误。
- 隧道断线后能自动重连或显示失败状态。
- 停止/删除规则后后台任务确实退出，本地端口释放。
- `cargo fmt`、`cargo check` 通过。
- `docs/code-map.md` 已更新。

## Codex 提示词建议

```text
请执行阶段 8：隧道规则与本地端口转发第一版。
要求：
1. 先阅读 docs/code-map.md。
2. 只实现 Local Forward。
3. 新增 TunnelRule、TunnelStatus、TunnelHandle 和 tunnel.rs。
4. 先完成 Tunnel Core，再扩展 TunnelPanel。
5. 规则保存到独立 tunnels.json，不改原有 Session 结构。
6. 支持新增、启用、禁用、删除规则。
7. 支持 keepalive、自动重连、backoff、状态显示、stop/cancel handle。
8. 停止或删除规则后必须释放本地端口。
9. 隧道失败不能影响主终端连接。
10. 不做 Remote Forward 和 Dynamic SOCKS。
11. 更新 docs/code-map.md。
12. 运行 cargo fmt 和 cargo check。
```

---

# 阶段 9：配置、错误提示、文档与体验收尾

## 目标

在主要能力完成后，做一次克制的收尾整理，重点解决配置、状态恢复、用户体验和文档一致性。

## 不做什么

本阶段不做大功能。尤其不做：

- 不新增新的传输协议。
- 不新增插件系统。
- 不重写主题系统。
- 不重构全部 UI。
- 不新增复杂配置迁移框架。

## 具体任务

### 任务 9.1：配置文件整理

检查以下配置是否稳定：

- session 配置
- `tunnels.json`
- 文件传输窗口默认本地路径
- 终端引擎模式配置
- 底部面板默认显示状态

原则：

- 能不迁移旧配置就不迁移。
- 必须迁移时，写清兼容逻辑。
- 不引入复杂版本迁移框架，除非已有配置已经明显不兼容。

### 任务 9.2：连接状态统一显示

统一主窗口、tab、文件传输窗口、隧道面板中的状态：

```text
Connecting
Connected
Disconnected
Reconnecting
Failed
```

不要每个面板各自发明一套状态文案。

### 任务 9.3：基础错误提示优化

重点优化：

- SSH 连接失败。
- SFTP 连接失败。
- 文件上传/下载失败。
- 隧道端口被占用。
- 隧道认证失败。
- alacritty 引擎初始化失败。

错误提示要具体，但不做复杂错误码系统。

### 任务 9.4：文档更新

更新：

```text
docs/code-map.md
README.md 或 docs/usage.md
```

写清：

- 顶部工具栏功能。
- 文件传输窗口使用方式。
- 隧道规则使用方式。
- 终端引擎模式说明。
- Legacy / Alacritty Experimental 的切换方式。

## 验收标准

- 配置重启后能恢复。
- 常见错误有明确提示。
- 文档与实际功能一致。
- `cargo fmt`、`cargo check`、`cargo test` 通过。

## Codex 提示词建议

```text
请执行阶段 9：配置、错误提示、文档与体验收尾。
要求：
1. 先阅读 docs/code-map.md。
2. 不新增大功能。
3. 统一连接状态文案。
4. 检查配置持久化和重启恢复。
5. 优化常见错误提示。
6. 更新 docs/code-map.md 和使用文档。
7. 运行 cargo fmt、cargo check、cargo test。
```

---

# 推荐执行顺序

建议严格按以下顺序执行：

```text
阶段 0：同步独立维护规则
阶段 1：基线整理与最小 App 分层
阶段 2：顶部工具栏与底部面板骨架
阶段 3：连接生命周期统一入口
阶段 4：终端类型与 Legacy 引擎边界
阶段 5：alacritty_terminal 实验引擎
阶段 6：终端鼠标与 TUI 交互增强
阶段 7：独立文件传输窗口第一版
阶段 8：隧道规则与本地端口转发第一版
阶段 9：配置、错误提示、文档与体验收尾
```

其中阶段 3 和阶段 4 都是基础重构阶段，但它们的目的不同：

- 阶段 3 是为了连接、断开、重连、状态统一。
- 阶段 4 是为了终端核心替换。

不建议把这两个阶段合并，否则改动面会过大。

---

# 每个阶段的提交建议

每个阶段建议至少一个独立分支：

```text
phase-0-independent-maintenance
phase-1-app-state
phase-2-toolbar-bottom-panel
phase-3-connection-manager
phase-4-terminal-engine-boundary
phase-5-alacritty-engine
phase-6-terminal-mouse-tui
phase-7-transfer-window
phase-8-tunnel-local-forward
phase-9-polish-config-docs
```

每个阶段提交信息建议：

```text
phase N: 简短说明
```

例如：

```text
phase 2: add top action bar and bottom panel tabs
```

---

# 风险控制

## 高风险阶段

高风险阶段包括：

- 阶段 5：接入 `alacritty_terminal`
- 阶段 6：终端鼠标/TUI 交互
- 阶段 8：隧道 Local Forward

这些阶段建议单独开发、单独测试，不要和 UI 样式调整混在一起。

## 回退策略

必须保留以下回退能力：

- 终端引擎可回退 Legacy。
- 文件传输窗口关闭不影响主窗口。
- 隧道规则失败不影响终端连接。
- 工具栏新增按钮失败不影响原有 tab 操作。

## 后台任务安全

涉及 SSH、SFTP、文件传输、隧道、重连循环时，必须满足：

- 每个后台任务都有明确 stop/cancel 入口。
- UI 状态不能显示“已停止”，但后台任务仍在运行。
- 自动重连必须有 backoff。
- session/window/rule 删除后，相关后台任务必须退出。
- 端口监听失败必须显示具体错误。

---

# 最终目标状态

完成所有阶段后，项目应具备以下结构特征：

```text
app.rs
  仍是入口，但不再无限膨胀

app_state
  保存少量全局 UI 状态

connection
  统一管理连接、断开、重连、状态

terminal_types / terminal_engine
  支持 Legacy 和 Alacritty Experimental

file_transfer
  支持独立窗口、本地/远程双栏、基础上传下载

tunnel
  支持 session 关联的 Local Forward 规则

ui
  有顶部工具栏、底部 tab 面板、独立文件传输窗口
```

最终使用体验应达到：

- 日常 SSH 连接和原项目一致或更稳定。
- 顶部工具栏提供常用操作。
- 底部面板支持文件和隧道切换。
- 文件传输窗口具备基础 Xftp 使用体验。
- 终端模拟能力明显强于原有实现。
- TUI 应用和 PowerShell 兼容性明显提升。
- 隧道 Local Forward 可用于常见开发场景。
