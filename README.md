# meatshell

**简体中文** | [English](./README.en.md)

一个轻量级、低内存占用的 SSH / 终端客户端，灵感来自 FinalShell，但完全由
**Rust + [Slint](https://slint.dev)** 实现。目标是保留 FinalShell 的核心体验
（资源监控侧栏、会话管理、多标签页终端）的同时，把内存占用从 400 MB+ 的
JVM 压到几十 MB 原生级别。

## 截图

<p align="center">
  <img src="docs/screenshots/01-welcome.png" alt="欢迎页 / 会话管理" width="800"><br>
  <em>欢迎页：会话管理 + 左侧本机资源监控</em>
</p>

<p align="center">
  <img src="docs/screenshots/02-terminal-btop.png" alt="终端 + SFTP" width="800"><br>
  <em>多标签页终端（btop 全屏渲染）+ 底部 SFTP 文件浏览 + 远端资源监控</em>
</p>

## 下载与安装

每次打 `v*` 标签，GitHub Actions 会自动构建 **Windows / Linux / macOS** 三平台二进制，
发布到 [Releases](https://github.com/jeff141/meatshell/releases) 页面。

### Windows

下载 `meatshell-*-windows-x86_64.zip`，解压后双击 `meatshell.exe`。

### Linux

```bash
tar -xzf meatshell-*-linux-x86_64.tar.gz
cd meatshell-*-linux-x86_64
./meatshell                                  # 直接运行
# 可选：装应用图标 + 启动器入口（Dock / 应用列表里显示图标，无需传参）
chmod +x install-linux.sh && ./install-linux.sh
```

> 需要 glibc ≥ 2.35（Ubuntu 22.04+ / Debian 12+）。Wayland 下首次装完图标可能要注销重登一次。

### macOS

```bash
tar -xzf meatshell-*-macos-*.tar.gz          # aarch64 = Apple 芯片，x86_64 = Intel
xattr -dr com.apple.quarantine meatshell     # 去掉「未签名应用」的 Gatekeeper 拦截
./meatshell
```

> 从源码构建见下方 [运行](#运行)。

## 路线图

### v0.1（当前）

- [x] FinalShell 风格深色主题 UI
- [x] 左侧本机系统监控（CPU / 内存 / 交换 / 网络吞吐，1 Hz）
- [x] 多标签页（欢迎页 + 多个终端会话）
- [x] 会话管理：新建 / 编辑 / 删除，本地 JSON 持久化
  - 配置位置：`%APPDATA%/meatshell/sessions.json`（Windows）
    / `~/.config/meatshell/sessions.json`（Linux）
    / `~/Library/Application Support/meatshell/sessions.json`（macOS）
- [x] SSH 连接骨架（`russh`，纯 Rust 实现，支持密码 + 私钥）
- [x] 行缓冲终端视图（输入一行 → 回车发送）

### v0.2

- [ ] 完整 VT/ANSI 终端模拟（`alacritty_terminal` 实验引擎已可选启用，鼠标/TUI 仍在后续阶段）
- [ ] 远端主机资源监控（与 FinalShell 一样执行远端脚本收集）
- [x] SFTP 文件浏览 + 拖拽上传/下载
- [x] 顶部工具栏骨架：侧边栏、底部面板、断开、重连、文件传输入口
- [x] 底部“文件 / 隧道”页签骨架（文件页继续使用 SFTP 面板）
- [x] alacritty 实验引擎的基础 SGR 鼠标上报（左键、释放、滚轮）
- [x] 独立文件传输窗口第一版（本地/远程双栏、基础上传下载）
- [x] 隧道 Local Forward 第一版（session 关联、启用后自动启动、独立 `tunnels.json`）
- [ ] 已知主机 (known_hosts) 校验
- [ ] 会话密码使用 OS 钥匙串存储

### v0.3+

- [ ] 多标签页终端分屏
- [ ] 会话分组 / 文件夹
- [ ] 主题切换（浅色 / 跟随系统）
- [ ] 命令历史与片段管理

## 技术栈

| 模块          | 选型                                                              |
| ------------- | ----------------------------------------------------------------- |
| UI            | [Slint](https://slint.dev)（纯 Rust 编译，无 GC）                 |
| 异步运行时    | [`tokio`](https://tokio.rs)                                       |
| SSH 协议      | [`russh`](https://crates.io/crates/russh)（无 libssh 依赖）       |
| 终端解析      | 默认 legacy `vt100`；实验 [`alacritty_terminal`](https://crates.io/crates/alacritty_terminal) |
| 隧道转发      | `russh` direct-tcpip + `tokio` TCP 转发                         |
| 系统指标      | [`sysinfo`](https://crates.io/crates/sysinfo)                     |
| 序列化        | `serde` + `serde_json`                                            |
| 日志          | `tracing` + `tracing-subscriber`                                  |

## 运行

```bash
cargo run --release
```

实验性的 alacritty 终端引擎默认不启用；需要启动前设置环境变量：

```bash
MEATSHELL_TERMINAL_ENGINE=alacritty cargo run --release
```

PowerShell：

```powershell
$env:MEATSHELL_TERMINAL_ENGINE = "alacritty"; cargo run --release
```

首次启动会建立空的会话库。点击右上角 **“＋ 新建会话”** 添加第一台服务器。

## 常用功能

- 顶部工具栏：切换左侧资源栏、切换底部面板、断开当前 tab、重连当前 tab、打开独立文件传输窗口。
- 文件传输窗口：在已连接的终端 tab 上点击工具栏文件传输按钮，左侧浏览本机目录，右侧浏览当前远程 session，支持基础上传/下载。
- 隧道：底部 **“隧道”** 页签支持 Local Forward。新增规则后填写 `本地地址:端口 -> 远端地址:端口`，保存并启用；该 session 下 enabled 规则会在终端连接成功后自动启动，断开或关闭 tab 时停止。
- 终端引擎：默认使用 legacy `vt100`；启动前设置 `MEATSHELL_TERMINAL_ENGINE=alacritty` 可切换到实验 alacritty 引擎。

## 配置文件

`sessions.json` 保存会话、语言和下载目录；`tunnels.json` 单独保存隧道规则，不写入会话结构。

默认配置目录：

- Windows：`%APPDATA%\meatshell\meatshell\config`
- Linux：`~/.config/meatshell`
- macOS：`~/Library/Application Support/dev.meatshell.meatshell`

终端引擎模式目前只由启动前环境变量控制；侧边栏/底部面板默认显示状态暂不持久化。

## 项目布局

```
meatshell/
├── Cargo.toml
├── build.rs                 # Slint 编译器入口
├── ui/
│   ├── app.slint            # 顶层窗口
│   ├── theme.slint          # 设计 tokens
│   ├── widgets.slint        # 可复用按钮 / 输入框 / sparkline
│   ├── sidebar.slint        # 左侧系统监控面板
│   ├── tabs.slint           # 顶部标签栏
│   ├── top_action_bar.slint # 标签页下方工具栏
│   ├── bottom_panel.slint   # 底部文件 / 隧道页签外壳
│   ├── tunnel_panel.slint   # Local Forward 隧道规则面板
│   ├── transfer_window.slint # 独立文件传输窗口
│   ├── local_file_panel.slint # 文件传输本地面板
│   ├── remote_file_panel.slint # 文件传输远程面板
│   ├── welcome.slint        # 欢迎页 / 快速连接
│   ├── session_dialog.slint # 新建 / 编辑会话弹框
│   └── terminal_view.slint  # 终端视图（v0.1 行缓冲）
└── src/
    ├── main.rs
    ├── app/                 # UI 状态机与后端 glue
    │   ├── mod.rs
    │   ├── state.rs
    │   ├── layout.rs
    │   ├── events.rs
    │   ├── models.rs
    │   ├── platform.rs
    │   ├── sessions.rs
    │   ├── sidebar.rs
    │   ├── sftp_panel.rs
    │   ├── tabs.rs
    │   ├── terminal_input.rs
    │   ├── terminal_render.rs
    │   ├── transfer.rs
    │   ├── tunnels.rs
    │   └── types.rs
    ├── connection.rs        # 连接运行态、断开、重连入口
    ├── config.rs            # 会话 JSON 持久化
    ├── file_transfer.rs     # 文件传输窗口本地目录 helper
    ├── i18n.rs              # 运行时语言切换
    ├── proxy.rs             # SSH / SFTP 出站代理
    ├── ssh.rs               # SSH 会话 worker
    ├── ssh_config.rs        # 导入 ~/.ssh/config
    ├── sftp.rs              # SFTP worker
    ├── system.rs            # CPU / 内存 / 网络采样
    ├── tunnel.rs            # Local Forward 隧道规则与后台转发任务
    └── terminal/
        ├── mod.rs
        ├── alacritty.rs     # alacritty 实验终端引擎
        ├── engine.rs        # 终端引擎 trait
        ├── legacy.rs        # legacy vt100 实现
        └── types.rs         # 终端渲染数据类型
```

## 开发提示

- Slint 控件有非常严格的布局 DSL，改 `.slint` 后 `cargo check` 是最快的
  反馈方式。
- 应用事件循环是单线程（Slint 要求），所有跨线程 UI 更新通过
  `slint::invoke_from_event_loop` 回调。
- 目前 `check_server_key` 接受任意服务端密钥（类似 `StrictHostKeyChecking=no`），
  生产使用前请接入 known_hosts 校验。

## License

MIT OR Apache-2.0（双许可）。
