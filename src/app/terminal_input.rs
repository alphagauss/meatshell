use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::terminal::alacritty::AlacrittyTerminalEngine;
use crate::terminal::engine::TerminalEngine;
use crate::terminal::legacy::{build_row, CsiState, MAX_HISTORY};
use crate::terminal::types::Line;

use super::models::set_terminal_row;
use super::terminal_render::{compute_find_matches, extract_selection, rebuild_tab_display};
use super::types::{ConnectionStore, TermBuffers};
use super::{AppWindow, TermMatch, TermSpan};

// ---------------------------------------------------------------------------
// Raw keystroke forwarding and PTY resize
// ---------------------------------------------------------------------------

pub(super) fn wire_key_input(
    window: &AppWindow,
    connections: ConnectionStore,
    bufs: TermBuffers,
    last_term_size: Arc<Mutex<(u32, u32)>>,
) {
    // Forward each keystroke as raw bytes to the SSH PTY. The server's bash /
    // readline handles echo, history (↑↓), Tab completion, Ctrl+C, etc.
    {
        let connections = connections.clone();
        let bufs = bufs.clone();
        // Shared timestamp: the last time the Shift key alone was pressed
        // (key="", shift=true).  Used by the time-based Backspace filter below.
        let last_shift_time: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));
        window.on_send_key(move |tab_id: SharedString, key: SharedString, ctrl: bool, alt: bool, shift: bool| {
            // Check whether the remote PTY switched to application cursor mode
            // (DECCKM, set by nano/vim via \x1b[?1h). In that mode the terminal
            // must send \x1bOA/B/C/D instead of \x1b[A/B/C/D.
            let app_cursor = {
                let mut map = bufs.lock().unwrap();
                match map.get_mut(tab_id.as_str()) {
                    Some(b) => {
                        // Typing snaps the view back to the live bottom so the
                        // user always sees what they're entering.
                        b.view_offset = 0;
                        TerminalEngine::application_cursor(b)
                    }
                    None => false,
                }
            };
            tracing::debug!(
                "send_key tab={} key={:?} ctrl={} alt={} shift={} app_cursor={}",
                tab_id, key.as_str(), ctrl, alt, shift, app_cursor
            );

            // ── Shift / Backspace 诊断日志 (info 级, 无需 RUST_LOG=debug) ─────
            // 每个 Shift 相关事件都打印 key 的 Unicode 码位，方便对比
            // 左Shift / 右Shift 是否产生不同的 key 字符串。
            if shift || key.as_str() == "\u{0008}" {
                let codepoints: Vec<String> = if key.as_str().is_empty() {
                    vec!["(empty)".to_string()]
                } else {
                    key.as_str().chars().map(|c| format!("U+{:04X}", c as u32)).collect()
                };
                let elapsed_ms = last_shift_time
                    .lock()
                    .unwrap()
                    .map(|t| format!("{}ms ago", t.elapsed().as_millis()))
                    .unwrap_or_else(|| "never".to_string());
                tracing::info!(
                    "[KEY_DIAG] key={} shift={} ctrl={} alt={} | last_shift={}",
                    codepoints.join(","), shift, ctrl, alt, elapsed_ms
                );
            }

            // ── Track lone-Shift presses for the time-based Backspace filter ──
            // Slint sends key="" (empty string) when a bare modifier key (Shift,
            // Ctrl, Alt) is pressed.  We record the timestamp whenever Shift
            // alone fires so the filter below can catch IME-injected Backspace
            // events even if they arrive with shift=false.
            if key.as_str().is_empty() && shift && !ctrl && !alt {
                *last_shift_time.lock().unwrap() = Some(std::time::Instant::now());
                tracing::info!("[KEY_DIAG] lone-Shift recorded → timestamp saved");
            }

            // ── 拦截百度拼音注入的 Shift 标记字符（核心修复）────────────────────
            // 诊断日志证实，百度拼音通过 WH_KEYBOARD_LL 钩子，在 Shift 键按下时
            // 向消息队列注入一个 C0 控制字符，而非空字符串：
            //
            //   左 Shift → U+0015 (Ctrl+U / NAK), shift=true, ctrl=false
            //   右 Shift → U+0010 (Ctrl+P / DLE), shift=true, ctrl=false
            //              紧接着注入: U+0008 (Backspace), shift=false
            //
            // 这些字符绝对不应送入 PTY：
            //   0x15 (Ctrl+U) 在 bash/vim 中会清空当前输入行 → "左Shift替换字符"
            //   0x10 (Ctrl+P) 在 vim 中翻历史/触发补全     → "右Shift乱跳"
            //   0x08 (Backspace) 紧随其后                   → "右Shift删除字符"
            //
            // 合法独立 C0 键（Backspace=0x08, Tab=0x09, LF=0x0A, CR=0x0D,
            // ESC=0x1B）不受此过滤影响，由下方代码单独处理。
            //
            // 检测到 IME Shift 标记后，记录时间戳，让 Layer 2 在 1500ms 内
            // 拦截随后可能到来的 Backspace（右Shift场景，日志显示间隔约 914ms）。
            if !ctrl && !alt {
                if let Some(c) = key.as_str().chars().next() {
                    let cp = c as u32;
                    let is_standalone = matches!(cp, 0x08 | 0x09 | 0x0A | 0x0D | 0x1B);
                    if key.as_str().chars().count() == 1
                        && (0x01..=0x1f).contains(&cp)
                        && !is_standalone
                    {
                        *last_shift_time.lock().unwrap() = Some(std::time::Instant::now());
                        tracing::info!(
                            "[KEY_DIAG] DROPPED IME C0 marker U+{:04X} (shift={}) → timestamp saved",
                            cp, shift
                        );
                        return;
                    }
                }
            }

            // ── Windows: filter synthetic Ctrl+char injections ──────────────
            // Some keyboards / IME drivers (e.g. Aula F99 + Baidu Pinyin)
            // inject a synthetic WM_CHAR 0x11 (Ctrl+Q) when Left Ctrl is
            // briefly tapped, WITHOUT sending a WM_KEYDOWN VK_Q beforehand.
            //
            // FinalShell avoids this because it builds Ctrl+letter from
            // WM_KEYDOWN (virtual-key codes).  Slint uses WM_CHAR, so it
            // sees the injected byte and forwards it straight to us.
            //
            // Fix: for C0 control chars (Ctrl+A…Ctrl+Z, i.e. 0x01–0x1A),
            // use GetKeyState — which returns the key state *as of the last
            // processed message*, not the live hardware state — to verify
            // the corresponding letter VK was actually queued as a keydown
            // before this WM_CHAR arrived.  If Q was never keyed down,
            // GetKeyState(VK_Q) = 0 → the event is synthetic → drop it.
            #[cfg(windows)]
            if ctrl {
                if let Some(ch) = key.as_str().chars().next() {
                    let cp = ch as u32;
                    // Always let Enter / Tab pass through regardless of Ctrl
                    // state.  These C0 codes (0x09 Tab, 0x0a LF, 0x0d CR) are
                    // "double-duty" keys: pressing Enter while Ctrl is still
                    // physically held (e.g. just after Ctrl+O in nano) generates
                    // Ctrl+M (0x0d) with ctrl=true — but GetKeyState(VK_M) is 0
                    // because the user never pressed M.  Without this exemption
                    // the filter would silently drop the Enter, making it
                    // impossible to confirm nano's "File Name to Write:" prompt.
                    let always_pass = matches!(cp, 0x09 | 0x0a | 0x0d);
                    if !always_pass
                        && key.as_str().chars().count() == 1
                        && (0x01..=0x1a).contains(&cp)
                        && !c0_letter_key_down(cp)
                    {
                        tracing::debug!(
                            "send_key: dropped synthetic Ctrl+{} \
                             (VK_{:02X} not down per GetKeyState)",
                            (0x40u8 + cp as u8) as char,
                            cp + 0x40
                        );
                        return;
                    }
                }
            }

            // ── Filter synthetic Backspace injected by Chinese IME ────────────
            // Baidu Pinyin (and similar Chinese IMEs) hooks the keyboard at the
            // driver level via WH_KEYBOARD_LL, below Win32's ImmDisableIME.
            // When the user presses Shift to switch from Chinese to English mode
            // while a pinyin syllable is in-flight, the IME:
            //   1. Cancels the composition (discards the syllable).
            //   2. Posts WM_KEYDOWN VK_BACK + WM_CHAR 0x08 to erase whatever
            //      character it had already forwarded to the app.
            //
            // Three-layer defence:
            //
            //   Layer 1 – shift=true guard.
            //     The synthetic Backspace arrives during Shift keydown, so
            //     GetKeyState(VK_SHIFT) is still "down" → Slint reports shift=true.
            //     Drop any Backspace (0x08) arriving while Shift is flagged.
            //
            //   Layer 2 – time-based guard.
            //     Baidu Pinyin posts WM_CHAR 0x08 asynchronously, so by the time
            //     the message is dequeued Shift may already read as "up"
            //     → shift=false defeats Layer 1.
            //     Mitigation: we recorded the timestamp when the Shift key alone
            //     was pressed (key="", shift=true) a few lines above.  Drop any
            //     Backspace arriving within 200 ms of that moment.
            //
            //   Layer 3 – GetKeyState guard (belt-and-suspenders).
            //     If VK_BACK is not actually "down" (i.e. no real WM_KEYDOWN
            //     VK_BACK was ever queued), the Backspace must be synthetic.
            if key.as_str() == "\u{0008}" && !ctrl && !alt {
                // Layer 1
                if shift {
                    tracing::info!("[KEY_DIAG] Backspace DROPPED by layer-1 (shift=true)");
                    return;
                }
                // Layer 2 — 时间窗口 1500ms
                // 日志显示百度拼音注入 U+0010(右Shift标记) 到 Backspace 之间
                // 间隔约 914ms，因此窗口设为 1500ms 以覆盖该场景。
                let (shift_just_pressed, elapsed_ms) = {
                    let guard = last_shift_time.lock().unwrap();
                    match *guard {
                        Some(t) => {
                            let ms = t.elapsed().as_millis();
                            (ms < 1500, ms)
                        }
                        None => (false, 0),
                    }
                };
                if shift_just_pressed {
                    tracing::info!(
                        "[KEY_DIAG] Backspace DROPPED by layer-2 ({}ms after IME Shift marker)",
                        elapsed_ms
                    );
                    return;
                }
                // Layer 3
                #[cfg(windows)]
                if !is_vk_back_down() {
                    tracing::info!("[KEY_DIAG] Backspace DROPPED by layer-3 (VK_BACK not down)");
                    return;
                }
                tracing::info!("[KEY_DIAG] Backspace PASSED all filters → sent to PTY");
            }

            let bytes = key_to_pty_bytes(key.as_str(), ctrl, alt, app_cursor);
            // Log only the length — never the keystroke bytes, which can be
            // password characters (#15).
            tracing::debug!(
                "send_key len={} connection_known={}",
                bytes.len(),
                connections
                    .lock()
                    .unwrap()
                    .session(tab_id.as_str())
                    .is_some(),
            );
            if !bytes.is_empty() {
                connections.lock().unwrap().send_raw(tab_id.as_str(), bytes);
            }
        });
    }

    // Propagate PTY resize to the SSH worker and vt100 parser. Pixel
    // dimensions come from Slint; we approximate col/row counts using
    // Consolas 13px metrics.
    //
    // terminal_view.slint now passes the FocusScope height (not the full
    // TerminalView height), so the SFTP panel is already excluded.
    // Layout breakdown for the FocusScope:
    //   16 px  – bottom strip (TouchArea for focus-regain)
    //    8 px  – y-offset of the output Text element inside the Flickable
    // = 24 px  total vertical chrome within FocusScope
    //
    // Consolas 13 px renders at ≈ 8 px wide × 16 px tall per cell.
    {
        let connections = connections.clone();
        let bufs_resize = bufs.clone(); // keep bufs alive for the copy handler below
                                        // The Slint side now measures the real Consolas cell size (via a hidden
                                        // probe Text) and passes whole column/row counts directly, so there is
                                        // no pixel→cell guesswork here.  This keeps full-screen programs like
                                        // nano from over-counting rows and clipping their bottom shortcut bar.
        window.on_terminal_resize(move |tab_id: SharedString, cols_f: f32, rows_f: f32| {
            let cols = (cols_f as u32).max(10);
            let rows = (rows_f as u32).max(5);
            tracing::debug!("terminal_resize tab={} cols={} rows={}", tab_id, cols, rows);
            // Keep the shared size up-to-date so future connections start
            // with the correct PTY dimensions.
            *last_term_size.lock().unwrap() = (cols, rows);
            connections
                .lock()
                .unwrap()
                .resize(tab_id.as_str(), cols, rows);
            if let Some(buf) = bufs_resize.lock().unwrap().get_mut(tab_id.as_str()) {
                let (old_rows, old_cols) = buf.parser.screen().size();
                let new_rows = rows as u16;
                // Shrinking the grid (e.g. dragging the SFTP panel up) makes
                // vt100's set_size truncate rows from the BOTTOM — silently
                // dropping the most recent output + prompt (#18).  Before
                // shrinking, save the top rows that should scroll off into our
                // scrollback, then scroll the screen up so vt100 keeps the
                // BOTTOM rows visible (correct terminal semantics).  Skipped on
                // the alternate screen (vim/btop own their full-screen buffer).
                if new_rows < old_rows && !buf.parser.screen().alternate_screen() {
                    let delta = old_rows - new_rows;
                    let saved: Vec<Line> = {
                        let s = buf.parser.screen();
                        (0..delta).map(|r| build_row(s, r, old_cols)).collect()
                    };
                    for line in saved {
                        buf.history.push(line);
                    }
                    if buf.history.len() > MAX_HISTORY {
                        let drop = buf.history.len() - MAX_HISTORY;
                        buf.history.drain(0..drop);
                    }
                    buf.parser.process(format!("\x1b[{delta}S").as_bytes());
                }
                TerminalEngine::resize(buf, new_rows as usize, cols as usize);
                // The pre/post-resize screens differ in size+content; drop the
                // scroll-detection snapshot so the next output isn't mis-read as
                // a scroll (which would double-capture lines).
                buf.prev.clear();
            }
        });
    }

    // Terminal mouse reporting: only active when the terminal engine has seen
    // an app enable SGR mouse mode. Plain terminal text selection stays local.
    {
        let connections = connections.clone();
        let bufs_mouse = bufs.clone();
        window.on_terminal_mouse(
            move |tab_id: SharedString, button: i32, pressed: bool, row: i32, col: i32| {
                let tid = tab_id.to_string();
                let seq = {
                    let map = bufs_mouse.lock().unwrap();
                    let Some(buf) = map.get(&tid) else { return };
                    if !buf.mouse_reporting() {
                        return;
                    }
                    let (rows, cols) = buf.parser.screen().size();
                    sgr_mouse_sequence(button, pressed, row, col, rows, cols)
                };
                connections.lock().unwrap().send_raw(&tid, seq);
            },
        );
    }

    // Ctrl+Shift+C: copy current terminal screen to clipboard.
    {
        let bufs = bufs.clone();
        window.on_copy_terminal_text(move |tab_id: SharedString| {
            let text = {
                let map = bufs.lock().unwrap();
                match map.get(tab_id.as_str()) {
                    Some(buf) => {
                        // Copy the drag-selection when there is one, else the
                        // whole displayed screen.
                        match buf.sel {
                            Some((sr, sc, er, ec)) if (sr, sc) != (er, ec) => {
                                let displayed_text = buf.displayed_text();
                                extract_selection(&displayed_text, sr, sc, er, ec)
                            }
                            _ => buf.displayed_text().join("\n"),
                        }
                    }
                    None => String::new(),
                }
            };
            // Run the clipboard write on a dedicated OS thread.  arboard's
            // Windows backend opens the clipboard and pumps Win32 messages;
            // doing that on the Slint/winit event-loop thread re-enters the
            // message loop and dead-locks the whole UI.
            std::thread::spawn(move || {
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                    Ok(()) => tracing::debug!("copy_terminal: clipboard updated"),
                    Err(e) => tracing::warn!("copy_terminal: clipboard error: {}", e),
                }
            });
        });
    }

    // Middle-click / Ctrl+Shift+V: paste clipboard text into PTY.
    {
        let connections = connections.clone();
        let bufs = bufs.clone();
        window.on_paste_from_clipboard(move |tab_id: SharedString| {
            let connections = connections.clone();
            let bufs = bufs.clone();
            let tid = tab_id.to_string();
            std::thread::spawn(move || {
                match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) => {
                        let payload = {
                            let map = bufs.lock().unwrap();
                            match map.get(&tid) {
                                Some(buf) if TerminalEngine::bracketed_paste(buf) => {
                                    format!("\x1b[200~{text}\x1b[201~").into_bytes()
                                }
                                _ => text.into_bytes(),
                            }
                        };
                        connections.lock().unwrap().send_raw(&tid, payload);
                    }
                    Err(e) => tracing::warn!("paste_from_clipboard: clipboard error: {}", e),
                }
            });
        });
    }

    // Context menu → 清空缓存: reset the local vt100 buffer (drops scrollback),
    // wipe the displayed screen, then nudge the remote to redraw a fresh prompt.
    {
        let bufs_clear = bufs.clone();
        let connections_clear = connections.clone();
        let weak = window.as_weak();
        window.on_clear_terminal(move |tab_id: SharedString| {
            let tid = tab_id.to_string();
            if let Some(buf) = bufs_clear.lock().unwrap().get_mut(&tid) {
                let (rows, cols) = buf.parser.screen().size();
                buf.parser = vt100::Parser::new(rows, cols, 5000);
                if buf.alacritty.is_some() {
                    buf.alacritty = Some(AlacrittyTerminalEngine::new(rows, cols));
                }
                buf.find_query.clear();
                buf.history = Vec::new(); // recycle the session scrollback
                buf.prev = Vec::new();
                buf.view_offset = 0;
                buf.sel = None;
                buf.displayed_text.replace(Vec::new());
                buf.csi_state = CsiState::Normal;
            }
            if let Some(win) = weak.upgrade() {
                set_terminal_row(&win, &tid, |row| {
                    row.spans = ModelRc::from(Rc::new(VecModel::<TermSpan>::default()));
                    row.find_matches = ModelRc::from(Rc::new(VecModel::<TermMatch>::default()));
                    row.selection = ModelRc::from(Rc::new(VecModel::<TermMatch>::default()));
                    row.cursor_row = 0;
                    row.cursor_col = 0;
                    row.rows_used = 0;
                    row.mouse_reporting = false;
                });
            }
            connections_clear.lock().unwrap().send_raw(&tid, vec![0x0c]); // Ctrl+L → shell clears + redraws prompt
        });
    }

    // Context menu → 查找: store the query and recompute highlight rectangles.
    {
        let bufs_find = bufs.clone();
        let weak = window.as_weak();
        window.on_find_query_changed(move |tab_id: SharedString, query: SharedString| {
            let tid = tab_id.to_string();
            let q = query.to_string();
            let matches = {
                let mut map = bufs_find.lock().unwrap();
                if let Some(buf) = map.get_mut(&tid) {
                    buf.find_query = q.clone();
                    let displayed_text = buf.displayed_text();
                    compute_find_matches(&displayed_text, &q)
                } else {
                    Vec::new()
                }
            };
            if let Some(win) = weak.upgrade() {
                let model = ModelRc::from(Rc::new(VecModel::from(matches)));
                set_terminal_row(&win, &tid, |row| {
                    row.find_matches = model.clone();
                });
            }
        });
    }

    // Mouse-wheel → scroll the scrollback history.
    {
        let bufs_scroll = bufs.clone();
        let weak = window.as_weak();
        window.on_terminal_scroll(move |tab_id: SharedString, delta: i32| {
            let tid = tab_id.to_string();
            {
                let mut map = bufs_scroll.lock().unwrap();
                let Some(buf) = map.get_mut(&tid) else { return };
                // Scroll within our own session scrollback (history lines above
                // the live screen).  Offset 0 = live bottom.
                let max_off = buf.history.len() as i64;
                let cur = buf.view_offset as i64;
                buf.view_offset = (cur + delta as i64).clamp(0, max_off) as usize;
            }
            if let Some(win) = weak.upgrade() {
                rebuild_tab_display(&win, &bufs_scroll, &tid);
            }
        });
    }

    // Drag-selection lifecycle.
    {
        let bufs_sel = bufs.clone();
        let weak = window.as_weak();
        window.on_term_select_start(move |tab_id: SharedString, row: i32, col: i32| {
            let tid = tab_id.to_string();
            {
                let mut map = bufs_sel.lock().unwrap();
                let Some(buf) = map.get_mut(&tid) else { return };
                let (rows, cols) = buf.parser.screen().size();
                let r = row.clamp(0, rows.saturating_sub(1) as i32) as u16;
                let c = col.clamp(0, cols.saturating_sub(1) as i32) as u16;
                buf.sel = Some((r, c, r, c));
            }
            if let Some(win) = weak.upgrade() {
                rebuild_tab_display(&win, &bufs_sel, &tid);
            }
        });
    }
    {
        let bufs_sel = bufs.clone();
        let weak = window.as_weak();
        window.on_term_select_update(move |tab_id: SharedString, row: i32, col: i32| {
            let tid = tab_id.to_string();
            {
                let mut map = bufs_sel.lock().unwrap();
                let Some(buf) = map.get_mut(&tid) else { return };
                let (rows, cols) = buf.parser.screen().size();
                let r = row.clamp(0, rows.saturating_sub(1) as i32) as u16;
                let c = col.clamp(0, cols.saturating_sub(1) as i32) as u16;
                if let Some((sr, sc, _, _)) = buf.sel {
                    buf.sel = Some((sr, sc, r, c));
                }
            }
            if let Some(win) = weak.upgrade() {
                rebuild_tab_display(&win, &bufs_sel, &tid);
            }
        });
    }
    {
        let bufs_sel = bufs.clone();
        let weak = window.as_weak();
        window.on_term_select_end(move |tab_id: SharedString| {
            let tid = tab_id.to_string();
            // Extract the selected text; a zero-area selection (a plain click)
            // is cleared instead of copied.
            let text = {
                let mut map = bufs_sel.lock().unwrap();
                let Some(buf) = map.get_mut(&tid) else { return };
                match buf.sel {
                    Some((sr, sc, er, ec)) if (sr, sc) != (er, ec) => {
                        let displayed_text = buf.displayed_text();
                        Some(extract_selection(&displayed_text, sr, sc, er, ec))
                    }
                    _ => {
                        buf.sel = None; // treat as click → clear selection
                        None
                    }
                }
            };
            match text {
                Some(t) if !t.is_empty() => {
                    // Auto-copy on release (select-to-copy, PuTTY style).
                    std::thread::spawn(move || {
                        let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(t));
                    });
                }
                _ => {}
            }
            if let Some(win) = weak.upgrade() {
                rebuild_tab_display(&win, &bufs_sel, &tid);
            }
        });
    }
    // Auto-scroll while drag-selecting past the visible top/bottom edge.  We
    // move the scrollback view by a couple of lines per tick and shift the
    // selection anchor by the same amount so it stays pinned to its content
    // while the end is parked at the edge row.
    {
        let bufs_sel = bufs.clone();
        let weak = window.as_weak();
        window.on_term_select_autoscroll(move |tab_id: SharedString, dir: i32| {
            let tid = tab_id.to_string();
            {
                let mut map = bufs_sel.lock().unwrap();
                let Some(buf) = map.get_mut(&tid) else { return };
                // No scrollback on the alternate screen (vim/btop own the view).
                if buf.parser.screen().alternate_screen() {
                    return;
                }
                let rows = buf.parser.screen().size().0;
                let last = rows.saturating_sub(1);
                let max_off = buf.history.len();
                let step = 2usize;
                let Some((sr, sc, _er, ec)) = buf.sel else {
                    return;
                };
                if dir < 0 {
                    // Mouse above the top → reveal older lines.
                    let new_off = (buf.view_offset + step).min(max_off);
                    let delta = new_off - buf.view_offset;
                    if delta == 0 {
                        return; // already at the oldest line
                    }
                    buf.view_offset = new_off;
                    let nsr = ((sr as usize) + delta).min(last as usize) as u16;
                    buf.sel = Some((nsr, sc, 0, ec));
                } else if dir > 0 {
                    // Mouse below the bottom → move toward the live tail.
                    let new_off = buf.view_offset.saturating_sub(step);
                    let delta = buf.view_offset - new_off;
                    if delta == 0 {
                        return; // already at the live bottom
                    }
                    buf.view_offset = new_off;
                    let nsr = (sr as i32 - delta as i32).max(0) as u16;
                    buf.sel = Some((nsr, sc, last, ec));
                }
            }
            if let Some(win) = weak.upgrade() {
                rebuild_tab_display(&win, &bufs_sel, &tid);
            }
        });
    }
}

/// Convert a Slint `KeyEvent.text` + modifier flags into the byte sequence
/// that the remote PTY expects.
///
/// Slint uses Unicode Private Use Area (`\u{F700}`…) for special keys.
/// Regular printable characters and C0 control characters are passed as-is.
///
/// `app_cursor` mirrors the remote terminal's DECCKM mode (`\x1b[?1h/l`):
/// when true the four arrow keys must use SS3 sequences (`\x1bOA`…) instead
/// of the default CSI sequences (`\x1b[A`…).  Full-screen apps like nano and
/// vim set this mode on startup.
fn key_to_pty_bytes(key: &str, ctrl: bool, alt: bool, app_cursor: bool) -> Vec<u8> {
    // --- Special keys (Slint PUA code points) ------------------------------
    // Arrow keys: respect DECCKM application-cursor mode.
    let special: Option<&[u8]> = match key {
        "\u{F700}" => Some(if app_cursor { b"\x1bOA" } else { b"\x1b[A" }), // Up
        "\u{F701}" => Some(if app_cursor { b"\x1bOB" } else { b"\x1b[B" }), // Down
        "\u{F702}" => Some(if app_cursor { b"\x1bOD" } else { b"\x1b[D" }), // Left
        "\u{F703}" => Some(if app_cursor { b"\x1bOC" } else { b"\x1b[C" }), // Right
        "\u{F729}" => Some(b"\x1b[H"),                                      // Home
        "\u{F72B}" => Some(b"\x1b[F"),                                      // End
        "\u{F72C}" => Some(b"\x1b[5~"),                                     // PageUp
        "\u{F72D}" => Some(b"\x1b[6~"),                                     // PageDown
        "\u{F728}" => Some(b"\x1b[3~"),                                     // Delete (forward)
        "\u{F704}" => Some(b"\x1bOP"),                                      // F1
        "\u{F705}" => Some(b"\x1bOQ"),                                      // F2
        "\u{F706}" => Some(b"\x1bOR"),                                      // F3
        "\u{F707}" => Some(b"\x1bOS"),                                      // F4
        "\u{F708}" => Some(b"\x1b[15~"),                                    // F5
        "\u{F709}" => Some(b"\x1b[17~"),                                    // F6
        "\u{F70A}" => Some(b"\x1b[18~"),                                    // F7
        "\u{F70B}" => Some(b"\x1b[19~"),                                    // F8
        "\u{F70C}" => Some(b"\x1b[20~"),                                    // F9
        "\u{F70D}" => Some(b"\x1b[21~"),                                    // F10
        "\u{F70E}" => Some(b"\x1b[23~"),                                    // F11
        "\u{F70F}" => Some(b"\x1b[24~"),                                    // F12
        _ => None,
    };
    if let Some(seq) = special {
        return seq.to_vec();
    }

    // Slint sometimes sends `\u{0008}` for Backspace; terminals expect DEL.
    if key == "\u{0008}" {
        return vec![0x7f];
    }

    // Slint encodes Key::Return as "\n" (U+000A, LF).  Every real terminal
    // emulator (xterm, WezTerm, PuTTY …) sends 0x0D (CR) for Enter because
    // that is what a physical keyboard generates over a serial line.  bash/
    // readline happens to accept LF too, but ncurses apps in raw mode (nano,
    // vim command-line, passwd prompts …) strictly require CR to confirm input.
    // Ctrl+J (ctrl=true, "\n") intentionally stays 0x0A — it is a distinct
    // control character in some applications.
    if key == "\n" && !ctrl && !alt {
        return vec![0x0d];
    }

    // Empty text (e.g. the Ctrl/Shift/Alt key press itself) — nothing to send.
    if key.is_empty() {
        return vec![];
    }

    // --- Ctrl + letter: synthesise C0 control character --------------------
    // Two cases:
    //   A) Platform already encoded the control char in `key` (e.g. "\x18" for
    //      Ctrl+X on some Linux/macOS builds). Pass through directly.
    //   B) Platform sends the letter ("x") with modifiers.control=true.
    //      We synthesise the C0 code ourselves.
    if ctrl {
        // Case A: key is already a C0 control character (0x01..0x1F, not ESC).
        if let Some(c) = key.chars().next() {
            let cp = c as u32;
            if key.chars().count() == 1 && (0x01..=0x1f).contains(&cp) {
                return vec![cp as u8];
            }
        }
        // Case B: letter + ctrl modifier.
        if let Some(c) = key.chars().next() {
            if key.chars().count() == 1 {
                let upper = c.to_ascii_uppercase() as u8;
                let ctrl_char: Option<u8> = match upper {
                    b'A'..=b'Z' => Some(upper - b'A' + 1), // Ctrl+A=\x01 … Ctrl+Z=\x1A
                    b'[' => Some(0x1b),                    // Ctrl+[ = ESC
                    b'\\' => Some(0x1c),
                    b']' => Some(0x1d),
                    b'^' => Some(0x1e),
                    b'_' => Some(0x1f),
                    b'@' => Some(0x00),
                    _ => None,
                };
                if let Some(byte) = ctrl_char {
                    return vec![byte];
                }
            }
        }
    }

    // --- Skip unknown Private Use Area code points -------------------------
    if key.chars().any(|c| (0xE000..=0xF8FF).contains(&(c as u32))) {
        return vec![];
    }

    // --- Alt + key: prefix with ESC ----------------------------------------
    if alt && !ctrl {
        let mut bytes = vec![0x1b];
        bytes.extend_from_slice(key.as_bytes());
        return bytes;
    }

    // --- Everything else: send UTF-8 bytes as-is ---------------------------
    // This covers printable characters, \r (Enter), \t (Tab), \x1b (Escape),
    // and any C0 control chars the platform already encoded in `key`.
    key.as_bytes().to_vec()
}

fn sgr_mouse_sequence(
    button: i32,
    pressed: bool,
    row: i32,
    col: i32,
    rows: u16,
    cols: u16,
) -> Vec<u8> {
    let max_row = rows.saturating_sub(1) as i32;
    let max_col = cols.saturating_sub(1) as i32;
    let row = row.clamp(0, max_row) + 1;
    let col = col.clamp(0, max_col) + 1;
    let button = button.max(0);
    let final_byte = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{button};{col};{row}{final_byte}").into_bytes()
}

/// Windows-only: returns `true` when the physical Backspace key (VK_BACK) is
/// currently "down" according to `GetKeyState`.
///
/// Used to distinguish real Backspace key presses from synthetic WM_CHAR 0x08
/// events injected by IME drivers (Baidu Pinyin, etc.) when they cancel an
/// in-flight composition.  For a real Backspace, WM_KEYDOWN VK_BACK precedes
/// WM_CHAR 0x08, so GetKeyState returns "down".  For an IME-synthesised
/// Backspace, no VK_BACK keydown was queued, so GetKeyState returns "up".
#[cfg(windows)]
fn is_vk_back_down() -> bool {
    #[allow(non_snake_case)]
    extern "system" {
        fn GetKeyState(nVirtKey: i32) -> i16;
    }
    const VK_BACK: i32 = 0x08;
    unsafe { (GetKeyState(VK_BACK) as u16) & 0x8000 != 0 }
}

/// Windows-only: returns `true` when the letter key for a C0 control code
/// is currently "down" according to `GetKeyState`.
///
/// `GetKeyState` is synchronised with the Windows message queue: its value
/// reflects the state as of the *last message processed by this thread*.
/// When we are called from within a `WM_CHAR` dispatch:
///
/// * **Real Ctrl+Q**: `WM_KEYDOWN VK_Q` was dequeued and processed just
///   before `WM_CHAR 0x11`, so `GetKeyState(VK_Q)` returns "down". ✓
/// * **Synthetic injection** (Aula F99 / Baidu Pinyin tap-Left-Ctrl):
///   the driver posts `WM_CHAR 0x11` directly — no `WM_KEYDOWN VK_Q` was
///   ever in the queue — so `GetKeyState(VK_Q)` returns "up". → dropped ✓
///
/// `cp` is the C0 code point (0x01 = Ctrl+A … 0x1A = Ctrl+Z).
/// Returns `true` (allow) for code points outside 0x01–0x1A (e.g. ESC).
#[cfg(windows)]
fn c0_letter_key_down(cp: u32) -> bool {
    if !(0x01..=0x1a).contains(&cp) {
        return true; // Not a Ctrl+letter — don't filter.
    }
    let vk = (cp + 0x40) as i32; // 0x01→0x41 ('A') … 0x11→0x51 ('Q') …
    #[allow(non_snake_case)]
    extern "system" {
        fn GetKeyState(nVirtKey: i32) -> i16;
    }
    unsafe { (GetKeyState(vk) as u16) & 0x8000 != 0 }
}
