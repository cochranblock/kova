// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

//! f403 = bridge_run. PTY proxy for Claude Code CLI; logs sessions to tele/ for retraining.

use crate::storage::t12;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

// ── Raw terminal mode ────────────────────────────────────────────────────────

#[cfg(unix)]
struct RawMode {
    original: libc::termios,
}

#[cfg(unix)]
impl RawMode {
    fn enter() -> Option<Self> {
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut t) != 0 {
                return None;
            }
            let original = t;
            libc::cfmakeraw(&mut t);
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &t);
            Some(RawMode { original })
        }
    }
}

#[cfg(unix)]
impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &self.original);
        }
    }
}

// ── Terminal size ────────────────────────────────────────────────────────────

fn pty_size_from_terminal() -> PtySize {
    #[cfg(unix)]
    {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        let ok = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) };
        if ok == 0 && ws.ws_row > 0 && ws.ws_col > 0 {
            return PtySize {
                rows: ws.ws_row,
                cols: ws.ws_col,
                pixel_width: ws.ws_xpixel,
                pixel_height: ws.ws_ypixel,
            };
        }
    }
    PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
}

// ── ANSI strip ───────────────────────────────────────────────────────────────

/// Remove ANSI/VT escape sequences from PTY output so stored training text is clean.
/// Strips CSI sequences (`ESC [`…letter), OSC sequences (`ESC ]`…ST/BEL), and bare ESC+char.
fn strip_ansi(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == 0x1b {
            i += 1;
            if i >= raw.len() {
                break;
            }
            match raw[i] {
                b'[' => {
                    // CSI: skip until a byte in 0x40–0x7E (the final byte)
                    i += 1;
                    while i < raw.len() && !(0x40..=0x7e).contains(&raw[i]) {
                        i += 1;
                    }
                    i += 1; // skip final byte
                }
                b']' => {
                    // OSC: skip until ST (ESC \) or BEL (0x07)
                    i += 1;
                    while i < raw.len() {
                        if raw[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if raw[i] == 0x1b && i + 1 < raw.len() && raw[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    i += 1; // bare ESC+char — skip both
                }
            }
        } else if raw[i] < 0x20 && raw[i] != b'\n' && raw[i] != b'\r' && raw[i] != b'\t' {
            i += 1; // drop other control characters
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── f403 ─────────────────────────────────────────────────────────────────────

/// f403 = bridge_run. Spawn `cmd` in a PTY, proxy I/O, log session to `tele/` in redb.
///
/// Stored keys: `tele/{ts}/raw_i` (stdin, ANSI-stripped), `tele/{ts}/raw_o` (PTY output,
/// ANSI-stripped). These feed the same export_tele pipeline as REPL telemetry.
///
/// The stdin thread is detached (not joined) because `libc::read` on STDIN_FILENO has no
/// interruptible cancel path without a pipe/eventfd. It will exit on the next write failure
/// after the child exits, or when the process exits — whichever comes first.
pub fn f403_run(cmd: &str, extra_args: &[String]) -> anyhow::Result<()> {
    let pty_system = native_pty_system();
    let size = pty_size_from_terminal();
    let pair = pty_system
        .openpty(size)
        .map_err(|e| anyhow::anyhow!("openpty failed: {e}"))?;

    let mut cb = CommandBuilder::new(cmd);
    for a in extra_args {
        cb.arg(a);
    }

    let mut child = pair
        .slave
        .spawn_command(cb)
        .map_err(|e| anyhow::anyhow!("failed to spawn {cmd:?}: {e}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("PTY reader clone failed: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| anyhow::anyhow!("PTY writer failed: {e}"))?;

    let input_log: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let output_log: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let input_log_t = Arc::clone(&input_log);
    let output_log_t = Arc::clone(&output_log);

    // Enter raw mode; RAII restores on drop (including on panic path).
    #[cfg(unix)]
    let _raw = RawMode::enter();

    // Detached thread: stdin → PTY master.
    // Uses libc::read directly to bypass Rust stdio buffering in raw mode.
    // Not joined — see doc comment on f403_run.
    std::thread::spawn(move || {
        let mut writer = writer;
        let mut buf = [0u8; 256];
        loop {
            let n = unsafe {
                libc::read(libc::STDIN_FILENO, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
            };
            if n <= 0 {
                break;
            }
            let n = n as usize;
            if let Ok(mut log) = input_log_t.lock() {
                log.extend_from_slice(&buf[..n]);
            }
            if writer.write_all(&buf[..n]).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    // Main thread: PTY master → stdout.
    let mut stdout = std::io::stdout();
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                if let Ok(mut log) = output_log_t.lock() {
                    log.extend_from_slice(chunk);
                }
                let _ = stdout.write_all(chunk);
                let _ = stdout.flush();
            }
        }
    }

    let _ = child.wait();
    // _raw drops here, restoring terminal mode before the summary line.

    if let Ok(store) = t12::f39() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let raw_in = strip_ansi(&input_log.lock().unwrap());
        let raw_out = strip_ansi(&output_log.lock().unwrap());
        let _ = store.f40(format!("tele/{ts}/raw_i").as_bytes(), &raw_in);
        let _ = store.f40(format!("tele/{ts}/raw_o").as_bytes(), &raw_out);
        eprintln!("[bridge] session logged ({} in, {} out bytes)", raw_in.len(), raw_out.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn strip_ansi_removes_csi_sequences() {
        let input = b"\x1b[32mhello\x1b[0m world";
        assert_eq!(strip_ansi(input), "hello world");
    }

    #[test]
    fn strip_ansi_removes_cursor_movement() {
        let input = b"foo\x1b[2J\x1b[Hbar";
        assert_eq!(strip_ansi(input), "foobar");
    }

    #[test]
    fn strip_ansi_removes_osc_sequence() {
        let input = b"\x1b]0;window title\x07text";
        assert_eq!(strip_ansi(input), "text");
    }

    #[test]
    fn strip_ansi_preserves_newlines_and_tabs() {
        let input = b"line1\nline2\ttab";
        assert_eq!(strip_ansi(input), "line1\nline2\ttab");
    }

    #[test]
    fn strip_ansi_bare_escape_skipped() {
        let input = b"a\x1bMb"; // ESC M = reverse linefeed
        assert_eq!(strip_ansi(input), "ab");
    }
}
