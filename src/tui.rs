// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Terminal UI. ratatui + crossterm. Replaces egui GUI and plain REPL.
//! `kova` (no args) launches this. Chat + tool display + Visual QC.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::path::{Path, PathBuf};

// Theme colors from THEME.md mapped to terminal RGB.
const PRIMARY: Color = Color::Rgb(0x00, 0xd4, 0xff);
const SECONDARY: Color = Color::Rgb(0xa8, 0x55, 0xf7);
const TERTIARY: Color = Color::Rgb(0x14, 0xb8, 0xa6);
const TEXT: Color = Color::Rgb(0xe2, 0xe8, 0xf0);
const MUTED: Color = Color::Rgb(0x64, 0x74, 0x8b);
const BG: Color = Color::Rgb(0x0a, 0x0a, 0x0f);
const SURFACE: Color = Color::Rgb(0x14, 0x14, 0x1f);
const APPROVE_GREEN: Color = Color::Rgb(0x16, 0xa3, 0x4a);
const REJECT_RED: Color = Color::Rgb(0xdc, 0x26, 0x26);

/// Active mode in the TUI.
#[derive(PartialEq)]
enum Mode {
    Chat,
    VisualQc,
}

/// Visual QC verdict.
#[derive(Clone, Copy, PartialEq)]
enum Verdict {
    Approve,
    Reject,
    Skip,
}

/// One image file in the QC queue.
struct QcEntry {
    path: PathBuf,
    label: String,
    verdict: Option<Verdict>,
}

/// Visual QC state (terminal version of sprite_qc).
struct VisualQc {
    entries: Vec<QcEntry>,
    current: usize,
    root: PathBuf,
}

impl VisualQc {
    fn scan(root: &Path) -> Self {
        let mut entries = Vec::new();
        collect_images(root, root, &mut entries);
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Self {
            entries,
            current: 0,
            root: root.to_path_buf(),
        }
    }

    fn total(&self) -> usize {
        self.entries.len()
    }
    fn approved(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == Some(Verdict::Approve)).count()
    }
    fn rejected(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == Some(Verdict::Reject)).count()
    }
    fn remaining(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict.is_none()).count()
    }
    fn is_done(&self) -> bool {
        self.current >= self.entries.len()
    }

    fn decide(&mut self, v: Verdict) {
        if self.current < self.entries.len() {
            self.entries[self.current].verdict = Some(v);
            self.current += 1;
        }
    }

    fn apply_verdicts(&self) -> (usize, usize) {
        let approved_dir = self.root.join("approved");
        let rejected_dir = self.root.join("rejected");
        let _ = std::fs::create_dir_all(&approved_dir);
        let _ = std::fs::create_dir_all(&rejected_dir);

        let (mut a, mut r) = (0, 0);
        for entry in &self.entries {
            let rel = entry.path.strip_prefix(&self.root).unwrap_or(&entry.path);
            match entry.verdict {
                Some(Verdict::Approve) => {
                    let dest = approved_dir.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&entry.path, &dest);
                    a += 1;
                }
                Some(Verdict::Reject) => {
                    let dest = rejected_dir.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&entry.path, &dest);
                    r += 1;
                }
                _ => {}
            }
        }
        (a, r)
    }
}

fn collect_images(root: &Path, dir: &Path, out: &mut Vec<QcEntry>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "approved" || name == "rejected" {
                continue;
            }
            collect_images(root, &path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("png") | Some("jpg") | Some("jpeg") | Some("svg")
        ) {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let label = rel.to_string_lossy().replace('/', " / ").replace(".png", "");
            out.push(QcEntry { path, label, verdict: None });
        }
    }
}

/// Main TUI app state.
struct App {
    mode: Mode,
    input: String,
    cursor_pos: usize,
    messages: Vec<ChatMessage>,
    scroll: u16,
    project_dir: PathBuf,
    model_path: Option<PathBuf>,
    system_prompt: String,
    qc: Option<VisualQc>,
    status: String,
    running: bool,
}

struct ChatMessage {
    role: &'static str,
    content: String,
}

impl App {
    fn new(project: Option<PathBuf>) -> Self {
        let project_dir = project
            .or_else(|| std::env::var("KOVA_PROJECT").ok().map(PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let model_path = crate::config::inference_model_path();
        let project_name = project_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let status = format!(
            "project: {} | model: {} | /qc /clear /quit",
            project_name,
            model_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "none".into())
        );

        #[cfg(feature = "inference")]
        let system_prompt = crate::repl::f139(&project_dir);
        #[cfg(not(feature = "inference"))]
        let system_prompt = String::new();

        Self {
            mode: Mode::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir,
            model_path,
            system_prompt,
            qc: None,
            status,
            running: true,
        }
    }

    fn submit_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }
        self.input.clear();
        self.cursor_pos = 0;

        // Commands.
        if input == "/quit" || input == "/exit" || input == "/q" {
            self.running = false;
            return;
        }
        if input == "/clear" {
            self.messages.clear();
            self.scroll = 0;
            return;
        }
        if input == "/qc" {
            self.open_visual_qc();
            return;
        }
        if input.starts_with("/project ") {
            let p = input.strip_prefix("/project ").unwrap().trim();
            let path = PathBuf::from(p);
            if path.exists() {
                self.project_dir = path;
                self.status = format!("project: {} | /qc /clear /quit", p);
                self.messages.push(ChatMessage {
                    role: "system",
                    content: format!("project switched to {}", p),
                });
            } else {
                self.messages.push(ChatMessage {
                    role: "system",
                    content: format!("not found: {}", p),
                });
            }
            return;
        }
        if input == "/tools" {
            let mut lines = String::new();
            for tool in crate::tools::TOOLS {
                lines.push_str(&format!("  {} — {}\n", tool.name, tool.description));
            }
            self.messages.push(ChatMessage {
                role: "system",
                content: lines,
            });
            return;
        }

        // Chat message.
        self.messages.push(ChatMessage {
            role: "user",
            content: input.clone(),
        });

        // Run agent loop (blocking — tokens go to a buffer, not stdout).
        #[cfg(feature = "inference")]
        {
            if let Some(ref model_path) = self.model_path {
                let max_iter = crate::config::orchestration_max_fix_retries() + 20;
                let response = crate::agent_loop::f148(
                    model_path,
                    &self.system_prompt,
                    &input,
                    &self.project_dir,
                    max_iter,
                );

                // Store in sled.
                let store_path = crate::config::sled_path();
                if let Ok(store) = crate::storage::t12::f39(&store_path) {
                    let _ = crate::context::f73(&store, "user", &input);
                    let _ = crate::context::f73(&store, "assistant", &response);
                }

                self.messages.push(ChatMessage {
                    role: "assistant",
                    content: response,
                });
            } else {
                self.messages.push(ChatMessage {
                    role: "system",
                    content: "No model loaded. Run: kova model install".into(),
                });
            }
        }
        #[cfg(not(feature = "inference"))]
        {
            self.messages.push(ChatMessage {
                role: "system",
                content: "Inference not available. Build with --features inference".into(),
            });
        }
        // Auto-scroll to bottom.
        self.scroll = u16::MAX;
    }

    fn open_visual_qc(&mut self) {
        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screenshots")
            .join(std::env::consts::OS)
            .join("kova");

        if cache.is_dir() {
            let qc = VisualQc::scan(&cache);
            if qc.total() == 0 {
                self.messages.push(ChatMessage {
                    role: "system",
                    content: format!("No images found in {}", cache.display()),
                });
            } else {
                let count = qc.total();
                self.qc = Some(qc);
                self.mode = Mode::VisualQc;
                self.messages.push(ChatMessage {
                    role: "system",
                    content: format!("Visual QC: {} images loaded from {}", count, cache.display()),
                });
            }
        } else {
            self.messages.push(ChatMessage {
                role: "system",
                content: format!("No screenshots dir: {}", cache.display()),
            });
        }
    }
}

/// f113=tui_run. Main TUI entry point.
pub fn run(project: Option<PathBuf>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(project);

    while app.running {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match app.mode {
                    Mode::Chat => handle_chat_key(&mut app, key.code, key.modifiers),
                    Mode::VisualQc => handle_qc_key(&mut app, key.code),
                }
            }
        }
    }

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_chat_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        KeyCode::Enter => app.submit_input(),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.running = false;
        }
        KeyCode::Char(c) => {
            app.input.insert(app.cursor_pos, c);
            app.cursor_pos += 1;
        }
        KeyCode::Backspace => {
            if app.cursor_pos > 0 {
                app.cursor_pos -= 1;
                app.input.remove(app.cursor_pos);
            }
        }
        KeyCode::Delete => {
            if app.cursor_pos < app.input.len() {
                app.input.remove(app.cursor_pos);
            }
        }
        KeyCode::Left => {
            if app.cursor_pos > 0 {
                app.cursor_pos -= 1;
            }
        }
        KeyCode::Right => {
            if app.cursor_pos < app.input.len() {
                app.cursor_pos += 1;
            }
        }
        KeyCode::Home => app.cursor_pos = 0,
        KeyCode::End => app.cursor_pos = app.input.len(),
        KeyCode::Up => {
            if app.scroll > 0 {
                app.scroll = app.scroll.saturating_sub(1);
            }
        }
        KeyCode::Down => {
            app.scroll = app.scroll.saturating_add(1);
        }
        KeyCode::Esc => {
            if app.mode == Mode::VisualQc {
                app.mode = Mode::Chat;
            }
        }
        _ => {}
    }
}

fn handle_qc_key(app: &mut App, code: KeyCode) {
    match code {
        // Approve
        KeyCode::Char('d') | KeyCode::Right => {
            if let Some(ref mut qc) = app.qc {
                qc.decide(Verdict::Approve);
            }
        }
        // Reject
        KeyCode::Char('a') | KeyCode::Left => {
            if let Some(ref mut qc) = app.qc {
                qc.decide(Verdict::Reject);
            }
        }
        // Skip
        KeyCode::Char('s') | KeyCode::Down => {
            if let Some(ref mut qc) = app.qc {
                qc.decide(Verdict::Skip);
            }
        }
        // Save results
        KeyCode::Enter => {
            if let Some(ref qc) = app.qc {
                if qc.is_done() {
                    let (a, r) = qc.apply_verdicts();
                    app.messages.push(ChatMessage {
                        role: "system",
                        content: format!("Saved: {} approved, {} rejected", a, r),
                    });
                    app.qc = None;
                    app.mode = Mode::Chat;
                }
            }
        }
        // Exit QC
        KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = Mode::Chat;
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Clear background.
    let bg_block = Block::default().style(Style::default().bg(BG));
    f.render_widget(bg_block, size);

    match app.mode {
        Mode::Chat => draw_chat(f, app, size),
        Mode::VisualQc => draw_visual_qc(f, app, size),
    }
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(5),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" KOVA ", Style::default().fg(BG).bg(PRIMARY).add_modifier(Modifier::BOLD)),
        Span::styled(" augment engine", Style::default().fg(MUTED)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(SURFACE)));
    f.render_widget(header, chunks[0]);

    // Messages
    let msg_area = chunks[1];
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        let (prefix, color) = match msg.role {
            "user" => ("> ", PRIMARY),
            "assistant" => ("  ", TEXT),
            "system" => ("  ", MUTED),
            _ => ("  ", TEXT),
        };

        // Add blank line between messages.
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        for line in msg.content.lines() {
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::styled(line, Style::default().fg(color)),
            ]));
        }
    }

    // Calculate scroll: auto-scroll to bottom if scroll is MAX.
    let visible_height = msg_area.height as usize;
    let total_lines = lines.len();
    let scroll_offset = if app.scroll == u16::MAX {
        total_lines.saturating_sub(visible_height) as u16
    } else {
        app.scroll.min(total_lines.saturating_sub(visible_height) as u16)
    };

    let messages = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0))
        .block(
            Block::default()
                .borders(Borders::NONE)
                .style(Style::default().bg(BG)),
        );
    f.render_widget(messages, msg_area);

    // Input
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.input.is_empty() { SURFACE } else { PRIMARY }))
        .style(Style::default().bg(SURFACE));

    let display_input = if app.input.is_empty() {
        Span::styled("augment...", Style::default().fg(MUTED))
    } else {
        Span::styled(&app.input, Style::default().fg(TEXT))
    };

    let input = Paragraph::new(Line::from(display_input)).block(input_block);
    f.render_widget(input, chunks[2]);

    // Place cursor in input field.
    f.set_cursor_position((
        chunks[2].x + 1 + app.cursor_pos as u16,
        chunks[2].y + 1,
    ));

    // Status bar
    let status = Paragraph::new(Line::from(Span::styled(
        &app.status,
        Style::default().fg(MUTED),
    )))
    .style(Style::default().bg(SURFACE));
    f.render_widget(status, chunks[3]);
}

fn draw_visual_qc(f: &mut Frame, app: &App, area: Rect) {
    let qc = match &app.qc {
        Some(q) => q,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Progress
            Constraint::Min(5),   // File list
            Constraint::Length(3), // Controls
            Constraint::Length(1), // Status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" VISUAL QC ", Style::default().fg(BG).bg(SECONDARY).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("  {} approved  {} rejected  {} remaining",
                qc.approved(), qc.rejected(), qc.remaining()),
            Style::default().fg(MUTED),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(SURFACE)));
    f.render_widget(header, chunks[0]);

    // Progress bar
    let progress = if qc.total() == 0 {
        1.0
    } else {
        qc.current as f64 / qc.total() as f64
    };
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(TERTIARY).bg(SURFACE))
        .ratio(progress)
        .label(format!("{}/{}", qc.current, qc.total()));
    f.render_widget(gauge, chunks[1]);

    // File list — show current and surrounding entries.
    if qc.is_done() {
        let summary = Paragraph::new(vec![
            Line::from(Span::styled("QC Complete", Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled(
                format!("Approved: {}  Rejected: {}  Skipped: {}",
                    qc.approved(), qc.rejected(),
                    qc.entries.iter().filter(|e| e.verdict == Some(Verdict::Skip)).count()),
                Style::default().fg(TEXT),
            )),
            Line::from(""),
            Line::from(Span::styled("Press Enter to save results, Esc to go back", Style::default().fg(MUTED))),
        ])
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(SURFACE)).style(Style::default().bg(BG)));
        f.render_widget(summary, chunks[2]);
    } else {
        let visible_height = chunks[2].height as usize;
        let start = qc.current.saturating_sub(visible_height / 2);

        let visible_items: Vec<ListItem> = qc.entries.iter().enumerate()
            .skip(start)
            .take(visible_height)
            .map(|(i, entry)| {
                let (marker, style) = if i == qc.current {
                    (">> ", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))
                } else if let Some(v) = entry.verdict {
                    match v {
                        Verdict::Approve => ("[+] ", Style::default().fg(APPROVE_GREEN)),
                        Verdict::Reject => ("[-] ", Style::default().fg(REJECT_RED)),
                        Verdict::Skip => ("[~] ", Style::default().fg(MUTED)),
                    }
                } else {
                    ("    ", Style::default().fg(TEXT))
                };
                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(&entry.label, style),
                ]))
            })
            .collect();

        let scrolled_list = List::new(visible_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(SURFACE))
                    .title(Span::styled(
                        format!(" {} ", qc.entries[qc.current].path.display()),
                        Style::default().fg(MUTED),
                    ))
                    .style(Style::default().bg(BG)),
            );
        f.render_widget(scrolled_list, chunks[2]);
    }

    // Controls
    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" A/Left ", Style::default().fg(Color::White).bg(REJECT_RED).add_modifier(Modifier::BOLD)),
        Span::styled(" Reject  ", Style::default().fg(TEXT)),
        Span::styled(" S/Down ", Style::default().fg(Color::White).bg(SURFACE).add_modifier(Modifier::BOLD)),
        Span::styled(" Skip  ", Style::default().fg(TEXT)),
        Span::styled(" D/Right ", Style::default().fg(Color::White).bg(APPROVE_GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(" Approve  ", Style::default().fg(TEXT)),
        Span::styled(" Esc ", Style::default().fg(MUTED)),
        Span::styled(" Back", Style::default().fg(MUTED)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(SURFACE)).style(Style::default().bg(SURFACE)));
    f.render_widget(controls, chunks[3]);

    // Status
    let status = Paragraph::new(Line::from(Span::styled(
        format!("Visual QC — {}", qc.root.display()),
        Style::default().fg(MUTED),
    )))
    .style(Style::default().bg(SURFACE));
    f.render_widget(status, chunks[4]);
}
