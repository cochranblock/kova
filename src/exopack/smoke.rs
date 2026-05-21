// Unlicense — public domain — cochranblock.org
//! smoke — screenshot every user-facing CLI + HTTP surface of kova.
//!
//! f435=smoke_run, f436=render_term_png, f437=run_cli_surface, f438=smoke_http
//! T219=SmokeReport, T220=SmokeSurface

#![allow(non_camel_case_types, non_snake_case, dead_code, unused_imports)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

// ── T220 = SmokeSurface ──────────────────────────────────────────────────────

/// T220 = SmokeSurface. One captured surface: name, command, PNG path, status.
pub struct T220 {
    pub name: String,
    pub command: String,
    pub png: PathBuf,
    pub ok: bool,
    pub err: Option<String>,
}

// ── T219 = SmokeReport ──────────────────────────────────────────────────────

/// T219 = SmokeReport. All captured surfaces + output directory.
pub struct T219 {
    pub out_dir: PathBuf,
    pub surfaces: Vec<T220>,
}

impl T219 {
    /// Print colored summary: ✓/✗ per surface, final directory.
    pub fn print_summary(&self) {
        let ok = self.surfaces.iter().filter(|s| s.ok).count();
        let total = self.surfaces.len();
        for s in &self.surfaces {
            let (icon, color) = if s.ok {
                ("\u{2713}", "\x1b[32m")
            } else {
                ("\u{2717}", "\x1b[31m")
            };
            println!(
                "  {}{}\x1b[0m  {:<22}  {}",
                color,
                icon,
                s.name,
                s.png.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
            );
            if let Some(e) = &s.err {
                println!("       \x1b[90m{}\x1b[0m", e);
            }
        }
        let color = if ok == total { "\x1b[32m" } else { "\x1b[33m" };
        println!("\n  {}{}/{} surfaces captured\x1b[0m", color, ok, total);
        println!("  out: {}", self.out_dir.display());
    }
}

// ── ANSI color table (GitHub dark palette) ──────────────────────────────────

fn ansi_color(code: u8) -> Option<&'static str> {
    match code {
        30 => Some("#555566"),
        31 => Some("#ff7b72"),
        32 => Some("#56d364"),
        33 => Some("#e3b341"),
        34 => Some("#58a6ff"),
        35 => Some("#d2a8ff"),
        36 => Some("#39c5cf"),
        37 => Some("#b1bac4"),
        39 => Some("#c9d1d9"),
        90 => Some("#6e7681"),
        91 => Some("#ffa198"),
        92 => Some("#4ae168"),
        93 => Some("#f7ca38"),
        94 => Some("#79c0ff"),
        95 => Some("#e2c5ff"),
        96 => Some("#56d4dd"),
        97 => Some("#e6edf3"),
        _ => None,
    }
}

// ── ANSI span ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Span {
    color: String,
    bold: bool,
    text: String,
}

// ── XML escape ───────────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ── Parse ANSI line → Vec<Span> ──────────────────────────────────────────────

fn parse_ansi_line(line: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut cur_color = "#c9d1d9".to_string();
    let mut cur_bold = false;
    let mut buf = String::new();

    let flush_buf = |buf: &mut String, spans: &mut Vec<Span>, color: &str, bold: bool| {
        if !buf.is_empty() {
            let text = buf.replace('\t', "    ");
            spans.push(Span {
                color: color.to_string(),
                bold,
                text,
            });
            buf.clear();
        }
    };

    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\x1b' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // CSI sequence — find final byte (letter)
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && !bytes[end].is_ascii_alphabetic() {
                end += 1;
            }
            if end < bytes.len() {
                let final_byte = bytes[end];
                if final_byte == b'm' {
                    flush_buf(&mut buf, &mut spans, &cur_color.clone(), cur_bold);
                    let params = &line[start..end];
                    if params.is_empty() || params == "0" {
                        cur_color = "#c9d1d9".to_string();
                        cur_bold = false;
                    } else {
                        for part in params.split(';') {
                            let code: u8 = part.trim().parse().unwrap_or(0);
                            match code {
                                0 => {
                                    cur_color = "#c9d1d9".to_string();
                                    cur_bold = false;
                                }
                                1 => cur_bold = true,
                                2 | 22 => cur_bold = false,
                                _ => {
                                    if let Some(c) = ansi_color(code) {
                                        cur_color = c.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
                i = end + 1;
            } else {
                i += 1;
            }
        } else {
            let c = line[i..].chars().next().unwrap_or('\0');
            if c == '\t' || c >= ' ' {
                buf.push(c);
            }
            i += c.len_utf8();
        }
    }
    let col = cur_color.clone();
    flush_buf(&mut buf, &mut spans, &col, cur_bold);
    spans
}

// ── SVG helpers (avoid format! with hex colors in raw strings) ───────────────

fn svg_rect(w: u32, h: u32, fill: &str) -> String {
    let mut s = String::new();
    s.push_str("<rect width=\"");
    s.push_str(&w.to_string());
    s.push_str("\" height=\"");
    s.push_str(&h.to_string());
    s.push_str("\" fill=\"");
    s.push_str(fill);
    s.push_str("\"/>");
    s
}

fn svg_title_text(px: u32, ty: u32, fs: u32, cmd: &str) -> String {
    let mut s = String::new();
    s.push_str("<text x=\"");
    s.push_str(&px.to_string());
    s.push_str("\" y=\"");
    s.push_str(&ty.to_string());
    s.push_str("\" font-family=\"'DejaVu Sans Mono','Menlo',monospace\" font-size=\"");
    s.push_str(&fs.to_string());
    s.push_str("\" fill=\"#58a6ff\">$ ");
    s.push_str(cmd);
    s.push_str("</text>");
    s
}

fn svg_line_open(px: u32, y: u32, fs: u32) -> String {
    let mut s = String::new();
    s.push_str("<text x=\"");
    s.push_str(&px.to_string());
    s.push_str("\" y=\"");
    s.push_str(&y.to_string());
    s.push_str("\" font-family=\"'DejaVu Sans Mono','Menlo',monospace\" font-size=\"");
    s.push_str(&fs.to_string());
    s.push_str("\" xml:space=\"preserve\">");
    s
}

fn svg_tspan(color: &str, bold: bool, text: &str) -> String {
    let mut s = String::new();
    s.push_str("<tspan fill=\"");
    s.push_str(color);
    s.push_str("\"");
    if bold {
        s.push_str(" font-weight=\"bold\"");
    }
    s.push('>');
    s.push_str(text);
    s.push_str("</tspan>");
    s
}

// ── f436 = render_term_png ───────────────────────────────────────────────────

/// f436 = render_term_png. Parse ANSI text → SVG → PNG via resvg/tiny-skia.
pub fn f436_render_term_png(
    lines: &[String],
    command: &str,
    out: &Path,
) -> Result<(), String> {
    const LINE_HEIGHT: f64 = 18.0;
    const FONT_SIZE: f64 = 13.0;
    const CHAR_WIDTH: f64 = 7.8;
    const PADDING: f64 = 14.0;
    const TITLE_H: f64 = 26.0;
    const MIN_WIDTH: f64 = 640.0;

    // Compute visible char count for each line (strip ANSI)
    let longest = lines
        .iter()
        .map(|l| {
            let mut n = 0usize;
            let b = l.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'\x1b' && i + 1 < b.len() && b[i + 1] == b'[' {
                    let s = i + 2;
                    let mut e = s;
                    while e < b.len() && !b[e].is_ascii_alphabetic() {
                        e += 1;
                    }
                    i = if e < b.len() { e + 1 } else { b.len() };
                } else {
                    let c = l[i..].chars().next().unwrap_or('\0');
                    if c == '\t' {
                        n += 4;
                    } else if c >= ' ' {
                        n += 1;
                    }
                    i += c.len_utf8();
                }
            }
            n
        })
        .max()
        .unwrap_or(0);

    let width = (longest as f64 * CHAR_WIDTH + PADDING * 2.0).max(MIN_WIDTH);
    let height = TITLE_H + PADDING + (lines.len() as f64) * LINE_HEIGHT + PADDING;

    let w_px = width as u32;
    let h_px = height as u32;

    // Build SVG
    let mut svg = String::with_capacity(4096);
    // Opening tag
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
    svg.push_str(&w_px.to_string());
    svg.push_str("\" height=\"");
    svg.push_str(&h_px.to_string());
    svg.push_str("\">");

    // Background
    svg.push_str(&svg_rect(w_px, h_px, "#0d1117"));

    // Title bar
    svg.push_str(&svg_rect(w_px, TITLE_H as u32, "#161b22"));

    // Title text: $ <command>
    let cmd_escaped = xml_escape(command);
    svg.push_str(&svg_title_text(
        PADDING as u32,
        (TITLE_H * 0.72) as u32,
        FONT_SIZE as u32,
        &cmd_escaped,
    ));

    // Lines
    for (idx, line) in lines.iter().enumerate() {
        let y = TITLE_H + PADDING + (idx as f64) * LINE_HEIGHT + LINE_HEIGHT * 0.8;
        let spans = parse_ansi_line(line);

        svg.push_str(&svg_line_open(PADDING as u32, y as u32, FONT_SIZE as u32));

        if spans.is_empty() {
            svg.push_str(&svg_tspan("#c9d1d9", false, " "));
        } else {
            for span in &spans {
                let text_escaped = xml_escape(&span.text);
                svg.push_str(&svg_tspan(&span.color, span.bold, &text_escaped));
            }
        }

        svg.push_str("</text>");
    }

    svg.push_str("</svg>");

    // Render SVG → PNG via usvg + resvg + tiny_skia
    // In usvg 0.47, fontdb is set via Options::fontdb field
    let mut opt = usvg::Options::default();
    {
        let fdb = Arc::make_mut(&mut opt.fontdb);
        fdb.load_system_fonts();
    }

    let tree = usvg::Tree::from_str(&svg, &opt)
        .map_err(|e| format!("usvg parse: {}", e))?;

    let sz = tree.size();
    let tw = sz.width() as u32;
    let th = sz.height() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(tw, th)
        .ok_or_else(|| "pixmap alloc failed".to_string())?;

    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());

    pixmap
        .save_png(out)
        .map_err(|e| format!("save_png: {}", e))?;

    Ok(())
}

// ── f437 = run_cli_surface ────────────────────────────────────────────────────

/// f437 = run_cli_surface. Spawn kova with argv, capture combined stdout+stderr, render PNG.
pub fn f437_run_cli_surface(
    kova_bin: &Path,
    argv: &[&str],
    out_dir: &Path,
    name: &str,
) -> Result<T220, String> {
    let command_str = format!("kova {}", argv.join(" "));
    let png = out_dir.join(format!("{}.png", name));

    let output = Command::new(kova_bin)
        .args(argv)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("spawn kova: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Combine stdout + stderr
    let combined = if stderr.is_empty() {
        stdout
    } else if stdout.is_empty() {
        stderr
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    const MAX_LINES: usize = 60;
    let all_lines: Vec<String> = combined.lines().map(|l| l.to_string()).collect();
    let total = all_lines.len();

    let mut display_lines: Vec<String> = all_lines.into_iter().take(MAX_LINES).collect();
    if total > MAX_LINES {
        display_lines.push(format!(
            "\x1b[90m... ({} more lines)\x1b[0m",
            total - MAX_LINES
        ));
    }

    f436_render_term_png(&display_lines, &command_str, &png)?;

    Ok(T220 {
        name: name.to_string(),
        command: command_str,
        png,
        ok: true,
        err: None,
    })
}

// ── f438 = smoke_http ─────────────────────────────────────────────────────────

/// f438 = smoke_http. Spin up `kova serve`, hit HTTP endpoints, render PNGs.
/// Returns `Err(String)` if the server never becomes ready.
pub fn f438_smoke_http(
    kova_bin: &Path,
    out_dir: &Path,
) -> Result<Vec<T220>, String> {
    // Find a free port
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind free port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {}", e))?
        .port();
    drop(listener); // release so kova can bind it

    let bind_addr = format!("127.0.0.1:{}", port);
    let base_url = format!("http://127.0.0.1:{}", port);

    // Spawn `kova serve`
    let mut child = Command::new(kova_bin)
        .arg("serve")
        .env("KOVA_BIND", &bind_addr)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn kova serve: {}", e))?;

    // Poll /api/status for readiness (15 × 300ms = 4.5 s)
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("reqwest client: {}", e))?;

    let ready_url = format!("{}/api/status", base_url);
    let mut ready = false;
    for _ in 0..15 {
        std::thread::sleep(Duration::from_millis(300));
        if client.get(&ready_url).send().is_ok() {
            ready = true;
            break;
        }
    }

    if !ready {
        let _ = child.kill();
        return Err(format!(
            "kova serve never became ready on {} (timeout 4.5s)",
            bind_addr
        ));
    }

    // Endpoints to capture
    let endpoints: &[(&str, &str)] = &[
        ("api-status", "/api/status"),
        ("api-projects", "/api/projects"),
        ("api-prompts", "/api/prompts"),
        ("openapi", "/openapi.json"),
    ];

    let mut surfaces = Vec::new();
    for (name, path) in endpoints {
        let url = format!("{}{}", base_url, path);
        let command_str = format!("GET {}", url);
        let png = out_dir.join(format!("{}.png", name));

        let result: Result<T220, String> = (|| {
            let resp = client
                .get(&url)
                .send()
                .map_err(|e| format!("GET {}: {}", url, e))?;

            let status = resp.status();
            let body = resp.text().map_err(|e| format!("body: {}", e))?;

            // Try to pretty-print JSON
            let display_body = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                serde_json::to_string_pretty(&v).unwrap_or(body)
            } else {
                body
            };

            const MAX_LINES: usize = 60;
            let all_lines: Vec<String> = display_body.lines().map(|l| l.to_string()).collect();
            let total = all_lines.len();
            let mut display_lines: Vec<String> = all_lines.into_iter().take(MAX_LINES).collect();
            if total > MAX_LINES {
                display_lines.push(format!(
                    "\x1b[90m... ({} more lines)\x1b[0m",
                    total - MAX_LINES
                ));
            }

            // Prepend status line
            let status_color = if status.is_success() { "\x1b[32m" } else { "\x1b[31m" };
            let mut render_lines = vec![format!(
                "{}{} {}\x1b[0m",
                status_color,
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            )];
            render_lines.extend(display_lines);

            f436_render_term_png(&render_lines, &command_str, &png)?;

            Ok(T220 {
                name: name.to_string(),
                command: command_str.clone(),
                png: png.clone(),
                ok: true,
                err: None,
            })
        })();

        match result {
            Ok(s) => surfaces.push(s),
            Err(e) => {
                let fallback_lines = vec![format!("\x1b[31merror: {}\x1b[0m", e)];
                let _ = f436_render_term_png(&fallback_lines, &command_str, &png);
                surfaces.push(T220 {
                    name: name.to_string(),
                    command: command_str,
                    png,
                    ok: false,
                    err: Some(e),
                });
            }
        }
    }

    let _ = child.kill();
    Ok(surfaces)
}

// ── HTML index ───────────────────────────────────────────────────────────────

fn write_html_index(surfaces: &[T220], out_dir: &Path) -> Result<(), String> {
    let ok = surfaces.iter().filter(|s| s.ok).count();
    let total = surfaces.len();

    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<title>kova smoke</title>\n<style>\n");
    html.push_str("body{background:#0d1117;color:#c9d1d9;font-family:'DejaVu Sans Mono',Menlo,monospace;margin:0;padding:20px}\n");
    html.push_str("h1{color:#58a6ff;margin-bottom:4px}\n");
    html.push_str(".summary{color:#8b949e;margin-bottom:20px}\n");
    html.push_str(".grid{display:flex;flex-wrap:wrap;gap:16px}\n");
    html.push_str(".card{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:12px;max-width:680px}\n");
    html.push_str(".card-name{font-weight:bold;color:#e6edf3;margin-bottom:4px}\n");
    html.push_str(".card-cmd{font-size:11px;color:#8b949e;margin-bottom:8px}\n");
    html.push_str(".card-cmd code{background:#21262d;padding:2px 6px;border-radius:4px;color:#58a6ff}\n");
    html.push_str(".card img{max-width:100%;border-radius:4px;display:block}\n");
    html.push_str(".card .err{color:#ff7b72;font-size:12px;margin-top:6px}\n");
    html.push_str(".ok{color:#56d364}\n.fail{color:#ff7b72}\n");
    html.push_str("</style>\n</head>\n<body>\n<h1>kova smoke</h1>\n");

    let sum_class = if ok == total { "ok" } else { "fail" };
    html.push_str("<div class=\"summary\"><span class=\"");
    html.push_str(sum_class);
    html.push_str("\">");
    html.push_str(&ok.to_string());
    html.push('/');
    html.push_str(&total.to_string());
    html.push_str(" surfaces captured</span></div>\n<div class=\"grid\">\n");

    for s in surfaces {
        let png_name = s
            .png
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        html.push_str("<div class=\"card\">\n<div class=\"card-name\">");
        html.push_str(if s.ok { "&#x2713; " } else { "&#x2717; " });
        html.push_str(&s.name);
        html.push_str("</div>\n<div class=\"card-cmd\"><code>");
        html.push_str(&s.command);
        html.push_str("</code></div>\n");

        if s.ok {
            html.push_str("<img src=\"");
            html.push_str(&png_name);
            html.push_str("\" alt=\"");
            html.push_str(&s.name);
            html.push_str("\" loading=\"lazy\">\n");
        }
        if let Some(e) = &s.err {
            html.push_str("<div class=\"err\">");
            html.push_str(e);
            html.push_str("</div>\n");
        }
        html.push_str("</div>\n");
    }

    html.push_str("</div>\n</body>\n</html>\n");

    let path = out_dir.join("index.html");
    std::fs::write(&path, html).map_err(|e| format!("write index.html: {}", e))?;
    Ok(())
}

// ── CLI surfaces table ────────────────────────────────────────────────────────

static CLI_SURFACES: &[(&str, &[&str])] = &[
    ("help", &["--help"]),
    ("tokens", &["tokens"]),
    ("queue-status", &["c2", "queue", "status"]),
    ("git-status", &["git", "g0"]),
    ("recent", &["recent", "--minutes", "30"]),
    ("c2-nodes", &["c2", "nodes"]),
    ("bridge-help", &["bridge", "--help"]),
    ("squeeze-help", &["squeeze", "--help"]),
    ("mcp-help", &["mcp", "--help"]),
    ("ci-help", &["ci", "--help"]),
];

// ── f435 = smoke_run ─────────────────────────────────────────────────────────

/// f435 = smoke_run. Capture all CLI + HTTP surfaces, write index.html, return T219.
pub fn f435_smoke_run(kova_bin: &Path, out_dir: &Path) -> Result<T219, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("mkdir {}: {}", out_dir.display(), e))?;

    let mut surfaces: Vec<T220> = Vec::new();

    // CLI surfaces
    for (name, argv) in CLI_SURFACES {
        println!("  capturing {} ...", name);
        match f437_run_cli_surface(kova_bin, argv, out_dir, name) {
            Ok(s) => {
                println!(
                    "    ok -> {}",
                    s.png.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
                );
                surfaces.push(s);
            }
            Err(e) => {
                println!("    \x1b[31mfail\x1b[0m: {}", e);
                let png = out_dir.join(format!("{}.png", name));
                let err_lines = vec![format!("\x1b[31merror: {}\x1b[0m", e)];
                let cmd_str = format!("kova {}", argv.join(" "));
                let _ = f436_render_term_png(&err_lines, &cmd_str, &png);
                surfaces.push(T220 {
                    name: name.to_string(),
                    command: cmd_str,
                    png,
                    ok: false,
                    err: Some(e),
                });
            }
        }
    }

    // HTTP surfaces
    println!("  capturing http surfaces ...");
    match f438_smoke_http(kova_bin, out_dir) {
        Ok(http_surfaces) => {
            for s in http_surfaces {
                println!(
                    "    {} {} -> {}",
                    if s.ok { "ok" } else { "fail" },
                    s.name,
                    s.png.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
                );
                surfaces.push(s);
            }
        }
        Err(e) => {
            eprintln!("  \x1b[33mwarn\x1b[0m: HTTP surfaces skipped: {}", e);
        }
    }

    // Write index.html
    write_html_index(&surfaces, out_dir)?;

    Ok(T219 {
        out_dir: out_dir.to_path_buf(),
        surfaces,
    })
}
