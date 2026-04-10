//! Kova screen automation — vision-based dual-browser Gemini prompting.
//!
//! Drives two Firefox Gemini windows simultaneously via screen vision.
//! Interleaved pipeline: while one generates, the other gets a new prompt.
//!
//! Flow per window:
//!   1. Focus window → click input → type prompt → Enter
//!   2. Switch to other window while this one generates
//!   3. Come back → hover image → click download button → new chat
//!
//! macOS: CoreGraphics screenshots, enigo input, AppleScript window focus.

// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

use anyhow::Result;
use anyhow::Context;
use std::process::Command;
use std::time::{Duration, Instant};
use std::path::PathBuf;

use enigo::{Enigo, Keyboard, Mouse, Settings, Coordinate, Button, Key, Direction};

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
                    after_dash.trim().to_lowercase().replace([' ', '-'], "_")
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
// Screen capture
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "macos", feature = "browser"))]
fn capture_screen() -> Result<(u32, u32, Vec<u8>)> {
    use core_graphics::display::*;

    let display = CGDisplay::main();
    let image = CGDisplay::screenshot(
        display.bounds(),
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageDefault,
    ).context("screenshot failed — grant Screen Recording permission")?;

    let w = image.width() as u32;
    let h = image.height() as u32;
    let bpr = image.bytes_per_row();
    let data = image.data();
    let bytes = data.bytes();

    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let offset = y * bpr + x * 4;
            if offset + 3 < bytes.len() {
                rgba.push(bytes[offset + 2]); // R
                rgba.push(bytes[offset + 1]); // G
                rgba.push(bytes[offset]);     // B
                rgba.push(bytes[offset + 3]); // A
            }
        }
    }

    Ok((w, h, rgba))
}

#[cfg(all(feature = "browser", not(target_os = "macos")))]
fn capture_screen() -> Result<(u32, u32, Vec<u8>)> {
    anyhow::bail!("screen capture only supported on macOS")
}

fn screen_hash(screen: &[u8]) -> u64 {
    let mut h: u64 = 0;
    for (i, &b) in screen.iter().step_by(997).enumerate() {
        h = h.wrapping_add(b as u64 * (i as u64 + 1));
    }
    h
}

// ---------------------------------------------------------------------------
// Visual finders
// ---------------------------------------------------------------------------

/// Find the center of the largest image on screen.
/// Images are large rectangles of varied color surrounded by UI chrome.
/// We look for a region in the middle ~60% of the screen with high color variance.
fn find_generated_image(screen: &[u8], sw: u32, sh: u32) -> Option<(i32, i32)> {
    // The generated image is usually in the center of the page
    // Scan the middle portion of the screen for a large block of non-uniform pixels
    let y_start = (sh as f32 * 0.15) as u32;
    let y_end = (sh as f32 * 0.75) as u32;
    let x_start = (sw as f32 * 0.1) as u32;
    let x_end = (sw as f32 * 0.9) as u32;

    // Sample blocks and find the one with highest color variance
    let block_size = 64u32;
    let mut best_var = 0.0f64;
    let mut best_x = sw / 2;
    let mut best_y = sh / 2;

    for by in (y_start..y_end).step_by(block_size as usize) {
        for bx in (x_start..x_end).step_by(block_size as usize) {
            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_r2 = 0u64;
            let mut sum_g2 = 0u64;
            let mut sum_b2 = 0u64;
            let mut count = 0u64;

            for dy in 0..block_size.min(y_end - by) {
                for dx in 0..block_size.min(x_end - bx) {
                    let px = bx + dx;
                    let py = by + dy;
                    let i = (py * sw + px) as usize * 4;
                    if i + 2 >= screen.len() { continue; }
                    let r = screen[i] as u64;
                    let g = screen[i + 1] as u64;
                    let b = screen[i + 2] as u64;
                    sum_r += r; sum_g += g; sum_b += b;
                    sum_r2 += r * r; sum_g2 += g * g; sum_b2 += b * b;
                    count += 1;
                }
            }

            if count == 0 { continue; }
            let var_r = (sum_r2 as f64 / count as f64) - (sum_r as f64 / count as f64).powi(2);
            let var_g = (sum_g2 as f64 / count as f64) - (sum_g as f64 / count as f64).powi(2);
            let var_b = (sum_b2 as f64 / count as f64) - (sum_b as f64 / count as f64).powi(2);
            let total_var = var_r + var_g + var_b;

            if total_var > best_var {
                best_var = total_var;
                best_x = bx + block_size / 2;
                best_y = by + block_size / 2;
            }
        }
    }

    // Only return if variance is meaningful (actual image, not solid background)
    if best_var > 500.0 {
        Some((best_x as i32, best_y as i32))
    } else {
        None
    }
}

/// After hovering over image, find the download button.
/// It's a small icon that appears on hover — typically a down-arrow,
/// lighter/darker than the image. We look for small UI elements that
/// appeared after the hover near the cursor position.
fn find_download_button(
    before: &[u8], after: &[u8],
    sw: u32, sh: u32,
    hover_x: i32, hover_y: i32,
) -> Option<(i32, i32)> {
    // Compare before/after hover to find newly appeared elements
    // Search in a region around the image (especially bottom-right, bottom-center)
    let search_radius = 200i32;
    let x_start = (hover_x - search_radius).max(0) as u32;
    let x_end = (hover_x + search_radius).min(sw as i32) as u32;
    let y_start = hover_y.max(0) as u32;  // only below the hover point
    let y_end = (hover_y + search_radius).min(sh as i32) as u32;

    let mut best_diff = 0u64;
    let mut best_x = 0i32;
    let mut best_y = 0i32;

    // Find where the biggest pixel difference is (where the button appeared)
    let block = 16u32;
    for by in (y_start..y_end).step_by(block as usize / 2) {
        for bx in (x_start..x_end).step_by(block as usize / 2) {
            let mut diff = 0u64;
            for dy in 0..block.min(y_end - by) {
                for dx in 0..block.min(x_end - bx) {
                    let px = bx + dx;
                    let py = by + dy;
                    let i = (py * sw + px) as usize * 4;
                    if i + 2 >= before.len() || i + 2 >= after.len() { continue; }
                    let dr = (before[i] as i32 - after[i] as i32).unsigned_abs();
                    let dg = (before[i+1] as i32 - after[i+1] as i32).unsigned_abs();
                    let db = (before[i+2] as i32 - after[i+2] as i32).unsigned_abs();
                    diff += (dr + dg + db) as u64;
                }
            }
            if diff > best_diff {
                best_diff = diff;
                best_x = (bx + block / 2) as i32;
                best_y = (by + block / 2) as i32;
            }
        }
    }

    // Only return if we found a meaningful new element
    if best_diff > 5000 {
        Some((best_x, best_y))
    } else {
        None
    }
}

/// Find Gemini input area — wide light bar in bottom portion of screen.
fn find_input(screen: &[u8], sw: u32, sh: u32) -> Option<(i32, i32)> {
    let y_start = (sh as f32 * 0.65) as u32;

    // Scan for a wide horizontal run of light pixels (the input bar)
    let mut best_y = 0i32;
    let mut best_run = 0u32;
    let min_run = sw / 3;

    for y in y_start..sh {
        let mut run = 0u32;
        for x in 0..sw {
            let i = (y * sw + x) as usize * 4;
            if i + 2 >= screen.len() { break; }
            let r = screen[i];
            let g = screen[i + 1];
            let b = screen[i + 2];
            // Light gray or white
            if r > 220 && g > 220 && b > 220 {
                run += 1;
            } else {
                if run > best_run && run >= min_run {
                    best_run = run;
                    best_y = y as i32;
                }
                run = 0;
            }
        }
        if run > best_run && run >= min_run {
            best_run = run;
            best_y = y as i32;
        }
    }

    if best_run >= min_run {
        Some((sw as i32 / 2, best_y))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Input helpers
// ---------------------------------------------------------------------------

fn click_at(x: i32, y: i32) -> Result<()> {
    let mut e = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.move_mouse(x, y, Coordinate::Abs).map_err(|e| anyhow::anyhow!("{}", e))?;
    std::thread::sleep(Duration::from_millis(100));
    e.button(Button::Left, Direction::Click).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

fn move_mouse(x: i32, y: i32) -> Result<()> {
    let mut e = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.move_mouse(x, y, Coordinate::Abs).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

fn type_text(text: &str) -> Result<()> {
    let mut e = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{}", e))?;
    for chunk in text.as_bytes().chunks(50) {
        let s = String::from_utf8_lossy(chunk);
        e.text(&s).map_err(|e| anyhow::anyhow!("{}", e))?;
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn press_enter() -> Result<()> {
    let mut e = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.key(Key::Return, Direction::Click).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Focus a specific Firefox window by index (0 or 1).
fn focus_gemini_window(index: usize) -> Result<()> {
    let script = format!(
        r#"tell application "System Events"
            set firefoxWindows to every window of process "firefox" whose name contains "Gemini"
            if (count of firefoxWindows) > {}
                perform action "AXRaise" of item {} of firefoxWindows
                set frontmost of process "firefox" to true
            end if
        end tell"#,
        index, index + 1
    );
    Command::new("osascript").args(["-e", &script]).output()?;
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

/// Count Gemini windows.
fn count_gemini_windows() -> usize {
    let out = Command::new("osascript")
        .args(["-e", r#"tell application "System Events" to count (every window of process "firefox" whose name contains "Gemini")"#])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0),
        Err(_) => 0,
    }
}

/// Start new chat in current Gemini window.
fn new_chat() -> Result<()> {
    // Gemini keyboard shortcut for new chat
    let mut e = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{}", e))?;
    // Try Cmd+Shift+O (common Gemini shortcut)
    e.key(Key::Meta, Direction::Press).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.key(Key::Shift, Direction::Press).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.key(Key::Unicode('o'), Direction::Click).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.key(Key::Shift, Direction::Release).map_err(|e| anyhow::anyhow!("{}", e))?;
    e.key(Key::Meta, Direction::Release).map_err(|e| anyhow::anyhow!("{}", e))?;
    std::thread::sleep(Duration::from_secs(1));
    Ok(())
}

// ---------------------------------------------------------------------------
// Single prompt cycle
// ---------------------------------------------------------------------------

/// Send a prompt in the currently focused window.
fn send_prompt(text: &str) -> Result<()> {
    let (sw, sh, screen) = capture_screen()?;

    let (ix, iy) = find_input(&screen, sw, sh)
        .unwrap_or((sw as i32 / 2, (sh as f32 * 0.85) as i32));

    click_at(ix, iy)?;
    std::thread::sleep(Duration::from_millis(500));
    type_text(text)?;
    std::thread::sleep(Duration::from_millis(300));
    press_enter()?;
    Ok(())
}

/// Wait for image to appear, hover it, find and click download.
fn harvest_image(timeout: Duration) -> Result<bool> {
    let (sw, sh, initial) = capture_screen()?;
    let initial_hash = screen_hash(&initial);
    let start = Instant::now();

    // Wait for screen to change (image generated)
    loop {
        if start.elapsed() > timeout { return Ok(false); }
        std::thread::sleep(Duration::from_secs(5));
        let (_, _, current) = capture_screen()?;
        let current_hash = screen_hash(&current);
        if (initial_hash as i64 - current_hash as i64).unsigned_abs() > 500000 {
            // Screen changed — wait a bit more for full render
            std::thread::sleep(Duration::from_secs(3));
            break;
        }
    }

    // Take pre-hover screenshot
    let (sw, sh, before_hover) = capture_screen()?;

    // Find the generated image
    let (img_x, img_y) = find_generated_image(&before_hover, sw, sh)
        .unwrap_or((sw as i32 / 2, sh as i32 / 2));

    // Hover over the image
    move_mouse(img_x, img_y)?;
    std::thread::sleep(Duration::from_secs(1));

    // Take post-hover screenshot
    let (_, _, after_hover) = capture_screen()?;

    // Find the download button (new element that appeared on hover)
    if let Some((dx, dy)) = find_download_button(&before_hover, &after_hover, sw, sh, img_x, img_y) {
        click_at(dx, dy)?;
        std::thread::sleep(Duration::from_secs(2));
        return Ok(true);
    }

    // Fallback: try clicking slightly below the image center
    // Many UIs put the download button below the image
    let fallback_y = img_y + 50;
    click_at(img_x, fallback_y)?;
    std::thread::sleep(Duration::from_secs(1));

    // Second fallback: try right side of image (common icon placement)
    let (_, _, after_click) = capture_screen()?;
    if screen_hash(&after_click) != screen_hash(&after_hover) {
        // Something happened — download might have triggered
        std::thread::sleep(Duration::from_secs(2));
        return Ok(true);
    }

    Ok(false)
}

// ---------------------------------------------------------------------------
// Main pipeline
// ---------------------------------------------------------------------------

pub async fn run_autoprompt(
    prompt_file: &str,
    output_dir: &str,
    _workers: usize,
    skip: usize,
) -> Result<()> {
    let prompts = load_prompts(prompt_file)?;
    println!("loaded {} prompts from {}", prompts.len(), prompt_file);

    let prompts: Vec<_> = prompts.into_iter().skip(skip).collect();
    if prompts.is_empty() {
        println!("all prompts done");
        return Ok(());
    }

    std::fs::create_dir_all(output_dir)?;

    // Check screen capture
    let (sw, sh, _) = capture_screen().context(
        "grant Screen Recording permission in System Settings"
    )?;
    println!("screen: {}x{}", sw, sh);

    // Count Gemini windows
    let num_windows = count_gemini_windows();
    println!("found {} Gemini windows", num_windows);

    if num_windows == 0 {
        anyhow::bail!("no Firefox windows with 'Gemini' in title found. Open Gemini first.");
    }

    let dual = num_windows >= 2;
    println!("mode: {}", if dual { "dual window (interleaved)" } else { "single window" });
    println!("output: {}", output_dir);
    println!("\nstarting in 3 seconds...\n");
    std::thread::sleep(Duration::from_secs(3));

    let mut saved = 0usize;
    let mut i = 0;

    while i < prompts.len() {
        let idx = skip + i;

        if dual && i + 1 < prompts.len() {
            // Dual mode: send to window 0, then window 1, then harvest both
            let entry0 = &prompts[i];
            let entry1 = &prompts[i + 1];

            // Window 0: send prompt
            println!("[win0] {}: {}...", entry0.label, &entry0.text[..50.min(entry0.text.len())]);
            focus_gemini_window(0)?;
            std::thread::sleep(Duration::from_millis(500));
            send_prompt(&entry0.text)?;

            // Window 1: send prompt while window 0 generates
            println!("[win1] {}: {}...", entry1.label, &entry1.text[..50.min(entry1.text.len())]);
            focus_gemini_window(1)?;
            std::thread::sleep(Duration::from_millis(500));
            send_prompt(&entry1.text)?;

            // Wait a bit for both to start generating
            std::thread::sleep(Duration::from_secs(30));

            // Harvest window 0
            focus_gemini_window(0)?;
            std::thread::sleep(Duration::from_secs(1));
            match harvest_image(Duration::from_secs(150)) {
                Ok(true) => { println!("  [win0] downloaded"); saved += 1; }
                Ok(false) => eprintln!("  [win0] no download button found"),
                Err(e) => eprintln!("  [win0] error: {}", e),
            }
            new_chat()?;

            // Harvest window 1
            focus_gemini_window(1)?;
            std::thread::sleep(Duration::from_secs(1));
            match harvest_image(Duration::from_secs(150)) {
                Ok(true) => { println!("  [win1] downloaded"); saved += 1; }
                Ok(false) => eprintln!("  [win1] no download button found"),
                Err(e) => eprintln!("  [win1] error: {}", e),
            }
            new_chat()?;

            i += 2;
        } else {
            // Single mode
            let entry = &prompts[i];
            println!("[{}] {}: {}...", idx, entry.label, &entry.text[..50.min(entry.text.len())]);

            focus_gemini_window(0)?;
            std::thread::sleep(Duration::from_millis(500));
            send_prompt(&entry.text)?;

            match harvest_image(Duration::from_secs(180)) {
                Ok(true) => { println!("  downloaded"); saved += 1; }
                Ok(false) => eprintln!("  no download button found"),
                Err(e) => eprintln!("  error: {}", e),
            }
            new_chat()?;

            i += 1;
        }
    }

    println!("\nautoprompt done: {} downloads triggered", saved);
    println!("run 'pixel-forge ingest-gemini' to process them");
    Ok(())
}
