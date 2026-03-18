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
#[derive(PartialEq, Debug)]
pub enum T201 {
    Chat,
    T203,
}

/// Visual QC verdict.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum T202 {
    Approve,
    Reject,
    Skip,
}

/// One image file in the QC queue.
struct QcEntry {
    path: PathBuf,
    label: String,
    verdict: Option<T202>,
}

/// Visual QC state (terminal version of sprite_qc).
pub struct T203 {
    entries: Vec<QcEntry>,
    current: usize,
    root: PathBuf,
}

impl T203 {
    pub fn scan(root: &Path) -> Self {
        let mut entries = Vec::new();
        collect_images(root, root, &mut entries);
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Self {
            entries,
            current: 0,
            root: root.to_path_buf(),
        }
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }
    pub fn approved(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == Some(T202::Approve)).count()
    }
    pub fn rejected(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == Some(T202::Reject)).count()
    }
    pub fn remaining(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict.is_none()).count()
    }
    pub fn is_done(&self) -> bool {
        self.current >= self.entries.len()
    }

    pub fn decide(&mut self, v: T202) {
        if self.current < self.entries.len() {
            self.entries[self.current].verdict = Some(v);
            self.current += 1;
        }
    }

    pub fn apply_verdicts(&self) -> (usize, usize) {
        let approved_dir = self.root.join("approved");
        let rejected_dir = self.root.join("rejected");
        let _ = std::fs::create_dir_all(&approved_dir);
        let _ = std::fs::create_dir_all(&rejected_dir);

        let (mut a, mut r) = (0, 0);
        for entry in &self.entries {
            let rel = entry.path.strip_prefix(&self.root).unwrap_or(&entry.path);
            match entry.verdict {
                Some(T202::Approve) => {
                    let dest = approved_dir.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&entry.path, &dest);
                    a += 1;
                }
                Some(T202::Reject) => {
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

/// Message type for richer rendering.
#[derive(Clone, PartialEq, Debug)]
pub enum T204 {
    User,
    Assistant,
    System,
    ToolCall { tool: String },
    ToolResult { tool: String, success: bool },
    CodeBlock { lang: String },
}

/// Main TUI app state.
struct App {
    mode: T201,
    input: String,
    cursor_pos: usize,
    messages: Vec<ChatMessage>,
    scroll: u16,
    project_dir: PathBuf,
    model_path: Option<PathBuf>,
    system_prompt: String,
    qc: Option<T203>,
    status: String,
    running: bool,
    thinking: bool,
    tick: usize,
    show_help: bool,
    history: Vec<String>,
    history_pos: Option<usize>,
}

struct ChatMessage {
    kind: T204,
    content: String,
    timestamp: String,
}

impl ChatMessage {
    fn new(kind: T204, content: String) -> Self {
        Self {
            kind,
            content,
            timestamp: format_time_now(),
        }
    }
}

fn format_time_now() -> String {
    // Use libc localtime for HH:MM without pulling in chrono.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = ((secs % 86400) / 3600) as u32;
    let mins = ((secs % 3600) / 60) as u32;
    // UTC offset — approximate with env TZ. Good enough for display.
    format!("{:02}:{:02}", hours, mins)
}

const SPINNER_FRAMES: &[&str] = &["   ", ".  ", ".. ", "...", " ..", "  .", "   "];
const LOGO: &str = r#"
  ██╗  ██╗ ██████╗ ██╗   ██╗ █████╗
  ██║ ██╔╝██╔═══██╗██║   ██║██╔══██╗
  █████╔╝ ██║   ██║██║   ██║███████║
  ██╔═██╗ ██║   ██║╚██╗ ██╔╝██╔══██║
  ██║  ██╗╚██████╔╝ ╚████╔╝ ██║  ██║
  ╚═╝  ╚═╝ ╚═════╝   ╚═══╝  ╚═╝  ╚═╝
"#;

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

        let model_display = model_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "none".into());

        let status = format!(
            " {} | {} | Ctrl+C exit | /help",
            project_name, model_display,
        );

        #[cfg(feature = "inference")]
        let system_prompt = crate::repl::f139(&project_dir);
        #[cfg(not(feature = "inference"))]
        let system_prompt = String::new();

        // Build welcome message.
        let mut welcome = String::new();
        welcome.push_str(&format!("project  {}\n", project_dir.display()));
        welcome.push_str(&format!("model    {}\n", model_display));

        // Git info
        if let Ok(branch) = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&project_dir)
            .output()
            && branch.status.success()
        {
            let b = String::from_utf8_lossy(&branch.stdout).trim().to_string();
            welcome.push_str(&format!("branch   {}\n", b));
        }
        welcome.push_str("\nCommands: /clear /qc /tools /project <path> /help /quit");

        let messages = vec![ChatMessage::new(T204::System, welcome)];

        Self {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages,
            scroll: 0,
            project_dir,
            model_path,
            system_prompt,
            qc: None,
            status,
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        }
    }

    fn submit_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Store in history.
        self.history.push(input.clone());
        self.history_pos = None;
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
        if input == "/help" {
            self.show_help = !self.show_help;
            return;
        }
        if input.starts_with("/project ") {
            let p = input.strip_prefix("/project ").unwrap().trim();
            let path = PathBuf::from(p);
            if path.exists() {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.project_dir = path;
                self.status = format!(" {} | Ctrl+C exit | /help", name);
                self.messages.push(ChatMessage::new(
                    T204::System,
                    format!("project switched to {}", p),
                ));
            } else {
                self.messages.push(ChatMessage::new(
                    T204::System,
                    format!("not found: {}", p),
                ));
            }
            return;
        }
        if input == "/tools" {
            let mut lines = String::from("Available tools:\n");
            for tool in crate::tools::TOOLS {
                lines.push_str(&format!("  {} — {}\n", tool.name, tool.description));
            }
            self.messages.push(ChatMessage::new(T204::System, lines));
            return;
        }

        // Chat message.
        self.messages.push(ChatMessage::new(T204::User, input.clone()));
        self.thinking = true;

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

                // Parse response for tool calls and code blocks.
                self.parse_and_push_response(&response);
            } else {
                self.messages.push(ChatMessage::new(
                    T204::System,
                    "No model loaded. Run: kova model install".into(),
                ));
            }
        }
        #[cfg(not(feature = "inference"))]
        {
            self.messages.push(ChatMessage::new(
                T204::System,
                "Inference not available. Build with --features inference".into(),
            ));
        }
        self.thinking = false;
        // Auto-scroll to bottom.
        self.scroll = u16::MAX;
    }

    /// Parse response into structured message blocks.
    fn parse_and_push_response(&mut self, response: &str) {
        let mut current_text = String::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_content = String::new();

        for line in response.lines() {
            if line.starts_with("```") && !in_code_block {
                // Flush text before code block.
                if !current_text.trim().is_empty() {
                    self.messages.push(ChatMessage::new(
                        T204::Assistant,
                        current_text.trim().to_string(),
                    ));
                    current_text.clear();
                }
                in_code_block = true;
                code_lang = line.trim_start_matches('`').trim().to_string();
                code_content.clear();
            } else if line.starts_with("```") && in_code_block {
                // End code block.
                self.messages.push(ChatMessage::new(
                    T204::CodeBlock { lang: code_lang.clone() },
                    code_content.trim_end().to_string(),
                ));
                in_code_block = false;
                code_lang.clear();
            } else if in_code_block {
                code_content.push_str(line);
                code_content.push('\n');
            } else {
                current_text.push_str(line);
                current_text.push('\n');
            }
        }

        // Flush remaining text.
        if !current_text.trim().is_empty() {
            self.messages.push(ChatMessage::new(
                T204::Assistant,
                current_text.trim().to_string(),
            ));
        }
    }

    fn open_visual_qc(&mut self) {
        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screenshots")
            .join(std::env::consts::OS)
            .join("kova");

        if cache.is_dir() {
            let qc = T203::scan(&cache);
            if qc.total() == 0 {
                self.messages.push(ChatMessage::new(
                    T204::System,
                    format!("No images found in {}", cache.display()),
                ));
            } else {
                let count = qc.total();
                self.qc = Some(qc);
                self.mode = T201::T203;
                self.messages.push(ChatMessage::new(
                    T204::System,
                    format!("Visual QC: {} images loaded from {}", count, cache.display()),
                ));
            }
        } else {
            self.messages.push(ChatMessage::new(
                T204::System,
                format!("No screenshots dir: {}", cache.display()),
            ));
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
        app.tick = app.tick.wrapping_add(1);
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match app.mode {
                T201::Chat => handle_chat_key(&mut app, key.code, key.modifiers),
                T201::T203 => handle_qc_key(&mut app, key.code),
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
            if modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Up = scroll
                if app.scroll > 0 {
                    app.scroll = app.scroll.saturating_sub(1);
                }
            } else {
                // Up = history navigation
                if !app.history.is_empty() {
                    let pos = match app.history_pos {
                        None => app.history.len() - 1,
                        Some(p) if p > 0 => p - 1,
                        Some(p) => p,
                    };
                    app.history_pos = Some(pos);
                    app.input = app.history[pos].clone();
                    app.cursor_pos = app.input.len();
                }
            }
        }
        KeyCode::Down => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                app.scroll = app.scroll.saturating_add(1);
            } else if let Some(pos) = app.history_pos {
                if pos + 1 < app.history.len() {
                    app.history_pos = Some(pos + 1);
                    app.input = app.history[pos + 1].clone();
                    app.cursor_pos = app.input.len();
                } else {
                    app.history_pos = None;
                    app.input.clear();
                    app.cursor_pos = 0;
                }
            }
        }
        KeyCode::Esc => {
            if app.show_help {
                app.show_help = false;
            } else if app.mode == T201::T203 {
                app.mode = T201::Chat;
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
                qc.decide(T202::Approve);
            }
        }
        // Reject
        KeyCode::Char('a') | KeyCode::Left => {
            if let Some(ref mut qc) = app.qc {
                qc.decide(T202::Reject);
            }
        }
        // Skip
        KeyCode::Char('s') | KeyCode::Down => {
            if let Some(ref mut qc) = app.qc {
                qc.decide(T202::Skip);
            }
        }
        // Save results
        KeyCode::Enter => {
            if let Some(ref qc) = app.qc
                && qc.is_done()
            {
                let (a, r) = qc.apply_verdicts();
                app.messages.push(ChatMessage::new(
                    T204::System,
                    format!("Saved: {} approved, {} rejected", a, r),
                ));
                app.qc = None;
                app.mode = T201::Chat;
            }
        }
        // Exit QC
        KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = T201::Chat;
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
        T201::Chat => draw_chat(f, app, size),
        T201::T203 => draw_visual_qc(f, app, size),
    }
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    // Help overlay.
    if app.show_help {
        draw_help(f, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(5),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Header — KOVA badge + mode tabs.
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" KOVA ", Style::default().fg(BG).bg(PRIMARY).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled(" Chat ", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        Span::styled("  ", Style::default()),
        Span::styled(" QC ", Style::default().fg(MUTED)),
        Span::styled("  ", Style::default()),
        Span::styled(" Help ", Style::default().fg(MUTED)),
        Span::raw("  "),
        Span::styled(
            if app.thinking {
                format!(" thinking{} ", SPINNER_FRAMES[app.tick / 3 % SPINNER_FRAMES.len()])
            } else {
                String::new()
            },
            Style::default().fg(SECONDARY),
        ),
    ]))
    .block(Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(SURFACE))
        .style(Style::default().bg(BG)));
    f.render_widget(header, chunks[0]);

    // Messages area.
    let msg_area = chunks[1];
    let mut lines: Vec<Line> = Vec::new();

    if app.messages.is_empty() {
        // Empty state with logo.
        for line in LOGO.lines() {
            lines.push(Line::from(Span::styled(line, Style::default().fg(PRIMARY))));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Augment engine. Type to begin.",
            Style::default().fg(MUTED),
        )));
    }

    for msg in &app.messages {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        match &msg.kind {
            T204::User => {
                lines.push(Line::from(vec![
                    Span::styled(&msg.timestamp, Style::default().fg(MUTED)),
                    Span::styled(" > ", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled(&msg.content, Style::default().fg(PRIMARY)),
                ]));
            }
            T204::Assistant => {
                lines.push(Line::from(vec![
                    Span::styled(&msg.timestamp, Style::default().fg(MUTED)),
                    Span::styled("   ", Style::default()),
                ]));
                for line in msg.content.lines() {
                    // Bold detection for **text**.
                    if line.contains("**") {
                        let mut spans = vec![Span::raw("   ")];
                        let mut rest = line;
                        while let Some(start) = rest.find("**") {
                            if start > 0 {
                                spans.push(Span::styled(&rest[..start], Style::default().fg(TEXT)));
                            }
                            rest = &rest[start + 2..];
                            if let Some(end) = rest.find("**") {
                                spans.push(Span::styled(
                                    &rest[..end],
                                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                                ));
                                rest = &rest[end + 2..];
                            }
                        }
                        if !rest.is_empty() {
                            spans.push(Span::styled(rest, Style::default().fg(TEXT)));
                        }
                        lines.push(Line::from(spans));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("   "),
                            Span::styled(line, Style::default().fg(TEXT)),
                        ]));
                    }
                }
            }
            T204::System => {
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("   ", Style::default()),
                        Span::styled(line, Style::default().fg(MUTED)),
                    ]));
                }
            }
            T204::ToolCall { tool } => {
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(" ", Style::default().bg(TERTIARY)),
                    Span::styled(
                        format!(" {} ", tool),
                        Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                    ),
                ]));
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("   ", Style::default()),
                        Span::styled(" ", Style::default().bg(TERTIARY)),
                        Span::styled(format!(" {}", line), Style::default().fg(MUTED)),
                    ]));
                }
            }
            T204::ToolResult { tool, success } => {
                let (marker, color) = if *success {
                    (" + ", APPROVE_GREEN)
                } else {
                    (" x ", REJECT_RED)
                };
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(marker, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{} result", tool),
                        Style::default().fg(color),
                    ),
                ]));
                for line in msg.content.lines().take(10) {
                    lines.push(Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(line, Style::default().fg(MUTED)),
                    ]));
                }
                if msg.content.lines().count() > 10 {
                    lines.push(Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(
                            format!("... ({} more lines)", msg.content.lines().count() - 10),
                            Style::default().fg(MUTED),
                        ),
                    ]));
                }
            }
            T204::CodeBlock { lang } => {
                let lang_label = if lang.is_empty() { "code" } else { lang };
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        format!("  {} ", lang_label),
                        Style::default().fg(BG).bg(SECONDARY),
                    ),
                    Span::styled(
                        " ".repeat(area.width.saturating_sub(8 + lang_label.len() as u16) as usize),
                        Style::default().bg(Color::Rgb(0x1a, 0x1a, 0x2e)),
                    ),
                ]));
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("   ", Style::default()),
                        Span::styled(
                            format!("  {}", line),
                            Style::default()
                                .fg(Color::Rgb(0xc0, 0xc0, 0xd0))
                                .bg(Color::Rgb(0x1a, 0x1a, 0x2e)),
                        ),
                    ]));
                }
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        " ".repeat(area.width.saturating_sub(4) as usize),
                        Style::default().bg(Color::Rgb(0x1a, 0x1a, 0x2e)),
                    ),
                ]));
            }
        }
    }

    // Thinking indicator at bottom.
    if app.thinking {
        lines.push(Line::from(""));
        let spinner = SPINNER_FRAMES[app.tick / 3 % SPINNER_FRAMES.len()];
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("thinking{}", spinner),
                Style::default().fg(SECONDARY).add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    // Scroll calculation.
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

    // Input box.
    let border_color = if app.thinking {
        SECONDARY
    } else if app.input.is_empty() {
        SURFACE
    } else {
        PRIMARY
    };
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            if app.thinking { " waiting... " } else { " augment " },
            Style::default().fg(border_color),
        ))
        .style(Style::default().bg(SURFACE));

    let display_input = if app.input.is_empty() && !app.thinking {
        Span::styled("type a message...", Style::default().fg(MUTED))
    } else {
        Span::styled(&app.input, Style::default().fg(TEXT))
    };

    let input = Paragraph::new(Line::from(display_input)).block(input_block);
    f.render_widget(input, chunks[2]);

    // Cursor.
    if !app.thinking {
        f.set_cursor_position((
            chunks[2].x + 1 + app.cursor_pos as u16,
            chunks[2].y + 1,
        ));
    }

    // Status bar.
    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.status, Style::default().fg(MUTED)),
        Span::styled(
            format!("  {} msgs", app.messages.len()),
            Style::default().fg(MUTED),
        ),
    ]))
    .style(Style::default().bg(SURFACE));
    f.render_widget(status, chunks[3]);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PRIMARY))
        .title(Span::styled(" KOVA Help ", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(BG));

    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled("  Commands", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("    /clear     ", Style::default().fg(TERTIARY)),
            Span::styled("Clear chat history", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    /qc        ", Style::default().fg(TERTIARY)),
            Span::styled("Open Visual QC (screenshot review)", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    /tools     ", Style::default().fg(TERTIARY)),
            Span::styled("List available tools", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    /project   ", Style::default().fg(TERTIARY)),
            Span::styled("Switch project directory", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    /help      ", Style::default().fg(TERTIARY)),
            Span::styled("Toggle this help panel", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    /quit      ", Style::default().fg(TERTIARY)),
            Span::styled("Exit (also /exit, /q)", Style::default().fg(TEXT)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Keybindings", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Ctrl+C     ", Style::default().fg(TERTIARY)),
            Span::styled("Exit", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    Up/Down    ", Style::default().fg(TERTIARY)),
            Span::styled("Input history", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    Shift+Up   ", Style::default().fg(TERTIARY)),
            Span::styled("Scroll messages", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    Home/End   ", Style::default().fg(TERTIARY)),
            Span::styled("Cursor to start/end", Style::default().fg(TEXT)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Visual QC Mode", Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("    D / Right  ", Style::default().fg(APPROVE_GREEN)),
            Span::styled("Approve", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    A / Left   ", Style::default().fg(REJECT_RED)),
            Span::styled("Reject", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    S / Down   ", Style::default().fg(MUTED)),
            Span::styled("Skip", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    Enter      ", Style::default().fg(TERTIARY)),
            Span::styled("Save results", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("    Esc        ", Style::default().fg(TERTIARY)),
            Span::styled("Back to chat", Style::default().fg(TEXT)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Press /help or Esc to close", Style::default().fg(MUTED))),
    ];

    let help = Paragraph::new(help_text).block(block);
    f.render_widget(help, area);
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
                    qc.entries.iter().filter(|e| e.verdict == Some(T202::Skip)).count()),
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
                        T202::Approve => ("[+] ", Style::default().fg(APPROVE_GREEN)),
                        T202::Reject => ("[-] ", Style::default().fg(REJECT_RED)),
                        T202::Skip => ("[~] ", Style::default().fg(MUTED)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_images(dir: &Path) -> Vec<PathBuf> {
        let png_data: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let mut paths = Vec::new();
        for name in ["alpha.png", "beta.png", "gamma.png"] {
            let p = dir.join(name);
            fs::write(&p, png_data).unwrap();
            paths.push(p);
        }
        let sub = dir.join("zone01");
        fs::create_dir_all(&sub).unwrap();
        for name in ["bg.png", "fg.png"] {
            let p = sub.join(name);
            fs::write(&p, png_data).unwrap();
            paths.push(p);
        }
        paths
    }

    #[test]
    fn visual_qc_scan_finds_pngs() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let qc = T203::scan(tmp.path());
        assert_eq!(qc.total(), 5);
        assert_eq!(qc.approved(), 0);
        assert_eq!(qc.rejected(), 0);
        assert_eq!(qc.remaining(), 5);
        assert!(!qc.is_done());
    }

    #[test]
    fn visual_qc_scan_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let qc = T203::scan(tmp.path());
        assert_eq!(qc.total(), 0);
        assert!(qc.is_done());
    }

    #[test]
    fn visual_qc_scan_skips_approved_rejected_dirs() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let approved = tmp.path().join("approved");
        fs::create_dir_all(&approved).unwrap();
        fs::write(approved.join("old.png"), b"fake").unwrap();
        let rejected = tmp.path().join("rejected");
        fs::create_dir_all(&rejected).unwrap();
        fs::write(rejected.join("old.png"), b"fake").unwrap();

        let qc = T203::scan(tmp.path());
        assert_eq!(qc.total(), 5);
    }

    #[test]
    fn visual_qc_decide_advances() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut qc = T203::scan(tmp.path());

        assert_eq!(qc.current, 0);
        qc.decide(T202::Approve);
        assert_eq!(qc.current, 1);
        assert_eq!(qc.approved(), 1);

        qc.decide(T202::Reject);
        assert_eq!(qc.current, 2);
        assert_eq!(qc.rejected(), 1);

        qc.decide(T202::Skip);
        assert_eq!(qc.current, 3);
        assert_eq!(qc.remaining(), 2);
    }

    #[test]
    fn visual_qc_decide_all_marks_done() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut qc = T203::scan(tmp.path());

        for _ in 0..5 {
            qc.decide(T202::Approve);
        }
        assert!(qc.is_done());
        assert_eq!(qc.approved(), 5);
        assert_eq!(qc.remaining(), 0);
    }

    #[test]
    fn visual_qc_decide_past_end_is_noop() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut qc = T203::scan(tmp.path());

        for _ in 0..10 {
            qc.decide(T202::Approve);
        }
        assert!(qc.is_done());
        assert_eq!(qc.current, 5);
    }

    #[test]
    fn visual_qc_apply_verdicts_copies_files() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut qc = T203::scan(tmp.path());

        qc.decide(T202::Approve);
        qc.decide(T202::Reject);
        qc.decide(T202::Skip);
        qc.decide(T202::Approve);
        qc.decide(T202::Reject);

        let (a, r) = qc.apply_verdicts();
        assert_eq!(a, 2);
        assert_eq!(r, 2);
        assert!(tmp.path().join("approved").is_dir());
        assert!(tmp.path().join("rejected").is_dir());
    }

    #[test]
    fn collect_images_finds_multiple_formats() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.png"), b"fake").unwrap();
        fs::write(tmp.path().join("b.jpg"), b"fake").unwrap();
        fs::write(tmp.path().join("c.jpeg"), b"fake").unwrap();
        fs::write(tmp.path().join("d.svg"), b"fake").unwrap();
        fs::write(tmp.path().join("e.txt"), b"not an image").unwrap();

        let mut entries = Vec::new();
        collect_images(tmp.path(), tmp.path(), &mut entries);
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn collect_images_nonexistent_dir() {
        let mut entries = Vec::new();
        collect_images(Path::new("/nonexistent"), Path::new("/nonexistent"), &mut entries);
        assert!(entries.is_empty());
    }

    #[test]
    fn verdict_equality() {
        assert_eq!(T202::Approve, T202::Approve);
        assert_ne!(T202::Approve, T202::Reject);
        assert_ne!(T202::Reject, T202::Skip);
    }

    #[test]
    fn theme_colors_are_rgb() {
        let colors = [PRIMARY, SECONDARY, TERTIARY, TEXT, MUTED, BG, SURFACE, APPROVE_GREEN, REJECT_RED];
        for c in colors {
            match c {
                Color::Rgb(_, _, _) => {}
                _ => panic!("Expected RGB color"),
            }
        }
    }

    #[test]
    fn primary_is_cyan() {
        assert_eq!(PRIMARY, Color::Rgb(0x00, 0xd4, 0xff));
    }

    #[test]
    fn handle_qc_key_approve_reject_skip() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut app = App {
            mode: T201::T203,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: tmp.path().to_path_buf(),
            model_path: None,
            system_prompt: String::new(),
            qc: Some(T203::scan(tmp.path())),
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        handle_qc_key(&mut app, KeyCode::Char('d'));
        assert_eq!(app.qc.as_ref().unwrap().approved(), 1);
        handle_qc_key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.qc.as_ref().unwrap().rejected(), 1);
        handle_qc_key(&mut app, KeyCode::Char('s'));
        assert_eq!(app.qc.as_ref().unwrap().current, 3);
    }

    #[test]
    fn handle_qc_key_esc_returns_to_chat() {
        let tmp = TempDir::new().unwrap();
        make_test_images(tmp.path());
        let mut app = App {
            mode: T201::T203,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: tmp.path().to_path_buf(),
            model_path: None,
            system_prompt: String::new(),
            qc: Some(T203::scan(tmp.path())),
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        handle_qc_key(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, T201::Chat);
    }

    #[test]
    fn handle_chat_key_typing() {
        let mut app = App {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        handle_chat_key(&mut app, KeyCode::Char('h'), KeyModifiers::empty());
        handle_chat_key(&mut app, KeyCode::Char('i'), KeyModifiers::empty());
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_pos, 2);

        handle_chat_key(&mut app, KeyCode::Backspace, KeyModifiers::empty());
        assert_eq!(app.input, "h");
        assert_eq!(app.cursor_pos, 1);
    }

    #[test]
    fn handle_chat_key_ctrl_c_exits() {
        let mut app = App {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        handle_chat_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.running);
    }

    #[test]
    fn submit_input_quit_commands() {
        for cmd in ["/quit", "/exit", "/q"] {
            let mut app = App {
                mode: T201::Chat,
                input: cmd.to_string(),
                cursor_pos: cmd.len(),
                messages: Vec::new(),
                scroll: 0,
                project_dir: PathBuf::from("."),
                model_path: None,
                system_prompt: String::new(),
                qc: None,
                status: String::new(),
                running: true,
                thinking: false,
                tick: 0,
                show_help: false,
                history: Vec::new(),
                history_pos: None,
            };
            app.submit_input();
            assert!(!app.running);
        }
    }

    #[test]
    fn submit_input_clear() {
        let mut app = App {
            mode: T201::Chat,
            input: "/clear".to_string(),
            cursor_pos: 6,
            messages: vec![ChatMessage::new(T204::User, "old".into())],
            scroll: 5,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };
        app.submit_input();
        assert!(app.messages.is_empty());
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn submit_input_empty_is_noop() {
        let mut app = App {
            mode: T201::Chat,
            input: "   ".to_string(),
            cursor_pos: 3,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };
        app.submit_input();
        assert!(app.messages.is_empty());
    }

    #[test]
    fn submit_input_tools_command() {
        let mut app = App {
            mode: T201::Chat,
            input: "/tools".to_string(),
            cursor_pos: 6,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };
        app.submit_input();
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].kind, T204::System);
        assert!(app.messages[0].content.contains("read_file"));
    }

    #[test]
    fn cursor_movement_boundaries() {
        let mut app = App {
            mode: T201::Chat,
            input: "abc".to_string(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        handle_chat_key(&mut app, KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.cursor_pos, 0);

        handle_chat_key(&mut app, KeyCode::Right, KeyModifiers::empty());
        assert_eq!(app.cursor_pos, 1);

        handle_chat_key(&mut app, KeyCode::End, KeyModifiers::empty());
        assert_eq!(app.cursor_pos, 3);

        handle_chat_key(&mut app, KeyCode::Right, KeyModifiers::empty());
        assert_eq!(app.cursor_pos, 3);

        app.cursor_pos = 0;
        handle_chat_key(&mut app, KeyCode::Backspace, KeyModifiers::empty());
        assert_eq!(app.input, "abc");

        app.cursor_pos = 3;
        handle_chat_key(&mut app, KeyCode::Delete, KeyModifiers::empty());
        assert_eq!(app.input, "abc");

        app.cursor_pos = 1;
        handle_chat_key(&mut app, KeyCode::Delete, KeyModifiers::empty());
        assert_eq!(app.input, "ac");
    }

    #[test]
    fn handle_chat_key_scroll() {
        let mut app = App {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 5,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        // Shift+Up/Down = scroll
        handle_chat_key(&mut app, KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(app.scroll, 4);
        handle_chat_key(&mut app, KeyCode::Down, KeyModifiers::SHIFT);
        assert_eq!(app.scroll, 5);
    }

    #[test]
    fn input_history_navigation() {
        let mut app = App {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: vec!["first".into(), "second".into()],
            history_pos: None,
        };

        // Up = go to last history item
        handle_chat_key(&mut app, KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.input, "second");
        assert_eq!(app.history_pos, Some(1));

        // Up again = go to first
        handle_chat_key(&mut app, KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.input, "first");
        assert_eq!(app.history_pos, Some(0));

        // Down = back to second
        handle_chat_key(&mut app, KeyCode::Down, KeyModifiers::empty());
        assert_eq!(app.input, "second");

        // Down again = clear input
        handle_chat_key(&mut app, KeyCode::Down, KeyModifiers::empty());
        assert!(app.input.is_empty());
        assert_eq!(app.history_pos, None);
    }

    #[test]
    fn parse_and_push_response_splits_code_blocks() {
        let mut app = App {
            mode: T201::Chat,
            input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            scroll: 0,
            project_dir: PathBuf::from("."),
            model_path: None,
            system_prompt: String::new(),
            qc: None,
            status: String::new(),
            running: true,
            thinking: false,
            tick: 0,
            show_help: false,
            history: Vec::new(),
            history_pos: None,
        };

        app.parse_and_push_response("Here is some code:\n```rust\nfn main() {}\n```\nDone.");
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[0].kind, T204::Assistant);
        assert!(matches!(&app.messages[1].kind, T204::CodeBlock { lang } if lang == "rust"));
        assert_eq!(app.messages[1].content, "fn main() {}");
        assert_eq!(app.messages[2].kind, T204::Assistant);
        assert_eq!(app.messages[2].content, "Done.");
    }

    #[test]
    fn msg_kind_equality() {
        assert_eq!(T204::User, T204::User);
        assert_ne!(T204::User, T204::Assistant);
        assert_eq!(
            T204::ToolCall { tool: "bash".into() },
            T204::ToolCall { tool: "bash".into() }
        );
        assert_ne!(
            T204::ToolResult { tool: "bash".into(), success: true },
            T204::ToolResult { tool: "bash".into(), success: false },
        );
    }
}
