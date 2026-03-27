//! Kova browser automation — drive web apps via WebDriver/geckodriver.
//! Used for: Gemini sprite generation, bulk prompting, data harvesting.
//!
//! Requires: geckodriver installed (brew install geckodriver)
//! Feature: --features browser

// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

#[cfg(feature = "browser")]
use fantoccini::{Client, ClientBuilder, Locator};

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Prompt entry — label + text to send.
#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub label: String,
    pub text: String,
}

/// Load prompts from a markdown file.
/// Expects format: ### LABEL\nCreate a 6x5 grid...
pub fn load_prompts(path: &str) -> Result<Vec<PromptEntry>> {
    let content = std::fs::read_to_string(path)?;
    let mut prompts = Vec::new();
    let mut current_label: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            // Extract label from "### Prompt N — label" or "### DND-N — label"
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

/// Check if geckodriver is available.
pub fn check_geckodriver() -> bool {
    std::process::Command::new("geckodriver")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Start a geckodriver instance on the given port. Returns the child process.
pub fn start_geckodriver(port: u16) -> Result<std::process::Child> {
    let child = std::process::Command::new("geckodriver")
        .args(["--port", &port.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Give it a moment to start
    std::thread::sleep(Duration::from_secs(2));
    Ok(child)
}

/// Run the autoprompt pipeline. Drives N browser workers through a prompt list.
#[cfg(feature = "browser")]
pub async fn run_autoprompt(
    prompt_file: &str,
    output_dir: &str,
    workers: usize,
    skip: usize,
) -> Result<()> {
    use tokio::time::sleep;

    if !check_geckodriver() {
        anyhow::bail!("geckodriver not found. Install: brew install geckodriver");
    }

    let prompts = load_prompts(prompt_file)?;
    println!("loaded {} prompts from {}", prompts.len(), prompt_file);

    // Skip already completed
    let prompts: Vec<_> = prompts.into_iter().skip(skip).collect();
    if prompts.is_empty() {
        println!("all prompts already completed (skip={})", skip);
        return Ok(());
    }

    std::fs::create_dir_all(output_dir)?;

    // Split prompts across workers
    let chunk_size = (prompts.len() + workers - 1) / workers;
    let chunks: Vec<Vec<PromptEntry>> = prompts
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    println!("starting {} workers, {} prompts each", workers, chunk_size);
    println!("output: {}", output_dir);
    println!();

    // Start geckodrivers
    let base_port = 4444u16;
    let mut gecko_procs = Vec::new();
    for i in 0..workers {
        let port = base_port + i as u16;
        println!("starting geckodriver on port {}...", port);
        gecko_procs.push(start_geckodriver(port)?);
    }

    // Launch workers
    let mut handles = Vec::new();
    for (i, chunk) in chunks.into_iter().enumerate() {
        let port = base_port + i as u16;
        let out = output_dir.to_string();
        let offset = skip + i * chunk_size;

        let handle = tokio::spawn(async move {
            match worker(port, chunk, &out, offset).await {
                Ok(count) => {
                    println!("[worker {}] done: {} sprites", i, count);
                    count
                }
                Err(e) => {
                    eprintln!("[worker {}] error: {}", i, e);
                    0
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all workers
    let mut total = 0usize;
    for h in handles {
        total += h.await.unwrap_or(0);
    }

    // Kill geckodrivers
    for mut proc in gecko_procs {
        let _ = proc.kill();
    }

    println!("\nautoprompt done: {} images saved to {}", total, output_dir);
    Ok(())
}

/// Single worker — connects to geckodriver, opens Gemini, sends prompts, saves images.
#[cfg(feature = "browser")]
async fn worker(port: u16, prompts: Vec<PromptEntry>, output_dir: &str, offset: usize) -> Result<usize> {
    use tokio::time::sleep;

    let url = format!("http://localhost:{}", port);
    let client = ClientBuilder::native()
        .connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("connect to geckodriver:{} failed: {}", port, e))?;

    // Navigate to Gemini
    client.goto("https://gemini.google.com/app").await?;
    println!("[port {}] opened Gemini — waiting for login...", port);

    // Wait for user to be logged in (check for input field)
    // Give generous time for manual login on first run
    let mut logged_in = false;
    for attempt in 0..60 {
        sleep(Duration::from_secs(5)).await;
        // Check for the chat input
        if find_input(&client).await.is_ok() {
            logged_in = true;
            break;
        }
        if attempt % 6 == 0 {
            println!("[port {}] waiting for login... ({}s)", port, attempt * 5);
        }
    }

    if !logged_in {
        anyhow::bail!("port {}: Gemini login timeout (5 min)", port);
    }

    println!("[port {}] logged in, starting prompts", port);

    let mut saved = 0usize;
    for (i, entry) in prompts.iter().enumerate() {
        let idx = offset + i;
        println!("[port {}] prompt {}: {}", port, idx, entry.label);

        match send_and_save(&client, &entry.text, &entry.label, idx, output_dir).await {
            Ok(true) => saved += 1,
            Ok(false) => eprintln!("[port {}] no image for prompt {}", port, idx),
            Err(e) => eprintln!("[port {}] error on prompt {}: {}", port, idx, e),
        }

        // Rate limit cooldown
        sleep(Duration::from_secs(3)).await;
    }

    client.close().await?;
    Ok(saved)
}

/// Find Gemini's input element.
#[cfg(feature = "browser")]
async fn find_input(client: &Client) -> Result<fantoccini::elements::Element> {
    // Gemini uses various input selectors — try each
    let selectors = [
        "div[contenteditable='true']",
        "div.ql-editor",
        "textarea",
        "div[role='textbox']",
        ".text-input-field",
        "rich-textarea div[contenteditable]",
    ];

    for sel in &selectors {
        if let Ok(el) = client.find(Locator::Css(sel)).await {
            return Ok(el);
        }
    }

    anyhow::bail!("input field not found")
}

/// Send a prompt, wait for image, download it.
#[cfg(feature = "browser")]
async fn send_and_save(
    client: &Client,
    prompt: &str,
    label: &str,
    index: usize,
    output_dir: &str,
) -> Result<bool> {
    use tokio::time::sleep;

    // Find and fill the input
    let input = find_input(client).await?;
    input.click().await?;
    sleep(Duration::from_millis(300)).await;

    // Clear existing text and type new prompt
    input.send_keys(prompt).await?;
    sleep(Duration::from_millis(500)).await;

    // Submit — try button first, then Enter key
    let submitted = if let Ok(btn) = client.find(Locator::Css("button[aria-label='Send message'], button[data-testid='send-button'], .send-button")).await {
        btn.click().await.is_ok()
    } else {
        input.send_keys("\n").await.is_ok()
    };

    if !submitted {
        return Ok(false);
    }

    // Wait for image generation (poll every 5s, up to 3 min)
    let mut img_data = None;
    for _ in 0..36 {
        sleep(Duration::from_secs(5)).await;

        // Look for generated images in the response
        let script = r#"
            // Find the most recent image in the chat
            const imgs = document.querySelectorAll('img[src^="blob:"], img[src^="data:image"], img.generated-image, img[alt*="Generated"]');
            if (imgs.length === 0) return null;
            const img = imgs[imgs.length - 1];

            // Convert to base64 via canvas
            try {
                const canvas = document.createElement('canvas');
                canvas.width = img.naturalWidth || img.width;
                canvas.height = img.naturalHeight || img.height;
                if (canvas.width === 0 || canvas.height === 0) return null;
                canvas.getContext('2d').drawImage(img, 0, 0);
                return canvas.toDataURL('image/png').split(',')[1];
            } catch(e) {
                return null;
            }
        "#;

        match client.execute(script, vec![]).await {
            Ok(val) => {
                if let Some(b64) = val.as_str() {
                    if !b64.is_empty() {
                        img_data = Some(b64.to_string());
                        break;
                    }
                }
            }
            Err(_) => continue,
        }
    }

    let Some(b64) = img_data else {
        return Ok(false);
    };

    // Decode and save
    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &b64,
    )?;

    let out_path = PathBuf::from(output_dir).join(format!("auto_{label}_{index:04}.png"));
    std::fs::write(&out_path, &bytes)?;
    println!("  saved: {}", out_path.display());

    // Scroll down / start new chat to prepare for next prompt
    // Click "new chat" if available
    let _ = client
        .execute("document.querySelector('button[aria-label=\"New chat\"]')?.click()", vec![])
        .await;
    sleep(Duration::from_secs(2)).await;

    Ok(true)
}

/// Stub when browser feature is disabled.
#[cfg(not(feature = "browser"))]
pub async fn run_autoprompt(
    _prompt_file: &str,
    _output_dir: &str,
    _workers: usize,
    _skip: usize,
) -> Result<()> {
    anyhow::bail!("browser automation requires --features browser. Rebuild with: cargo build -p kova --features browser")
}
