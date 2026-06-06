# 终端测试清单

手测时分别覆盖：

- `Legacy` fallback
- `Alacritty Experimental`

## 0. Alacritty 启动与冒烟

- [ ] `cargo run`
- [ ] `echo hello`
- [ ] `clear`
- [ ] 中文输出
- [ ] emoji 输出
- [ ] `PowerShell` 方向键历史 / `Home` / `End`
- [ ] `Insert` / `Delete` / `PageUp` / `PageDown`
- [ ] `F1-F12`
- [ ] `Ctrl+C` / `Ctrl+D` / `Alt+字母`
- [ ] 普通粘贴 / 多行粘贴 / bracketed paste
- [ ] `vim`
- [ ] `nano`
- [ ] `less`
- [ ] `htop`
- [ ] `tmux`
- [ ] PowerShell
- [ ] resize
- [ ] Alacritty 初始化失败时，新建会话回退到 Legacy 并出现 GUI 提示

## 1. 普通 shell

- [ ] `echo hello`
- [ ] `ls` / `dir`
- [ ] `clear`
- [ ] 长行自动换行
- [ ] 中文
- [ ] emoji

## 2. ANSI 样式

- [ ] 16 色
- [ ] 256 色
- [ ] truecolor
- [ ] bold
- [ ] underline
- [ ] inverse
- [ ] 前景色
- [ ] 背景色

## 3. TUI

- [ ] `vim`
- [ ] `nano`
- [ ] `less`
- [ ] `htop`
- [ ] `tmux`

## 4. PowerShell

- [ ] 普通命令
- [ ] 方向键历史
- [ ] `clear`
- [ ] 彩色输出
- [ ] 中文路径
- [ ] 长行输入

## 5. 输入

- [ ] 方向键
- [ ] application cursor mode（`vim` / `nano` / `tmux` 内方向键）
- [ ] `Home` / `End`
- [ ] `PageUp` / `PageDown`
- [ ] `Insert` / `Delete`
- [ ] `F1-F12`
- [ ] `Ctrl+C`
- [ ] `Ctrl+D`
- [ ] `Ctrl+A-Z`
- [ ] `Alt+字母`

## 6. 鼠标

- [ ] `htop` 点击
- [ ] `htop` 滚轮
- [ ] `vim` `mouse=a`
- [ ] `tmux` `mouse on`
- [ ] `less` 滚轮

## 7. Resize

- [ ] 普通 shell resize
- [ ] `vim` resize
- [ ] `htop` resize
- [ ] `tmux` resize

## 8. Paste

- [ ] 普通粘贴
- [ ] 多行粘贴
- [ ] bracketed paste
- [ ] `vim` / `nano` / `tmux` 内 bracketed paste
