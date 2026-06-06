# meatshell

[简体中文](./README.md) | **English**

A lightweight, low-memory SSH / terminal client inspired by FinalShell, but
written entirely in **Rust + [Slint](https://slint.dev)**. The goal is to keep
FinalShell's core experience (resource-monitor sidebar, session management,
tabbed terminals) while cutting memory use from the 400 MB+ of a JVM app down to
the tens-of-MB range of a native binary.

## Screenshots

<p align="center">
  <img src="docs/screenshots/01-welcome-en.png" alt="Welcome / session management" width="800"><br>
  <em>Welcome page: session management + local resource monitor sidebar</em>
</p>

<p align="center">
  <img src="docs/screenshots/02-terminal-btop.png" alt="Terminal + SFTP" width="800"><br>
  <em>Tabbed terminal (full-screen btop) + SFTP file browser + remote resource monitoring</em>
</p>

## Download & install

Every `v*` tag triggers a GitHub Actions build that produces native binaries for
**Windows / Linux / macOS**, published on the
[Releases](https://github.com/jeff141/meatshell/releases) page.

### Windows

Download `meatshell-*-windows-x86_64.zip`, unzip, and run `meatshell.exe`.

### Linux

```bash
tar -xzf meatshell-*-linux-x86_64.tar.gz
cd meatshell-*-linux-x86_64
./meatshell                                  # run it directly
# Optional: install the app icon + launcher entry (shows the icon in the dock /
# app list — no argument needed, it finds the binary next to the script)
chmod +x install-linux.sh && ./install-linux.sh
```

> Requires glibc ≥ 2.35 (Ubuntu 22.04+ / Debian 12+). On Wayland you may need to
> log out/in once after installing the icon.

### macOS

```bash
tar -xzf meatshell-*-macos-*.tar.gz          # aarch64 = Apple Silicon, x86_64 = Intel
xattr -dr com.apple.quarantine meatshell     # clear the "unsigned app" Gatekeeper flag
./meatshell
```

> To build from source, see [Running](#running) below.

## Roadmap

### v0.1 (current)

- [x] FinalShell-style dark theme UI
- [x] Local system monitor sidebar (CPU / memory / swap / network throughput, 1 Hz)
- [x] Tabs (welcome page + multiple terminal sessions)
- [x] Session management: create / edit / delete, persisted to local JSON
  - Config location: `%APPDATA%/meatshell/sessions.json` (Windows)
    / `~/.config/meatshell/sessions.json` (Linux)
    / `~/Library/Application Support/meatshell/sessions.json` (macOS)
- [x] SSH connection scaffold (`russh`, pure Rust, password + private key)
- [x] Line-buffered terminal view (type a line → Enter to send)

### v0.2

- [ ] Full VT/ANSI terminal emulation (`alacritty_terminal` experimental engine is opt-in; mouse/TUI work remains for a later phase)
- [ ] Remote host resource monitoring (run a remote collector script, like FinalShell)
- [x] SFTP file browser + drag-and-drop upload/download
- [x] Top toolbar shell: sidebar, bottom panel, disconnect, reconnect, transfer entry
- [x] Bottom Files / Tunnels tab shell (Files continues to use the SFTP panel)
- [x] Basic SGR mouse reporting for the experimental alacritty engine (left click, release, wheel)
- [x] First independent file-transfer window (local/remote split view, basic upload/download)
- [x] First Local Forward tunnel support (session-linked rules, auto-start when enabled, separate `tunnels.json`)
- [ ] Known-hosts (`known_hosts`) verification
- [ ] Store session passwords in the OS keychain

### v0.3+

- [ ] Split panes for tabbed terminals
- [ ] Session groups / folders
- [ ] Theme switching (light / follow system)
- [ ] Command history & snippet management

## Tech stack

| Module        | Choice                                                            |
| ------------- | ----------------------------------------------------------------- |
| UI            | [Slint](https://slint.dev) (compiled pure Rust, no GC)            |
| Async runtime | [`tokio`](https://tokio.rs)                                       |
| SSH protocol  | [`russh`](https://crates.io/crates/russh) (no libssh dependency)  |
| Terminal parser | Legacy `vt100` by default; experimental [`alacritty_terminal`](https://crates.io/crates/alacritty_terminal) |
| Tunnels       | `russh` direct-tcpip + `tokio` TCP forwarding                    |
| System metrics| [`sysinfo`](https://crates.io/crates/sysinfo)                     |
| Serialization | `serde` + `serde_json`                                            |
| Logging       | `tracing` + `tracing-subscriber`                                  |

## Running

```bash
cargo run --release
```

The experimental alacritty terminal engine is disabled by default. Set an
environment variable before launch to try it:

```bash
MEATSHELL_TERMINAL_ENGINE=alacritty cargo run --release
```

PowerShell:

```powershell
$env:MEATSHELL_TERMINAL_ENGINE = "alacritty"; cargo run --release
```

On first launch an empty session store is created. Click **"＋ New Session"** in
the top-right to add your first server.

## Common Features

- Top toolbar: toggle the resource sidebar, toggle the bottom panel, disconnect the current tab, reconnect the current tab, and open the independent file-transfer window.
- File-transfer window: from a connected terminal tab, click the transfer toolbar button. The left side browses local files, the right side browses the current remote session, with basic upload/download support.
- Tunnels: the bottom **Tunnels** tab supports Local Forward rules. Add a rule, fill `local host:port -> remote host:port`, save it, then enable it. Enabled rules for the session start after the terminal SSH connection succeeds and stop when the tab disconnects or closes.
- Terminal engine: legacy `vt100` is the default; set `MEATSHELL_TERMINAL_ENGINE=alacritty` before launch to try the experimental alacritty engine.

## Configuration Files

`sessions.json` stores sessions, language, and the download directory.
`tunnels.json` stores tunnel rules separately, outside the session structure.

Default configuration directories:

- Windows: `%APPDATA%\meatshell\meatshell\config`
- Linux: `~/.config/meatshell`
- macOS: `~/Library/Application Support/dev.meatshell.meatshell`

The terminal engine mode is currently controlled only by the launch-time
environment variable. Sidebar and bottom-panel default visibility are not
persisted yet.

## Project layout

```
meatshell/
├── Cargo.toml
├── build.rs                 # Slint compiler entry point
├── ui/
│   ├── app.slint            # top-level window
│   ├── theme.slint          # design tokens
│   ├── widgets.slint        # reusable buttons / inputs / sparkline
│   ├── sidebar.slint        # left-hand system monitor panel
│   ├── tabs.slint           # top tab bar
│   ├── top_action_bar.slint # toolbar below the tab bar
│   ├── bottom_panel.slint   # bottom Files / Tunnels tab shell
│   ├── tunnel_panel.slint   # Local Forward tunnel rules panel
│   ├── transfer_window.slint # independent file-transfer window
│   ├── local_file_panel.slint # transfer local panel
│   ├── remote_file_panel.slint # transfer remote panel
│   ├── welcome.slint        # welcome page / quick connect
│   ├── session_dialog.slint # new / edit session dialog
│   └── terminal_view.slint  # terminal view (v0.1 line-buffered)
└── src/
    ├── main.rs
    ├── app.rs               # UI ↔ backend bridge
    ├── connection.rs        # connection runtime, disconnect, reconnect entry
    ├── config.rs            # session JSON persistence
    ├── file_transfer.rs     # transfer-window local directory helper
    ├── tunnel.rs            # Local Forward tunnel rules and worker tasks
    ├── terminal_alacritty.rs # experimental alacritty terminal engine
    ├── terminal_engine.rs   # terminal engine trait
    ├── terminal_types.rs    # terminal render data types
    ├── system.rs            # CPU / memory / network sampling
    └── ssh.rs               # SSH session worker
```

## Development notes

- Slint widgets use a strict layout DSL; after editing a `.slint` file,
  `cargo check` is the fastest feedback loop.
- The application event loop is single-threaded (required by Slint); all
  cross-thread UI updates go through `slint::invoke_from_event_loop` callbacks.
- `check_server_key` currently accepts any server key (like
  `StrictHostKeyChecking=no`); wire up known-hosts verification before
  production use.

## License

Dual-licensed under MIT OR Apache-2.0.
