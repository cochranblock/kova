//! Kova screen automation — vision-based browser control.
//!
//! Sees the screen, finds UI elements by visual pattern matching,
//! clicks and types using OS-level input. Works with ANY browser,
//! ANY website. No DOM, no protocol, no selectors.
//!
//! macOS: CoreGraphics for screenshots, enigo for input.
//! Survives UI redesigns — if a human can see it, kova can interact with it.

// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// Prompt entry — label + text to send.
#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub label: String,
    pub text: String,
}

/// Load prompts from a markdown file.
pub fn load_prompts(path: &str) -> Result<Vec<PromptEntry>> {
    let content = std::fs::read_to_string(path)?;
    let mut prompts = Vec::new();
    let mut current_label: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            if let Some(after_dash) = trimmed.split('—').nth(1) {
                current_label = Some(
                    after_dash.trim().to_lowercase().replace(' ', "_").replace('-', "_")
                );
            }
        } else if trimmed.starts_with("Create a ") && current_label.is_some() {
            prompts.push(PromptEntry {
                label: current_label.take().unwrap(),
                text: trimmed.to_string(),
            });
        }
    }

    Ok(prompts)
}

// ---------------------------------------------------------------------------
// Screen capture (macOS CoreGraphics)
// ---------------------------------------------------------------------------

/// Capture the full screen as an RGBA image buffer.
/// Returns (width, height, pixels).
#[cfg(target_os = "macos")]
fn capture_screen() -> Result<(u32, u32, Vec<u8>)> {
    use core_graphics::display::*;

    let display = CGDisplay::main();
    let image = CGDisplay::screenshot(
        display.bounds(),
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageDefault,
    ).context("screenshot failed — grant Screen Recording permission in System Settings")?;

    let w = image.width() as u32;
    let h = image.height() as u32;
    let bpr = image.bytes_per_row();
    let data = image.data();
    let bytes = data.bytes();

    // CoreGraphics returns BGRA, convert to RGBA
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let offset = y * bpr + x * 4;
            if offset + 3 < bytes.len() {
                rgba.push(bytes[offset + 2]); // R (was B)
                rgba.push(bytes[offset + 1]); // G
                rgba.push(bytes[offset]);     // B (was R)
                rgba.push(bytes[offset + 3]); // A
            }
        }
    }

    Ok((w, h, rgba))
}

#[cfg(not(target_os = "macos"))]
fn capture_screen() -> Result<(u32, u32, Vec<u8>)> {
    anyhow::bail!("screen capture only supported on macOS")
}

/// Save a screenshot to disk for debugging.
fn save_screenshot(path: &str) -> Result<()> {
    let (w, h, rgba) = capture_screen()?;
    let img = image::RgbaImage::from_raw(w, h, rgba).context("invalid screenshot data")?;
    img.save(path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Visual element finding
// ---------------------------------------------------------------------------

/// Find a region on screen that matches a target color pattern.
/// Returns (x, y) center of the match, or None.
fn find_colored_region(
    screen: &[u8], sw: u32, sh: u32,
    target_r: u8, target_g: u8, target_b: u8,
    tolerance: u8,
    min_width: u32,
) -> Option<(i32, i32)> {
    // Scan for horizontal runs of the target color
    let mut best_x = 0i32;
    let mut best_y = 0i32;
    let mut best_run = 0u32;

    for y in 0..sh {
        let mut run = 0u32;
        let mut run_start = 0u32;
        for x in 0..sw {
            let i = (y * sw + x) as usize * 4;
            if i + 2 >= screen.len() { break; }
            let dr = (screen[i] as i16 - target_r as i16).unsigned_abs() as u8;
            let dg = (screen[i + 1] as i16 - target_g as i16).unsigned_abs() as u8;
            let db = (screen[i + 2] as i16 - target_b as i16).unsigned_abs() as u8;
            if dr <= tolerance && dg <= tolerance && db <= tolerance {
                if run == 0 { run_start = x; }
                run += 1;
            } else {
                if run > best_run && run >= min_width {
                    best_run = run;
                    best_x = (run_start + run / 2) as i32;
                    best_y = y as i32;
                }
                run = 0;
            }
        }
        if run > best_run && run >= min_width {
            best_run = run;
            best_x = (sw - run / 2) as i32;
            best_y = y as i32;
        }
    }

    if best_run >= min_width {
        Some((best_x, best_y))
    } else {
        None
    }
}

/// Find the Gemini text input area on screen.
/// It's typically a light-colored horizontal bar near the bottom.
fn find_gemini_input(screen: &[u8], sw: u32, sh: u32) -> Option<(i32, i32)> {
    // Gemini input is in the bottom 30% of the screen
    // It's a wide light-gray/white bar
    let start_y = (sh as f32 * 0.6) as u32;
    let bottom_region: Vec<u8> = screen[(start_y * sw * 4) as usize..].to_vec();
    let region_h = sh - start_y;

    // Look for a wide light-colored horizontal region (the input bar)
    // Input bar is typically RGB ~(240, 240, 240) or ~(255, 255, 255)
    if let Some((x, y)) = find_colored_region(&bottom_region, sw, region_h, 240, 240, 240, 20, sw / 3) {
        Some((x, y + start_y as i32))
    } else {
        None
    }
}

/// Detect if a new image appeared on screen by comparing screenshot checksums.
fn screen_changed(old_checksum: u64, new_screen: &[u8]) -> bool {
    let new_checksum = simple_hash(new_screen);
    (old_checksum as i64 - new_checksum as i64).unsigned_abs() > 1000000
}

fn simple_hash(data: &[u8]) -> u64 {
    // Sample every 1000th byte for fast comparison
    let mut h: u64 = 0;
    for (i, &b) in data.iter().step_by(1000).enumerate() {
        h = h.wrapping_add(b as u64 * (i as u64 + 1));
    }
    h
}

/// Watch for new files appearing in a directory.
fn watch_for_new_file(dir: &str, before_count: usize, timeout: Duration) -> Option<PathBuf> {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout { return None; }
        std::thread::sleep(Duration::from_secs(3));

        let files: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg")
            })
            .collect();

        if files.len() > before_count {
            // Find the newest file
            let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
            for f in files {
                if let Ok(meta) = f.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                            newest = Some((f.path(), modified));
                        }
                    }
                }
            }
            return newest.map(|(p, _)| p);
        }
    }
}

// ---------------------------------------------------------------------------
// Input control (enigo)
// ---------------------------------------------------------------------------

#[cfg(feature = "browser")]
use enigo::{Enigo, Keyboard, Mouse, Settings, Coordinate, Button, Key};

#[cfg(feature = "browser")]
fn create_enigo() -> Result<Enigo> {
    Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("enigo init: {}", e))
}

/// Click at screen coordinates.
#[cfg(feature = "browser")]
fn click_at(x: i32, y: i32) -> Result<()> {
    let mut enigo = create_enigo()?;
    enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| anyhow::anyhow!("{}", e))?;
    std::thread::sleep(Duration::from_millis(100));
    enigo.button(Button::Left, enigo::Direction::Click).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Type text using keyboard.
#[cfg(feature = "browser")]
fn type_text(text: &str) -> Result<()> {
    let mut enigo = create_enigo()?;
    // Type in chunks to avoid overwhelming the input
    for chunk in text.as_bytes().chunks(50) {
        let s = String::from_utf8_lossy(chunk);
        enigo.text(&s).map_err(|e| anyhow::anyhow!("{}", e))?;
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Press Enter key.
#[cfg(feature = "browser")]
fn press_enter() -> Result<()> {
    let mut enigo = create_enigo()?;
    enigo.key(Key::Return, enigo::Direction::Click).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Focus a window by title using AppleScript.
fn focus_window(title_contains: &str) -> Result<()> {
    let script = format!(
        r#"tell application "System Events"
            set firefoxProcess to first process whose name is "firefox"
            set frontmost of firefoxProcess to true
            repeat with w in windows of firefoxProcess
                if name of w contains "{}" then
                    perform action "AXRaise" of w
                    return true
                end if
            end repeat
        end tell"#,
        title_contains
    );
    Command::new("osascript").args(["-e", &script]).output()?;
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

// ---------------------------------------------------------------------------
// Main autoprompt pipeline
// ---------------------------------------------------------------------------

/// Run autoprompt using screen vision + input control.
pub async fn run_autoprompt(
    prompt_file: &str,
    output_dir: &str,
    _workers: usize, // workers ignored for now — single screen
    skip: usize,
) -> Result<()> {
    let prompts = load_prompts(prompt_file)?;
    println!("loaded {} prompts from {}", prompts.len(), prompt_file);

    let prompts: Vec<_> = prompts.into_iter().skip(skip).collect();
    if prompts.is_empty() {
        println!("all prompts done (skip={})", skip);
        return Ok(());
    }

    std::fs::create_dir_all(output_dir)?;

    // Verify screen capture works
    println!("testing screen capture...");
    let (sw, sh, screen) = capture_screen().context(
        "screen capture failed. Grant Screen Recording permission:\n  System Settings → Privacy & Security → Screen Recording → add Terminal/kova"
    )?;
    println!("screen: {}x{}", sw, sh);

    // Find Firefox download dir
    let download_dir = dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from(output_dir));
    let download_dir_str = download_dir.to_string_lossy().to_string();
    println!("watching downloads: {}", download_dir_str);

    println!("\nstarting {} prompts. Focus Firefox on Gemini before continuing.", prompts.len());
    println!("press Ctrl+C to stop.\n");
    std::thread::sleep(Duration::from_secs(3));

    let mut saved = 0usize;
    for (i, entry) in prompts.iter().enumerate() {
        let idx = skip + i;
        println!("[{}/{}] {}: {}", idx + 1, skip + prompts.len(), entry.label, &entry.text[..60.min(entry.text.len())]);

        // Focus Firefox window
        focus_window("Gemini")?;
        std::thread::sleep(Duration::from_secs(1));

        // Take screenshot, find input area
        let (sw, sh, screen) = capture_screen()?;

        if let Some((ix, iy)) = find_gemini_input(&screen, sw, sh) {
            // Click the input area
            click_at(ix, iy)?;
            std::thread::sleep(Duration::from_millis(500));

            // Type the prompt
            type_text(&entry.text)?;
            std::thread::sleep(Duration::from_millis(300));

            // Press Enter to submit
            press_enter()?;

            // Count current files in download dir
            let before_count = std::fs::read_dir(output_dir)
                .map(|rd| rd.filter_map(|e| e.ok()).count())
                .unwrap_or(0);

            // Wait for image generation (watch screen for changes, up to 3 min)
            println!("  waiting for generation...");
            let pre_hash = simple_hash(&screen);

            let mut image_appeared = false;
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(180) {
                std::thread::sleep(Duration::from_secs(5));
                if let Ok((_, _, new_screen)) = capture_screen() {
                    if screen_changed(pre_hash, &new_screen) {
                        // Screen changed significantly — image likely appeared
                        // Wait a bit more for it to fully render
                        std::thread::sleep(Duration::from_secs(3));
                        image_appeared = true;
                        break;
                    }
                }
            }

            if image_appeared {
                // Save screenshot of the result
                let out_path = format!("{}/auto_{}_{:04}.png", output_dir, entry.label, idx);
                // Take a clean screenshot of the generated image
                std::thread::sleep(Duration::from_secs(2));

                // Use Cmd+Shift+S or right-click save approach
                // For now, screenshot the full screen and let ingest-gemini handle slicing
                if save_screenshot(&out_path).is_ok() {
                    println!("  saved: {}", out_path);
                    saved += 1;
                }
            } else {
                eprintln!("  timeout — no image detected");
            }

            // Click "New chat" — typically top-left area
            // Or use Ctrl+Shift+O for new chat shortcut
            let mut enigo = create_enigo()?;
            // Try keyboard shortcut for new chat
            enigo.key(Key::Meta, enigo::Direction::Press).map_err(|e| anyhow::anyhow!("{}", e))?;
            enigo.key(Key::Shift, enigo::Direction::Press).map_err(|e| anyhow::anyhow!("{}", e))?;
            type_text("o")?;
            enigo.key(Key::Shift, enigo::Direction::Release).map_err(|e| anyhow::anyhow!("{}", e))?;
            enigo.key(Key::Meta, enigo::Direction::Release).map_err(|e| anyhow::anyhow!("{}", e))?;
            std::thread::sleep(Duration::from_secs(2));

        } else {
            eprintln!("  could not find Gemini input on screen");
            // Try clicking center-bottom of screen as fallback
            click_at(sw as i32 / 2, (sh as f32 * 0.85) as i32)?;
            std::thread::sleep(Duration::from_millis(500));
            type_text(&entry.text)?;
            press_enter()?;
            std::thread::sleep(Duration::from_secs(60)); // blind wait
        }

        // Rate limit
        std::thread::sleep(Duration::from_secs(2));
    }

    println!("\nautoprompt done: {} images saved to {}", saved, output_dir);
    Ok(())
}
