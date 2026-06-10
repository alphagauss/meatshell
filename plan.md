# meatshell dev 分支改动执行计划

> 适用场景：`main` 分支保持上游同步，`dev` 分支已经把原上游大文件 `src/app.rs` 拆成 `src/app/*.rs`，并新增了 Alacritty 模式、顶部菜单栏、底部窗格、隧道窗格、文件传输窗口等个性化功能。
>
> 执行原则：**以 dev 当前模块化架构为主，迁移 main 的成熟代码到 dev 的合适模块，不允许用 main 的 `src/app.rs` 整体覆盖 dev。**

---

## 执行进度

- [x] step-001：建立 dev 基线并确认差异范围
- [x] step-002：回归上游依赖项，支持主题、字体和 Wayland 剪贴板
- [x] step-003：回归上游配置字段：主题、终端字体、会话分组
- [x] step-004：回归上游主题和终端字体设置的 Rust 接线
- [x] step-005：回归上游 Slint 主题字体属性和设置界面
- [x] step-006：回归上游 SFTP 递归上传、递归下载、递归删除
- [x] step-007：回归上游安全文件名和远程打开/编辑的文件名保护
- [x] step-008：回归上游终端粘贴换行修复和 Alt 裸键修复
- [x] step-009：回归上游远程资源监控安全修复
- [x] step-010：回归上游 shell integration 隐藏注入命令修复
- [x] step-011：回归上游连接配置导入/导出和会话分组基础能力
- [x] step-012：修复 debug 启动 ICU4X ja 分词模型重复报错
- [x] step-013：修复主界面菜单栏位置，菜单栏在标签页上方
- [x] step-014：修复标签页/菜单按钮 tooltip 被遮挡
- [x] step-015：增加统一 active session guard，禁止新标签页执行会话操作
- [x] step-016：修复 Alacritty 模式普通页面鼠标滚动不生效
- [x] step-017：文件传输窗口改为单例窗口
- [x] step-018：文件传输窗口右侧远程区域支持多 tab
- [x] step-019：文件传输窗口 tab 双击重新连接
- [x] step-020：文件传输窗口底部增加传输记录窗格
- [x] step-021：文件传输窗口增加本地/远程右键菜单
- [x] step-022：文件传输窗口支持文件夹传输
- [x] step-023：文件传输窗口显示更多文件信息
- [x] step-024：隧道窗格改为右键菜单管理
- [x] step-025：侧边栏新增会话程序监测表
- [ ] step-026：侧边栏和底部窗格增加快速平移动画
- [ ] step-027：最终验证和整理

---

## 总体执行规则

1. 每个 step 尽量独立提交一次 git commit。
2. 每完成一个 step，至少执行：

```bash
cargo fmt
cargo check
```

3. 涉及 UI 的 step，至少启动一次 debug 程序，确认窗口能打开。
4. 上游回归类 step 必须优先从 `meatshell_main/` 复制或迁移已有代码，不要重新设计同类功能。
5. `src/app.rs` 是 main 的上游参考文件；dev 中对应拆分落点如下：

| main 单体位置                  | dev 推荐落点                           |
| ------------------------------ | -------------------------------------- |
| `src/app.rs` 初始化、全局回调  | `src/app/mod.rs`                       |
| 会话列表、导入导出、连接对话框 | `src/app/sessions.rs`                  |
| 终端输入、粘贴、鼠标、滚动     | `src/app/terminal_input.rs`            |
| 终端渲染、主题色、字体应用     | `src/app/terminal_render.rs`           |
| SFTP 面板回调                  | `src/app/sftp_panel.rs`                |
| 侧边栏资源刷新                 | `src/app/sidebar.rs`                   |
| 标签页与连接工具栏             | `src/app/tabs.rs`                      |
| 文件传输窗口                   | `src/app/transfer.rs`                  |
| 隧道管理                       | `src/app/tunnels.rs`、`src/tunnel.rs`  |
| 全局状态                       | `src/app/state.rs`、`src/app/types.rs` |

6. 本轮暂不迁移 Serial / Telnet，除非某个上游修复必须依赖它们。
7. 本轮字体观感只回归 main 的主题/字体设置能力，不做 Alacritty 字距、行高、字体 fallback 的大重构。

---

# step-001：建立 dev 基线并确认差异范围

## 目标

确认当前 dev 可以作为开发基线，明确禁止整体覆盖模块化结构。

## 修改文件

不修改业务代码。只允许新增临时记录或测试日志，最终不要提交临时文件。

## 执行

在 dev 根目录执行：

```bash
cargo fmt
cargo check
```

如果当前 dev 已经无法通过 `cargo check`，先记录错误，不要在本 step 大规模修复；后续 step 修复对应问题。

## 验收

- 明确当前项目入口是 `src/app/mod.rs`。
- 明确 `meatshell_main/src/app.rs` 只作为迁移参考。
- 不删除 dev 的 `src/app/` 目录。
- 不用 main 的 `src/app.rs` 覆盖 dev。

---

# step-002：回归上游依赖项，支持主题、字体和 Wayland 剪贴板

## 类型

上游功能回归。

## 目标

把 main 中主题、系统字体枚举、Wayland 剪贴板所需依赖迁移到 dev。

## 修改文件

- `Cargo.toml`

## 代码来源

参考：

- `meatshell_main/Cargo.toml`

## 执行

在 dev 的 `Cargo.toml` 中迁移以下依赖。注意：不要迁移 Serial / Telnet 相关依赖，除非后续明确实现 Serial / Telnet。

需要添加到 `[dependencies]`：

```toml
# Enumerate installed system fonts for the Interface (font) settings picker.
fontdb = "0.16"

# Detect whether the OS is running in dark or light mode.
# Used on startup to honour the system preference before the user overrides it.
dark-light = "1"
```

Linux 下 `arboard` 需要使用 Wayland feature。dev 当前已有 `arboard = "3"`，保留基础依赖，并增加 target-specific 配置：

```toml
[target.'cfg(target_os = "linux")'.dependencies]
# Native Wayland clipboard (wlr-data-control protocol) so copy/paste works on
# Wayland sessions without relying on XWayland.
arboard = { version = "3", features = ["wayland-data-control"] }
```

如果 Cargo 提示同一个 target 下重复声明 `arboard` 冲突，优先采用 main 的写法。

## 不迁移

本 step 不迁移：

```toml
serialport = "4"
chacha20poly1305 = "0.10"
image = { ... }
```

除非后续 step 明确需要。

## 验收

```bash
cargo check
```

通过或只剩下后续 step 预期内的未使用/未接线错误。

---

# step-003：回归上游配置字段：主题、终端字体、会话分组

## 类型

上游功能回归。

## 目标

让 dev 的配置层支持 main 已有的：

- `theme_pref`
- `font_family`
- `font_size`
- `Session.group`

这是后续主题字体设置、会话分组的基础。

## 修改文件

- `src/config.rs`

## 代码来源

参考：

- `meatshell_main/src/config.rs`

## 执行

### 1. 给 `Session` 增加 `group`

在 dev 的 `Session` 结构体中，`proxy` 后或 `last_used` 前增加：

```rust
/// Optional folder/group name to organize sessions in the list.
/// Empty = ungrouped. Sessions are grouped by this in Quick Connect.
#[serde(default)]
pub group: String,
```

在 `Session::new_empty()` 中增加默认值：

```rust
group: String::new(),
```

### 2. 给 `ConfigFile` 增加主题和字体字段

在 dev 的 `ConfigFile` 中增加：

```rust
/// Theme preference: "system" (default) | "dark" | "light".
#[serde(default)]
pub theme_pref: String,
/// Terminal font family. Empty = the built-in default (Cascadia Mono).
#[serde(default)]
pub font_family: String,
/// Terminal font size in px. 0 = the built-in default.
#[serde(default)]
pub font_size: u32,
```

### 3. 给 `ConfigStore` 增加访问方法

从 main 迁移以下方法到 dev 的 `impl ConfigStore`：

```rust
/// Theme preference: "system" (default) | "dark" | "light".
pub fn theme_pref(&self) -> &str {
    if self.cache.theme_pref.is_empty() {
        "system"
    } else {
        &self.cache.theme_pref
    }
}

pub fn set_theme_pref(&mut self, pref: String) {
    self.cache.theme_pref = pref;
}

/// Terminal font family ("" = built-in default).
pub fn font_family(&self) -> &str {
    &self.cache.font_family
}

pub fn set_font_family(&mut self, family: String) {
    self.cache.font_family = family;
}

/// Terminal font size in px (falls back to 13 when unset).
pub fn font_size(&self) -> u32 {
    if self.cache.font_size == 0 {
        13
    } else {
        self.cache.font_size
    }
}

pub fn set_font_size(&mut self, size: u32) {
    self.cache.font_size = size.clamp(8, 32);
}
```

## 注意事项

- 必须使用 `#[serde(default)]`，否则旧配置文件会反序列化失败。
- 不要改变现有 `terminal_engine` 配置。
- 不要删除 dev 已有的 `TerminalEngineMode` 支持。

## 验收

- 旧 `sessions.json` 能正常加载。
- 新字段缺省时不会崩溃。
- `cargo check` 不因字段缺失失败。

---

# step-004：回归上游主题和终端字体设置的 Rust 接线

## 类型

上游功能回归。

## 目标

把 main 中主题模式、终端字体、终端字号的初始化和保存逻辑迁移到 dev。

## 修改文件

- `src/app/mod.rs`

## 代码来源

参考：

- `meatshell_main/src/app.rs` 中窗口创建后的主题/字体初始化逻辑
- `meatshell_main/src/app.rs` 中 `system_monospace_fonts()`

## 执行

### 1. 在 `src/app/mod.rs` imports 中确认已有这些类型

如果没有，需要补齐：

```rust
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
```

`dev/src/app/mod.rs` 基本已有这些 import，避免重复导入。

### 2. 在 `AppWindow::new()` 后、`sync_app_state_to_window` 前后合适位置加入主题初始化

迁移 main 的逻辑：

```rust
// Apply the saved (or system-detected) theme.
// "dark" / "light" → use that directly; "system" or unset → ask the OS;
// OS unknown → fall back to dark.
{
    let is_dark = match store.borrow().theme_pref() {
        "light" => false,
        "dark" => true,
        _ => match dark_light::detect() {
            dark_light::Mode::Light => false,
            dark_light::Mode::Dark => true,
            dark_light::Mode::Default => true,
        },
    };
    window.set_dark_mode(is_dark);
}
```

### 3. 加入字体初始化

```rust
// Apply the saved terminal font. Empty family keeps the built-in default;
// the size always applies and defaults to 13.
{
    let s = store.borrow();
    let fam = s.font_family().to_string();
    if !fam.is_empty() {
        window.set_term_font_family(fam.into());
    }
    window.set_term_font_size(s.font_size() as f32);
}

window.set_term_fonts(ModelRc::from(Rc::new(VecModel::from(system_monospace_fonts()))));
```

### 4. 加入字体保存回调

放在已有设置类回调附近，例如 `window.on_set_terminal_engine_mode(...)` 附近：

```rust
{
    let weak = window.as_weak();
    let store = store.clone();
    window.on_set_term_font(move |family: SharedString| {
        {
            let mut s = store.borrow_mut();
            s.set_font_family(family.to_string());
            let _ = s.save();
        }
        if let Some(w) = weak.upgrade() {
            w.set_term_font_family(family);
        }
    });
}

{
    let weak = window.as_weak();
    let store = store.clone();
    window.on_set_term_font_size(move |size: i32| {
        {
            let mut s = store.borrow_mut();
            s.set_font_size(size as u32);
            let _ = s.save();
        }
        if let Some(w) = weak.upgrade() {
            w.set_term_font_size(size as f32);
        }
    });
}
```

### 5. 在 `src/app/mod.rs` 文件底部加入系统等宽字体扫描函数

从 main 迁移：

```rust
/// Enumerate installed monospace font families for the Interface font picker.
/// Terminals want fixed-width fonts, so non-monospace families are filtered out.
fn system_monospace_fonts() -> Vec<slint::SharedString> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .filter(|f| f.monospaced)
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(slint::SharedString::from).collect()
}
```

## 注意事项

- 如果 `window.set_dark_mode`、`window.set_term_font_family`、`window.set_term_font_size`、`window.set_term_fonts` 尚不存在，先完成 step-005 的 Slint 属性，再回到本 step。
- dev 已有 `terminal_engine_mode`，不要覆盖。

## 验收

- `cargo check` 不再提示对应 Slint 方法不存在。
- 启动后能读取配置中的主题和字体。
- 修改字体后配置能保存。

---

# step-005：回归上游 Slint 主题字体属性和设置界面

## 类型

上游功能回归。

## 目标

让 UI 层支持可配置终端字体和字号，并把 legacy / Alacritty 终端都接到同一套主题字体属性上。

## 修改文件

- `ui/theme.slint`
- `ui/app.slint`
- `ui/terminal_view.slint`

## 代码来源

参考：

- `meatshell_main/ui/theme.slint`
- `meatshell_main/ui/app.slint`
- `meatshell_main/ui/terminal_view.slint`

## 执行

### 1. 在 `ui/theme.slint` 中加入终端字体属性

从 main 迁移：

```slint
// Terminal font — user-configurable via Interface settings, set by Rust
// from the config on startup. cell size in terminal_view derives from these.
in-out property <string> term-font-family: "Cascadia Mono";
in-out property <length> term-font-size: 13px;
```

### 2. 在 `ui/app.slint` 的 `AppWindow` 根组件属性区加入

```slint
in-out property <string> term-font-family <=> Theme.term-font-family;
in-out property <length> term-font-size <=> Theme.term-font-size;
in property <[string]> term-fonts;
callback set-term-font(string);
callback set-term-font-size(int);
```

### 3. 在设置菜单中增加字体选择和字号设置

dev 当前 settings popup 比 main 简化。可以先采用最小实现：在 `ui/app.slint` 的 Settings menu popup 内增加一个二级区域，放在 terminal engine 行后、about 行前。

建议 UI 先做简单可用版本：

```slint
Rectangle { height: 1px; background: Theme.border-subtle; }

Text {
    text: root.lang-en ? "Terminal font" : "终端字体";
    color: Theme.text-secondary;
    font-size: Theme.fs-xs;
}

ComboBox {
    height: 28px;
    model: root.term-fonts;
    current-value: root.term-font-family;
    selected(v) => { root.set-term-font(v); }
}

HorizontalLayout {
    height: 28px;
    spacing: 8px;
    Text {
        text: root.lang-en ? "Font size" : "字号";
        color: Theme.text-primary;
        font-size: Theme.fs-sm;
        vertical-alignment: center;
    }
    SpinBox {
        minimum: 8;
        maximum: 32;
        value: root.term-font-size / 1px;
        edited(v) => { root.set-term-font-size(v); }
    }
    Text {
        text: (root.term-font-size / 1px) + " px";
        color: Theme.text-muted;
        font-size: Theme.fs-xs;
        vertical-alignment: center;
    }
}
```

如果 Slint 版本没有 `ComboBox` 或 `SpinBox`，则参考 main 的完整 `ui/app.slint` 中 Interface settings 实现，不要重新设计复杂控件。

### 4. 修改 `ui/terminal_view.slint`

把终端文字使用的固定字体和固定字号替换为：

```slint
font-family: Theme.term-font-family;
font-size: Theme.term-font-size;
```

涉及 terminal 文本、cursor/cell 计算等位置时，要确保 cell width/height 跟 `Theme.term-font-size` 同步。

## 验收

- 设置中能看到终端字体和字号入口。
- 修改字体/字号后，当前或新建终端能看到变化。
- 重启后字体/字号仍然生效。
- Alacritty 模式不再使用明显偏小的硬编码字号。

---

# step-006：回归上游 SFTP 递归上传、递归下载、递归删除

## 类型

上游功能回归。

## 目标

把 main 已实现的文件夹递归传输能力迁移到 dev。后续文件传输窗口也复用这套能力。

## 修改文件

- `src/sftp.rs`
- `src/app/sftp_panel.rs`
- `src/app/transfer.rs`

## 代码来源

参考：

- `meatshell_main/src/sftp.rs`

重点代码范围：

- `SftpCommand::Download` 分支：main `src/sftp.rs` 约第 344-389 行
- `SftpCommand::Upload` 分支：main `src/sftp.rs` 约第 391-443 行
- `SftpCommand::Delete` 分支：main `src/sftp.rs` 约第 445-480 行
- `sanitize_filename()`：main `src/sftp.rs` 约第 592 行开始
- `download_dir()`：main `src/sftp.rs` 约第 799 行开始
- `remove_dir_recursive()`：main `src/sftp.rs` 约第 834 行开始
- `upload_dir()`：main `src/sftp.rs` 约第 864 行开始
- `upload_pipelined()`：main `src/sftp.rs` 约第 901 行开始

## 执行

### 1. 迁移 Download 分支目录判断

把 dev `SftpCommand::Download { remote, local_dir }` 的处理逻辑改为 main 的目录优先逻辑：

```rust
let is_dir = sftp
    .metadata(&remote)
    .await
    .ok()
    .map(|m| (m.permissions.unwrap_or(0) & 0o170_000) == 0o040_000)
    .unwrap_or(false);

if is_dir {
    let dirname = base_name(&remote);
    let _ = events.send(SessionEvent::SftpStatus(format!(
        "{} {}/...", t("下载文件夹", "Downloading folder"), dirname
    )));
    match download_dir(&sftp, &remote, &local_dir, &events).await {
        Ok(_) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {}", t("下载完成", "Downloaded"), dirname
            )));
        }
        Err(e) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {e}", t("下载失败", "Download failed")
            )));
        }
    }
} else {
    let filename = sanitize_filename(&base_name(&remote));
    let local_path = format!("{}/{}", local_dir.trim_end_matches('/'), filename);
    let id = Uuid::new_v4().to_string();
    match download_impl(&sftp, &remote, &local_path, &filename, &id, &events).await {
        Ok(_) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {}", t("下载完成", "Downloaded"), filename
            )));
        }
        Err(e) => {
            emit_transfer(&events, &id, &filename, false, 0, 0, 2, &e.to_string());
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {e}", t("下载失败", "Download failed")
            )));
        }
    }
}
```

### 2. 迁移 Upload 分支目录判断

核心逻辑：

```rust
let is_dir = tokio::fs::metadata(&local)
    .await
    .map(|m| m.is_dir())
    .unwrap_or(false);

if is_dir {
    let dirname = base_name(&local);
    let res = upload_dir(&handle, &sftp, &local, &remote_dir, &events).await;
    if let Ok(entries) = list_dir_impl(&sftp, &remote_dir).await {
        let _ = events.send(SessionEvent::SftpEntries {
            path: remote_dir.clone(),
            entries,
        });
    }
    match res {
        Ok(_) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {}", t("上传完成", "Uploaded"), dirname
            )));
        }
        Err(e) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {e}", t("上传失败", "Upload failed")
            )));
        }
    }
} else {
    // 单文件上传继续使用 upload_pipelined
}
```

### 3. 迁移 Delete 分支递归删除

核心逻辑：

```rust
let is_dir = sftp
    .metadata(&path)
    .await
    .ok()
    .map(|m| (m.permissions.unwrap_or(0) & 0o170_000) == 0o040_000)
    .unwrap_or(false);

let res: Result<()> = if is_dir {
    remove_dir_recursive(&sftp, &path).await
} else {
    sftp.remove_file(&path)
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("{e}"))
};
```

### 4. 从 main 原样复制辅助函数

从 `meatshell_main/src/sftp.rs` 原样复制以下函数到 dev 的 `src/sftp.rs`：

```rust
fn sanitize_filename(name: &str) -> String { ... }
async fn download_dir(...) -> Result<()> { ... }
async fn remove_dir_recursive(...) -> Result<()> { ... }
async fn upload_dir(...) -> Result<()> { ... }
async fn upload_pipelined(...) -> Result<()> { ... }
```

不要重写逻辑。复制后只做必要 import 适配。

### 5. 检查 imports

确保 `src/sftp.rs` 中包含：

```rust
use anyhow::{anyhow, Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use russh_sftp::client::{RawSftpSession, SftpSession};
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
```

## 验收

- SFTP 面板可以上传本地文件夹。
- SFTP 面板可以下载远程文件夹。
- SFTP 面板可以递归删除远程文件夹。
- 单文件上传仍然走 pipelined upload。
- 文件传输进度事件仍然发出。

---

# step-007：回归上游安全文件名和远程打开/编辑的文件名保护

## 类型

上游功能回归。

## 目标

避免远程服务端返回恶意文件名时，本地下载或临时编辑路径出现路径穿越、Windows 保留名或 shell 特殊字符问题。

## 修改文件

- `src/sftp.rs`

## 代码来源

参考：

- `meatshell_main/src/sftp.rs` 的 `sanitize_filename()`
- `meatshell_main/src/sftp.rs` 中单文件下载和 `OpenTemp` 分支

## 执行

在所有远程文件名落地到本地路径的地方，使用：

```rust
let filename = sanitize_filename(&base_name(&remote));
```

重点检查：

- `SftpCommand::Download`
- `SftpCommand::OpenTemp { remote, edit }`
- 递归下载 `download_dir()` 中每个目录和文件名

不要把远程完整路径直接拼到本地路径。

## 验收

- 远程文件名为 `../../evil.txt` 时，不会写出目标目录。
- 远程文件名为 `CON.txt`、`NUL`、`COM1` 时，Windows 下不会命中设备名。
- 远程文件名带 `&`、`$`、`'` 等字符时，本地文件名被安全替换。

---

# step-008：回归上游终端粘贴换行修复和 Alt 裸键修复

## 类型

上游 bug 修复回归。

## 目标

迁移 main 中终端输入修复：

- 粘贴多行命令时统一换行为 CR；
- 单独按 Alt 不应发送 ESC，不应清空当前命令；
- 保留真实 Alt+字母作为 Meta 输入。

## 修改文件

- `src/app/terminal_input.rs`

## 代码来源

参考：

- `meatshell_main/src/app.rs` 的 `normalize_pasted_newlines()`
- `meatshell_main/src/app.rs` 的 `key_to_pty_bytes()` 相关 Alt guard

## 执行

### 1. 增加换行归一化函数

```rust
/// Normalise pasted text's line endings to a single CR (0x0d).
fn normalize_pasted_newlines(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
}
```

### 2. 修改 paste 逻辑

在 `window.on_paste_from_clipboard` 中，当前代码大致是：

```rust
clipboard_payload(&text, TerminalEngine::bracketed_paste(buf))
```

改为先归一化：

```rust
let text = normalize_pasted_newlines(&text);
clipboard_payload(&text, TerminalEngine::bracketed_paste(buf))
```

### 3. 修复 Alt 裸键

在 `key_to_pty_bytes` 中，`key.is_empty()` 之后增加对常见裸修饰键的忽略：

```rust
// Ignore bare modifier keys. Slint may send Alt alone as U+0012 with alt=true;
// it must not be treated as Meta+Ctrl-R or ESC-prefixed input.
if matches!(key, "\u{0010}" | "\u{0011}" | "\u{0012}" | "\u{0013}" | "\u{0014}") {
    return vec![];
}
```

### 4. 修复 dev 当前重复的 `if alt && !ctrl`

dev 当前 `src/app/terminal_input.rs` 中存在重复的：

```rust
if alt && !ctrl {
if alt && !ctrl {
```

改为单个分支：

```rust
if alt && !ctrl {
    let mut bytes = vec![0x1b];
    bytes.extend_from_slice(key.as_bytes());
    return bytes;
}
```

### 5. 增加测试

在已有 `#[cfg(test)]` 中增加：

```rust
#[test]
fn paste_normalizes_newlines_to_cr() {
    assert_eq!(normalize_pasted_newlines("a\nb\nc"), "a\rb\rc");
    assert_eq!(normalize_pasted_newlines("a\r\nb"), "a\rb");
    assert_eq!(normalize_pasted_newlines("echo hi"), "echo hi");
}

#[test]
fn bare_alt_sends_nothing() {
    assert!(key_to_pty_bytes("\u{0012}", false, true, false).is_empty());
}
```

## 验收

- 粘贴反斜杠续行命令不会被错误拆开。
- 单按 Alt 不会清空命令行。
- Alt+a 仍然发送 ESC + `a`。

---

# step-009：回归上游远程资源监控安全修复

## 类型

上游 bug 修复回归。

## 目标

迁移 main 中远程资源监控加固：

- 固定 PATH；
- 限制 monitor buffer；
- 限制网卡/磁盘条目数量；
- 使用 saturating arithmetic；
- 避免恶意服务端输出导致内存增长或 debug panic。

## 修改文件

- `src/ssh.rs`
- `src/app/sidebar.rs`
- `src/app/types.rs`

## 代码来源

参考：

- `meatshell_main/src/ssh.rs`

## 执行

### 1. 替换监控命令

使用 main 的固定 PATH 版本：

```rust
const MON_CMD: &[u8] = b"PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH; while :; do awk '/^cpu /{print}' /proc/stat; awk '/^(MemTotal|MemAvailable|SwapTotal|SwapFree):/{print}' /proc/meminfo; cat /proc/net/dev; echo __DF__; df -kP 2>/dev/null; echo __MSTICK__; sleep 2; done\n";
```

### 2. 限制 monitor buffer

在处理 `mon_buf` 的位置加入：

```rust
const MON_BUF_CAP: usize = 1 << 20;
if mon_buf.len() > MON_BUF_CAP {
    mon_buf.clear();
}
```

### 3. 限制条目数量

在 `parse_monitor_block` 中加入：

```rust
const MAX_MON_ENTRIES: usize = 64;
```

解析磁盘和网卡时都要检查长度上限。

### 4. 使用饱和计算

CPU total、idle、内存差值、KiB 转 bytes 均使用 main 的方式：

```rust
cpu_total = nums.iter().copied().fold(0u64, u64::saturating_add);
cpu_idle = nums[3].saturating_add(nums.get(4).copied().unwrap_or(0));

mem_used_kib: mem_total.saturating_sub(mem_avail),
swap_used_kib: swap_total.saturating_sub(swap_free),

Some((mount, avail_kb.saturating_mul(1024), total_kb.saturating_mul(1024)))
```

## 验收

- 远程资源监控仍正常显示 CPU、内存、swap、网卡、磁盘。
- 异常输出不会导致 UI 卡死或内存无限增长。
- 测试中构造超大数字不会 panic。

---

# step-010：回归上游 shell integration 隐藏注入命令修复

## 类型

上游 bug 修复回归。

## 目标

上游通过注入 `PROMPT_COMMAND` 获取远程 CWD，但注入命令不应显示到终端，也不应污染 shell history。

## 修改文件

- `src/ssh.rs`

## 代码来源

参考：

- `meatshell_main/src/ssh.rs` 中 `PROMPT_SETUP`、`ECHO_NEEDLE`、`suppress_echo` 逻辑

## 执行

迁移以下核心逻辑：

```rust
let mut prompt_injected = false;
let mut suppress_echo = false;

const PROMPT_SETUP: &[u8] = b" export PROMPT_COMMAND='printf \"\\033]7;file://${HOSTNAME}${PWD}\\007\"' && eval \"$PROMPT_COMMAND\"\r";

const ECHO_NEEDLE: &str = "export PROMPT_COMMAND='printf \"\\033]7;file://${HOSTNAME}${PWD}\\007\"' && eval \"$PROMPT_COMMAND\"";
```

在收到第一段真实 shell 输出后注入：

```rust
if !prompt_injected && !text.trim().is_empty() {
    prompt_injected = true;
    suppress_echo = true;
    let _ = channel.data(PROMPT_SETUP).await;
}

if suppress_echo {
    text = text.replace(ECHO_NEEDLE, "");
}

if let Some(cwd) = extract_osc7_path(&text) {
    suppress_echo = false;
    let _ = events.send(SessionEvent::CwdChanged(cwd));
}
```

## 验收

- 连接 SSH 后不会看到 `export PROMPT_COMMAND=...` 命令闪现。
- SFTP 面板仍可跟随远程 cwd。
- 该命令不应出现在 shell history 中。

---

# step-011：回归上游连接配置导入/导出和会话分组基础能力

## 类型

上游功能回归。

## 目标

迁移 main 中较成熟的连接配置管理增强。这个 step 可以在主题和 SFTP 回归后执行。

## 修改文件

- `src/config.rs`
- `src/app/sessions.rs`
- `ui/welcome.slint`
- `ui/session_dialog.slint`
- `ui/app.slint`

## 代码来源

参考：

- `meatshell_main/src/config.rs`
- `meatshell_main/src/app.rs` 中 `sync_sessions_to_model`、导入导出、move group、toggle group 逻辑
- `meatshell_main/ui/welcome.slint`
- `meatshell_main/ui/session_dialog.slint`

## 执行

### 1. 会话分组

给 Slint `SessionInfo` 增加字段：

```slint
group: string,
group-header: string,
collapsed: bool,
```

给 `SessionDraft` 增加：

```slint
group: string,
```

Rust 中同步修改对应结构体映射。

### 2. 迁移分组排序逻辑

把 main `sync_sessions_to_model` 中按 group 排序的逻辑迁移到 dev 的：

- `src/app/sessions.rs::sync_sessions_to_model`

核心行为：

- 空 group 显示为 `default`；
- 按 group 排序；
- 每组第一行写入 `group_header`；
- 折叠状态由 UI 控制。

### 3. 迁移 move-session / toggle-group 回调

dev `ui/app.slint` 增加：

```slint
callback move-session(string /* session-id */, string /* group */);
callback toggle-group(string /* group */);
```

Rust 中在 `src/app/sessions.rs` 接线。

### 4. 迁移导入/导出

从 main `src/config.rs` 迁移：

```rust
pub fn export_to(&self, path: &Path) -> Result<usize> { ... }
pub fn import_from(&mut self, path: &Path) -> Result<(usize, usize)> { ... }
```

从 main `src/app.rs` 把 `on_export_sessions`、`on_import_sessions` 的逻辑迁移到 dev `src/app/sessions.rs`。

## 验收

- 新建会话可以设置 group。
- 欢迎页按 group 展示。
- group 可折叠。
- 导出连接配置为 JSON。
- 导入 JSON 时跳过重复项。
- 旧会话没有 group 时进入 `default`。

---

# step-012：修复 debug 启动 ICU4X ja 分词模型重复报错

## 类型

dev bug 修复。

## 目标

解决 debug 启动时控制台反复输出：

```text
ICU4X data error: No segmentation model for language: ja
```

## 修改文件

- `src/i18n.rs`
- `src/app/mod.rs`

## 执行

### 1. 在 i18n 中增加语言白名单归一化

```rust
pub fn normalize_language(code: &str) -> &'static str {
    let lower = code.to_ascii_lowercase();
    if lower.starts_with("en") {
        "en"
    } else if lower.starts_with("zh") {
        "zh"
    } else {
        "zh"
    }
}
```

### 2. 修改 `set_language`

```rust
pub fn set_language(code: &str) {
    let code = normalize_language(code);
    let en = code == "en";
    LANG.store(if en { EN } else { ZH }, Ordering::Relaxed);
    apply_to_slint();
}
```

### 3. `ConfigStore::language()` 返回值也要避免 ja

如果配置中存在 `ja` 或系统传入 `ja`，保存时统一变为 `zh` 或 `en`。

### 4. 初始化顺序

在 `src/app/mod.rs` 中，语言设置仍在 `AppWindow::new()` 后执行，但传入必须是 `normalize_language(...)` 后的值。

## 验收

- debug 启动时不再重复输出 ICU4X ja segmentation model 错误。
- 中文/英文切换仍正常。
- 配置中写入 `ja` 后，下次启动也会 fallback 到 `zh`。

---

# step-013：修复主界面菜单栏位置，菜单栏在标签页上方

## 类型

dev bug 修复。

## 目标

当前 dev 中 `TopActionBar` 在 `TabBar` 下方，应改为菜单栏在上、标签页在下。

## 修改文件

- `ui/app.slint`

## 执行

当前 dev 右侧主区域顺序大致是：

```slint
TabBar { ... }
TopActionBar { ... }
Rectangle { /* content */ }
```

改为：

```slint
TopActionBar {
    sidebar-visible: root.sidebar-visible;
    bottom-panel-visible: root.bottom-panel-visible;
    toggle-sidebar => { root.toggle-sidebar(); }
    toggle-bottom-panel => { root.toggle-bottom-panel(); }
    disconnect-active-tab => { root.disconnect-active-tab(); }
    reconnect-active-tab => { root.reconnect-active-tab(); }
    open-transfer-window => { root.open-transfer-window(); }
}

TabBar {
    tabs: root.tabs;
    active-id: root.active-tab-id;
    tab-selected(id) => {
        root.active-tab-id = id;
        root.tab-selected(id);
    }
    tab-closed(id) => { root.tab-closed(id); }
    new-tab() => { root.new-tab-clicked(); }
}

Rectangle {
    vertical-stretch: 1;
    background: Theme.bg-root;
    // content
}
```

## 验收

- 主菜单栏显示在最上方。
- 标签页在菜单栏下方。
- 标签切换、新建、关闭仍正常。

---

# step-014：修复标签页/菜单按钮 tooltip 被遮挡

## 类型

dev bug 修复。

## 目标

当前 `top_action_bar.slint` 中 tooltip 是按钮子元素，容易被父容器裁剪或被终端区域遮挡。改为根窗口级 tooltip。

## 修改文件

- `ui/app.slint`
- `ui/top_action_bar.slint`
- `ui/tabs.slint`

## 执行

### 1. 在 `AppWindow` 根组件增加 tooltip 状态

```slint
in-out property <string> global-tooltip-text: "";
in-out property <length> global-tooltip-x: 0px;
in-out property <length> global-tooltip-y: 0px;
```

### 2. 在根组件末尾增加统一 tooltip 浮层

放在所有 popup/dialog 后面，保证 z-order 最高：

```slint
if root.global-tooltip-text != "" : Rectangle {
    x: root.global-tooltip-x;
    y: root.global-tooltip-y;
    width: tip.preferred-width + 14px;
    height: 24px;
    border-radius: Theme.radius-sm;
    background: Theme.bg-elevated;
    border-width: 1px;
    border-color: Theme.border-subtle;
    drop-shadow-blur: 8px;
    drop-shadow-color: #00000040;

    tip := Text {
        x: 7px;
        text: root.global-tooltip-text;
        color: Theme.text-primary;
        font-size: Theme.fs-xs;
        vertical-alignment: center;
    }
}
```

### 3. 修改按钮组件

`top_action_bar.slint` 中删除按钮内部的 `if touch.has-hover : Rectangle { ... }` tooltip。

增加回调：

```slint
callback tooltip-show(string, length, length);
callback tooltip-hide();
```

在 hover 变化时通知根组件。若 Slint 当前写法不好处理 hover changed，可以先使用按钮 `TouchArea` 的 `entered/exited` 或可用事件；如果没有对应事件，则简化为取消 tooltip，避免遮挡问题。

### 4. `AppWindow` 接线

`TopActionBar` 中：

```slint
tooltip-show(text, x, y) => {
    root.global-tooltip-text = text;
    root.global-tooltip-x = x;
    root.global-tooltip-y = y;
}
tooltip-hide => {
    root.global-tooltip-text = "";
}
```

## 验收

- tooltip 不再被标签栏、菜单栏、终端区域遮挡。
- tooltip 不影响按钮点击。
- 若实现成本过高，允许先禁用 tooltip，不允许继续显示被遮挡的 tooltip。

---

# step-015：增加统一 active session guard，禁止新标签页执行会话操作

## 类型

dev bug 修复。

## 目标

欢迎页/新标签页没有连接会话时，文件、隧道、断开、重连等操作都应统一提示“请先连接一个会话”，不得误执行。

## 修改文件

- `src/app/tabs.rs`
- `src/app/transfer.rs`
- `src/app/tunnels.rs`
- `src/app/sftp_panel.rs`
- `src/app/models.rs` 或新增 helper 模块

## 执行

### 1. 增加 helper

建议在 `src/app/tabs.rs` 或新建 `src/app/guards.rs`：

```rust
pub(super) fn active_session_or_hint(
    win: &super::AppWindow,
    connections: &super::types::ConnectionStore,
) -> Option<(String, crate::config::Session)> {
    let active = win.get_active_tab_id().to_string();
    if active == "welcome" {
        win.set_ssh_import_hint(crate::i18n::t(
            "请先连接一个会话",
            "Connect a session first",
        ).into());
        return None;
    }
    let session = connections.lock().unwrap().session(&active);
    if session.is_none() {
        super::models::set_terminal_row(win, &active, |row| {
            row.status = crate::i18n::t(
                "请先连接一个会话",
                "Connect a session first",
            ).into();
        });
        return None;
    }
    session.map(|s| (active, s))
}
```

### 2. 在以下入口使用 guard

- `open_transfer_window`
- `tunnel_add_rule`
- `tunnel_update_rule`
- `tunnel_toggle_rule`
- `tunnel_delete_rule`
- `disconnect_active_tab`
- `reconnect_active_tab`
- SFTP 上传/下载/删除/查看/编辑

### 3. 统一提示文案

中文：

```text
请先连接一个会话
```

英文：

```text
Connect a session first
```

## 验收

- 欢迎页点击“新建文件传输窗口”只提示，不打开窗口。
- 欢迎页点击隧道相关按钮只提示，不创建规则。
- 欢迎页点击断开/重连不报错。
- 已连接会话中原功能正常。

---

# step-016：修复 Alacritty 模式普通页面鼠标滚动不生效

## 类型

dev bug 修复。

## 目标

Alacritty 模式下：

- 普通 shell 页面滚轮滚动本地 scrollback；
- htop/vim/less 等启用 mouse reporting 或 alt screen 时，滚轮仍发送给远程应用；
- legacy 模式不受影响。

## 修改文件

- `src/app/terminal_input.rs`
- `src/app/terminal_render.rs`
- `src/terminal/alacritty.rs`
- `src/terminal/legacy.rs`

## 执行

### 1. 给 Alacritty engine 增加 scrollback offset

在 `src/terminal/alacritty.rs` 的状态结构中增加：

```rust
pub view_offset: usize,
```

如果已有类似字段，复用已有字段。

### 2. 增加滚动方法

```rust
pub fn scroll_lines(&mut self, delta: i32) {
    let max_off = self.scrollback_len();
    let cur = self.view_offset as i64;
    self.view_offset = (cur + delta as i64).clamp(0, max_off as i64) as usize;
}
```

`scrollback_len()` 根据 alacritty_terminal 当前可回看行数实现。

### 3. 修改 `on_terminal_scroll`

当前 dev 只改 legacy 的 `buf.view_offset`。需要判断当前 engine：

```rust
if let Some(alacritty) = buf.alacritty.as_mut() {
    if !buf.is_alt_screen() && !buf.mouse_reporting() {
        alacritty.scroll_lines(delta);
    } else {
        // 保持已有 mouse wheel → remote app 逻辑
    }
} else {
    // legacy 原逻辑
    let max_off = buf.history.len() as i64;
    let cur = buf.view_offset as i64;
    buf.view_offset = (cur + delta as i64).clamp(0, max_off) as usize;
}
```

具体方法名按 dev 当前 `LegacyTerminalEngine` / `Alacritty` 封装调整。

### 4. 修改渲染

`terminal_render.rs` 渲染 Alacritty 时必须使用 Alacritty 的 `view_offset` 取可见区域，而不是始终渲染底部。

## 验收

- Alacritty 模式普通 shell 输出可滚轮上翻/下翻。
- htop 中滚轮仍由 htop 响应。
- legacy 模式滚轮不回退。

---

# step-017：文件传输窗口改为单例窗口

## 类型

新增功能。

## 目标

点击“新建文件传输窗口”时，不再创建多个窗口；应用内最多只有一个文件传输窗口。

## 修改文件

- `src/app/types.rs`
- `src/app/transfer.rs`
- `ui/transfer_window.slint`

## 执行

### 1. 修改状态结构

当前 dev：

```rust
pub(super) type TransferWindows = Rc<RefCell<Vec<TransferWindowState>>>;
```

改为单例：

```rust
#[allow(dead_code)]
pub(super) struct TransferWindowState {
    pub(super) window: super::TransferWindow,
    // 后续 step 会把 _sftp 改为多 tab map
    pub(super) sftp: Rc<SftpHandle>,
}

pub(super) type TransferWindowStore = Rc<RefCell<Option<TransferWindowState>>>;
```

如暂时不想重命名太多，也可保留 `TransferWindows` 类型名，但内部改成 `Option`。

### 2. 修改 `open_transfer_window`

逻辑：

```rust
if let Some(existing) = transfer_windows.borrow().as_ref() {
    existing.window.show()?;
    existing.window.window().request_window_properties_update();
    return Ok(());
}
```

如果 Slint 没有 `request_window_properties_update`，只调用 `show()` 即可。

### 3. 关闭窗口时隐藏，不销毁状态

```rust
window.window().on_close_requested(move || {
    let _ = window.hide();
    slint::CloseRequestResponse::HideWindow
});
```

## 验收

- 多次点击菜单按钮，窗口数量始终只有一个。
- 关闭后再次打开仍是同一个窗口逻辑。
- 当前单 tab 文件传输功能仍可用。

---

# step-018：文件传输窗口右侧远程区域支持多 tab

## 类型

新增功能。

## 目标

文件传输窗口类似 Xftp：左侧本地面板共享，右侧远程面板按 SSH 会话分 tab。

## 修改文件

- `src/app/types.rs`
- `src/app/transfer.rs`
- `ui/transfer_window.slint`
- `ui/remote_file_panel.slint`

## 执行

### 1. 新增远程 tab 数据结构

Rust：

```rust
pub(super) struct TransferRemoteTab {
    pub id: String,
    pub title: String,
    pub session: crate::config::Session,
    pub sftp: Rc<crate::sftp::SftpHandle>,
    pub remote_path: String,
    pub connected: bool,
}
```

Slint：

```slint
export struct TransferRemoteTabInfo {
    id: string,
    title: string,
    connected: bool,
}
```

### 2. 文件传输窗口增加远程 tab model

`ui/transfer_window.slint` 增加：

```slint
in property <[TransferRemoteTabInfo]> remote-tabs;
in-out property <string> active-remote-tab-id;
callback remote-tab-selected(string);
callback remote-tab-closed(string);
callback remote-tab-reconnect(string);
```

### 3. 点击打开文件传输窗口时

- 如果窗口不存在：创建窗口。
- 如果当前会话没有 remote tab：创建一个 tab。
- 如果已有：激活已有 tab。
- 不重复创建同一 session tab。

### 4. 远程文件列表按 active tab 显示

当前 `TransferWindow` 只有一份 `remote_path` 和 `remote_entries`。本 step 可以先让 UI 仍用这一份显示模型，但切换 tab 时由 Rust 刷新为该 tab 的状态。

## 验收

- 同一个会话重复打开文件传输窗口，只激活已有 remote tab。
- 不同会话打开文件传输窗口，在右侧新增不同 tab。
- 切换 tab 后远程路径和文件列表对应正确。

---

# step-019：文件传输窗口 tab 双击重新连接

## 类型

新增功能。

## 目标

文件传输窗口的远程 tab 标签双击后，用该 tab 保存的 session config 重新建立 SFTP 连接。

## 修改文件

- `ui/transfer_window.slint`
- `src/app/transfer.rs`

## 执行

### 1. Slint tab 增加双击回调

如果 Slint `TouchArea` 支持 click count，使用双击；否则临时采用右键菜单或小按钮作为 fallback。

期望回调：

```slint
callback remote-tab-reconnect(string);
```

### 2. Rust 处理重连

```rust
fn reconnect_transfer_tab(tab_id: &str) {
    // 1. 找到 tab 保存的 Session
    // 2. 关闭旧 sftp handle
    // 3. spawn_sftp(runtime.handle(), session.clone(), tx)
    // 4. 设置状态为 connecting
    // 5. 成功后 list_dir(tab.remote_path)
}
```

## 验收

- 双击远程 tab 能重新连接。
- 失败只影响当前 tab。
- 成功后刷新当前远程路径。
- 不要求用户重新输入密码。

---

# step-020：文件传输窗口底部增加传输记录窗格

## 类型

新增功能。

## 目标

文件传输窗口底部显示当前传输任务和历史记录，并复用现有 `root.transfers` 数据，不新建第二套传输任务系统。

## 修改文件

- `ui/transfer_window.slint`
- `src/app/transfer.rs`
- `src/app/events.rs`

## 执行

### 1. 复用现有 transfer model

`TransferWindow` 增加属性：

```slint
in property <[TransferInfo]> transfers;
callback clear-transfers();
```

如果 `TransferInfo` 只在 `app.slint` 定义，需要移到可 import 的公共 ui 文件，或在 `transfer_window.slint` 重复定义同名结构并由 Rust 映射。

### 2. 布局

文件传输窗口主区域：

```text
上方：本地面板 + 远程面板
下方：传输记录窗格
```

传输记录先显示：

- 方向图标；
- 名称；
- 状态/进度；
- 进度条。

### 3. Rust 同步

每次主窗口 transfer model 更新时，也同步到已打开的 transfer window。

## 验收

- 文件传输窗口底部可以看到上传/下载记录。
- 与主窗口下载管理 popup 使用同一份数据。
- 清空记录时两个地方同时清空。

---

# step-021：文件传输窗口增加本地/远程右键菜单

## 类型

新增功能。

## 目标

文件传输窗口中，本地和远程文件列表都支持右键菜单：

- 传输；
- 打开；
- 用记事本编辑；
- 重命名。

## 修改文件

- `ui/local_file_panel.slint`
- `ui/remote_file_panel.slint`
- `ui/transfer_window.slint`
- `src/app/transfer.rs`
- `src/file_transfer.rs`
- `src/sftp.rs`

## 执行

### 1. Slint 增加右键回调

本地面板：

```slint
callback local-transfer(string /* full_path */);
callback local-open(string /* full_path */);
callback local-edit(string /* full_path */);
callback local-rename(string /* full_path */, string /* new_name */);
```

远程面板：

```slint
callback remote-transfer(string /* full_path */);
callback remote-open(string /* full_path */);
callback remote-edit(string /* full_path */);
callback remote-rename(string /* full_path */, string /* new_name */);
```

### 2. 本地行为

- 传输：上传到 active remote tab 的当前目录；
- 打开：使用系统默认打开；
- 用记事本编辑：Windows 优先 `notepad.exe`，其他系统用默认编辑器或 xdg-open；
- 重命名：`std::fs::rename`。

### 3. 远程行为

- 传输：下载到当前本地目录；
- 打开：复用 `SftpCommand::OpenTemp { edit: false }`；
- 用记事本编辑：复用 `SftpCommand::OpenTemp { edit: true }`；
- 重命名：新增 SFTP rename 命令。

### 4. 新增 SFTP rename 命令

`SftpCommand` 增加：

```rust
Rename { old_path: String, new_path: String },
```

worker 中处理：

```rust
SftpCommand::Rename { old_path, new_path } => {
    match sftp.rename(&old_path, &new_path).await {
        Ok(_) => {
            let parent = parent_dir(&new_path);
            if let Ok(entries) = list_dir_impl(&sftp, &parent).await {
                let _ = events.send(SessionEvent::SftpEntries { path: parent, entries });
            }
        }
        Err(e) => {
            let _ = events.send(SessionEvent::SftpStatus(format!(
                "{}: {e}", t("重命名失败", "Rename failed")
            )));
        }
    }
}
```

## 验收

- 本地文件右键上传可用。
- 远程文件右键下载可用。
- 远程文件打开/编辑复用现有临时下载逻辑。
- 重命名成功后列表刷新。

---

# step-022：文件传输窗口支持文件夹传输

## 类型

新增功能。

## 目标

文件传输窗口复用 step-006 的 SFTP 递归能力，实现：

- 本地文件夹上传；
- 远程文件夹下载；
- 远程文件夹删除，如果菜单后续添加删除，也应递归删除。

## 修改文件

- `src/app/transfer.rs`
- `ui/local_file_panel.slint`
- `ui/remote_file_panel.slint`

## 执行

本 step 不再新增递归函数，只调用现有：

```rust
sftp.upload(local_path, remote_dir);
sftp.download(remote_path, local_dir);
```

因为 step-006 已经让 `SftpCommand::Upload` / `Download` 自动判断文件夹。

## 验收

- 文件传输窗口中，本地文件夹右键“传输”可以递归上传。
- 远程文件夹右键“传输”可以递归下载。
- 传输记录显示文件夹内文件的进度。
- 不存在 transfer window 单独实现的递归代码。

---

# step-023：文件传输窗口显示更多文件信息

## 类型

新增功能。

## 目标

文件传输窗口文件列表不只显示名称和大小，还显示：

- 类型；
- 修改时间；
- 属性；
- 所有者。

## 修改文件

- `src/ssh.rs`
- `src/file_transfer.rs`
- `src/app/transfer.rs`
- `ui/local_file_panel.slint`
- `ui/remote_file_panel.slint`
- `ui/sftp_panel.slint`

## 执行

### 1. 扩展数据结构

`RemoteEntry` 增加：

```rust
pub file_type: String,
pub permissions: String,
pub owner: String,
```

`LocalFileEntry` 增加：

```rust
pub file_type: String,
pub permissions: String,
pub owner: String,
```

Slint `SftpEntry` 增加：

```slint
type-name: string,
permissions: string,
owner: string,
```

### 2. 远程侧填充

Linux 权限可以先从 `permissions` bits 转换：

```rust
fn format_unix_mode(mode: u32, is_dir: bool) -> String {
    let mut s = String::new();
    s.push(if is_dir { 'd' } else { '-' });
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        s.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    s
}
```

owner 如果当前 SFTP metadata 只能拿 uid/gid，就先显示：

```text
uid:gid
```

### 3. 本地侧填充

Windows 无法稳定获取所有者/权限时允许为空：

```rust
permissions: String::new(),
owner: String::new(),
```

类型：

- 文件夹：`Folder` / `文件夹`
- 普通文件：按扩展名或 `File` / `文件`

### 4. UI 列

列顺序建议：

```text
名称 | 大小 | 类型 | 修改时间 | 属性 | 所有者
```

文件名列保留最大宽度和 elide，避免布局被挤爆。

## 验收

- 文件传输窗口显示新增列。
- Linux 远程文件显示类似 `drwxr-xr-x` 权限。
- 修改时间仍显示正常。
- Windows 本地权限/所有者为空也不崩溃。

---

# step-024：隧道窗格改为右键菜单管理

## 类型

新增功能。

## 目标

隧道面板去掉 Add 按钮，改为右键菜单管理。菜单包含：

- 添加；
- 删除；
- 编辑；
- 挂起；
- 继续。

## 修改文件

- `src/tunnel.rs`
- `src/app/tunnels.rs`
- `ui/tunnel_panel.slint`

## 执行

### 1. 删除 UI 上显式 Add 按钮

`ui/tunnel_panel.slint` 中移除：

```slint
SmallButton {
    text: @tr("Add");
    primary: true;
    clicked => { root.add-rule(); }
}
```

### 2. 增加右键菜单状态

```slint
in-out property <string> selected-rule-id: "";
in-out property <bool> menu-open: false;
callback menu-add();
callback menu-edit(string);
callback menu-delete(string);
callback menu-suspend(string);
callback menu-resume(string);
```

### 3. 添加/编辑弹窗字段

弹窗字段：

- 类型/方向；
- 源主机；
- 侦听端口；
- 目标主机；
- 目标端口；
- 确定；
- 取消。

如果当前后端只支持本地转发，类型先固定为 `local`，UI 文案显示“本地转发”。

### 4. 数据结构预留方向字段

`TunnelRule` 增加：

```rust
#[serde(default)]
pub direction: TunnelDirection,
```

新增：

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelDirection {
    Local,
    Remote,
    Dynamic,
}

impl Default for TunnelDirection {
    fn default() -> Self {
        TunnelDirection::Local
    }
}
```

本轮只实现 `Local` 的启动逻辑。

### 5. 挂起/继续

- 挂起：停止当前 tunnel handle，但不删除规则，状态改为 `Stopped`，`enabled` 可以保留 true 或新增 `suspended` 字段。
- 继续：按原规则重新启动。

建议增加字段：

```rust
#[serde(default)]
pub suspended: bool,
```

挂起时：`suspended = true`。继续时：`suspended = false`。

## 验收

- 隧道面板没有 Add 按钮。
- 右键空白区域可添加。
- 右键已有规则可编辑/删除/挂起/继续。
- 删除/编辑必须选中规则才可用。
- 挂起只对运行中规则可用。
- 继续只对挂起规则可用。
- 无连接会话时提示“请先连接一个会话”。

---

# step-025：侧边栏新增会话程序监测表

## 类型

新增功能。

## 目标

在侧边栏“服务器资源”下方新增进程监测表，列为：

```text
内存 | CPU | 命令
```

支持点击“内存”或“CPU”表头按占用从大到小排序。

## 修改文件

- `src/ssh.rs`
- `src/app/types.rs`
- `src/app/sidebar.rs`
- `ui/sidebar.slint`

## 执行

### 1. 扩展事件数据

在 `SessionEvent::ResourceStats` 中增加：

```rust
processes: Vec<RemoteProcess>,
```

新增结构：

```rust
#[derive(Clone, Debug, Default)]
pub struct RemoteProcess {
    pub mem_percent: f32,
    pub cpu_percent: f32,
    pub command: String,
}
```

### 2. 扩展远程监控命令

在 `MON_CMD` 中追加 ps 输出。建议在 `__DF__` 后或前增加标记：

```bash
echo __PS__;
ps -eo pmem,pcpu,comm --sort=-pmem 2>/dev/null | head -n 21;
```

完整命令要继续保留固定 PATH：

```rust
const MON_CMD: &[u8] = b"PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH; while :; do awk '/^cpu /{print}' /proc/stat; awk '/^(MemTotal|MemAvailable|SwapTotal|SwapFree):/{print}' /proc/meminfo; cat /proc/net/dev; echo __DF__; df -kP 2>/dev/null; echo __PS__; ps -eo pmem,pcpu,comm --sort=-pmem 2>/dev/null | head -n 21; echo __MSTICK__; sleep 2; done\n";
```

### 3. 解析 `__PS__`

在 `parse_monitor_block` 中增加状态：

```rust
let mut in_ps = false;
let mut processes = Vec::new();
```

遇到 `__PS__` 后解析：

```rust
fn parse_ps_line(line: &str) -> Option<RemoteProcess> {
    let mut parts = line.split_whitespace();
    let mem: f32 = parts.next()?.parse().ok()?;
    let cpu: f32 = parts.next()?.parse().ok()?;
    let command = parts.collect::<Vec<_>>().join(" ");
    if command.is_empty() || command == "COMMAND" {
        return None;
    }
    Some(RemoteProcess {
        mem_percent: mem,
        cpu_percent: cpu,
        command,
    })
}
```

限制最多 20 条。

### 4. UI 表头排序

`ui/sidebar.slint` 增加：

```slint
in property <[ProcessInfo]> processes;
in-out property <string> process-sort-key: "mem";
callback process-sort-changed(string);
```

`ProcessInfo`：

```slint
export struct ProcessInfo {
    mem: string,
    cpu: string,
    command: string,
}
```

点击表头：

```slint
TouchArea { clicked => { root.process-sort-changed("mem"); } }
TouchArea { clicked => { root.process-sort-changed("cpu"); } }
```

排序可在 Rust 中做，避免 Slint 复杂逻辑。

## 验收

- 连接 Linux SSH 后，侧边栏显示进程表。
- 默认按内存降序。
- 点击 CPU 表头后按 CPU 降序。
- 非 Linux 或 ps 不可用时不崩溃，只显示空表。
- 最多显示 20 条左右。

---

# step-026：侧边栏和底部窗格增加快速平移动画

## 类型

新增功能。

## 目标

侧边栏和底部窗格展开/收起有轻快动画，避免生硬切换。

## 修改文件

- `ui/app.slint`
- `ui/sidebar.slint`
- `ui/bottom_panel.slint`

## 执行

### 1. 侧边栏动画

当前：

```slint
Sidebar {
    visible: root.sidebar-visible;
    width: root.sidebar-visible ? 220px : 0px;
}
```

改为不要直接 `visible=false` 销毁布局，使用宽度动画：

```slint
Sidebar {
    width: root.sidebar-visible ? 220px : 0px;
    clip: true;
    animate width { duration: 140ms; easing: ease-out; }
}
```

如果 `clip` 不支持或导致编译问题，删除 `clip`，但保留 width animation。

### 2. 底部窗格动画

同理，把底部窗格高度改为：

```slint
height: root.bottom-panel-visible ? 180px : 0px;
animate height { duration: 140ms; easing: ease-out; }
```

不要在动画时销毁内部组件。

## 验收

- 侧边栏展开/收起有快速平移动画。
- 底部窗格展开/收起有快速动画。
- 终端区域随布局重排正常。
- 切换 tab 不丢失底部窗格状态。

---

# step-027：最终验证和整理

## 类型

收尾。

## 目标

确认本轮迁移和新增功能没有破坏主流程。

## 修改文件

按实际修复情况修改，不新增大功能。

## 验证清单

### 基础启动

```bash
cargo fmt
cargo check
cargo test
cargo run
```

### 上游回归

- 主题可切换或可跟随系统。
- 终端字体可设置。
- 终端字号可设置。
- 重启后字体/字号生效。
- SFTP 可上传文件夹。
- SFTP 可下载文件夹。
- SFTP 可递归删除文件夹。
- 粘贴多行命令正常。
- 单独按 Alt 不影响命令行。
- Wayland 下复制/粘贴不回退。

### dev bug

- debug 启动不重复输出 ICU4X ja 错误。
- 菜单栏在标签页上方。
- tooltip 不被遮挡；如果暂时禁用 tooltip，也不能显示被遮挡的 tooltip。
- 欢迎页点击文件/隧道/断开/重连均提示“请先连接一个会话”。
- Alacritty 普通页面可滚轮翻页。
- htop/vim/less 中滚轮仍正常。

### 文件传输窗口

- 应用内最多一个文件传输窗口。
- 同一会话不会重复创建远程 tab。
- 不同会话可新增多个远程 tab。
- 双击远程 tab 可重新连接。
- 底部传输记录能显示任务。
- 本地/远程右键菜单可用。
- 文件夹传输可用。
- 文件列表显示名称、大小、类型、修改时间、属性、所有者。

### 隧道

- Add 按钮已移除。
- 右键菜单可添加、编辑、删除、挂起、继续。
- 无连接时提示“请先连接一个会话”。
- 挂起不删除规则。
- 继续能恢复规则。

### 侧边栏

- 服务器资源下方显示进程表。
- 内存排序可用。
- CPU 排序可用。
- 侧边栏动画正常。
- 底部窗格动画正常。

## 最终提交建议

如果分阶段 commit，建议提交名：

```text
step-001: establish dev baseline
step-002: add upstream ui setting dependencies
step-003: add theme font and group config fields
step-004: wire theme and terminal font settings
step-005: add terminal font controls to slint ui
step-006: port recursive sftp folder operations
step-007: sanitize remote filenames for local writes
step-008: port terminal paste and alt-key fixes
step-009: harden remote resource monitor
step-010: hide shell integration prompt command echo
step-011: port session grouping and import export basics
step-012: normalize unsupported locales
step-013: move top action bar above tabs
step-014: fix tooltip layering
step-015: add active session guard
step-016: fix alacritty scrollback wheel
step-017: make transfer window singleton
step-018: add remote tabs to transfer window
step-019: reconnect transfer tabs by double click
step-020: show transfer records in transfer window
step-021: add transfer window context menus
step-022: reuse recursive sftp for folder transfer window
step-023: show extended file metadata
step-024: manage tunnels from context menu
step-025: add remote process monitor table
step-026: animate sidebar and bottom panel
step-027: final verification
```
