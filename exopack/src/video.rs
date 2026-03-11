// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Screen capture and recording — trait + xcap impl.
//! Use for demo mode: screenshot before/after actions, optional video.
//!
//! Recording caps at 15 seconds. Minimum 8 frames for movement detection.

use std::path::Path;

/// Max recording duration in seconds.
pub const MAX_DURATION_SECS: u64 = 15;

/// Minimum frames needed to assess movement quality.
pub const MIN_FRAMES: usize = 8;

/// Capture interval in milliseconds (2 fps = 500ms).
const CAPTURE_INTERVAL_MS: u64 = 500;

/// Video recorder trait. Implement for platform-specific capture.
pub trait VideoRecorder: Send + Sync {
    /// Start recording.
    fn start(&mut self) -> Result<(), String>;
    /// Stop and save to path. Returns saved file path.
    fn stop(&mut self, out: &Path) -> Result<std::path::PathBuf, String>;
}

/// Screenshot capture. Returns path to saved PNG.
pub fn capture_screenshot(out_dir: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    #[cfg(feature = "video")]
    {
        use std::fs;
        fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
        let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .ok_or("no primary monitor")?;
        let image = primary.capture_image().map_err(|e| e.to_string())?;
        let safe = name.replace(['|', '\\', ':', '/', ' '], "_");
        let path = out_dir.join(format!("{}.png", safe));
        image.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }
    #[cfg(not(feature = "video"))]
    {
        let _ = (out_dir, name);
        Err("video feature not enabled".into())
    }
}

/// No-op recorder. Use when platform impl not available.
pub struct NoopRecorder;

impl VideoRecorder for NoopRecorder {
    fn start(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn stop(&mut self, _out: &Path) -> Result<std::path::PathBuf, String> {
        Err("video recording not implemented on this platform".into())
    }
}

/// Frame-based recorder. Captures screenshots at intervals, caps at 15 seconds.
/// Stores raw frames for movement analysis.
#[cfg(feature = "video")]
pub struct FrameRecorder {
    frames: Vec<image::RgbaImage>,
    recording: bool,
    start_time: Option<std::time::Instant>,
    handle: Option<std::thread::JoinHandle<Vec<image::RgbaImage>>>,
    stop_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

#[cfg(feature = "video")]
impl Default for FrameRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "video")]
impl FrameRecorder {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            recording: false,
            start_time: None,
            handle: None,
            stop_flag: None,
        }
    }

    /// Captured frames after stop.
    pub fn frames(&self) -> &[image::RgbaImage] {
        &self.frames
    }

    /// Check if movement is shitty: frozen, stuttering, or huge jumps.
    /// Returns (has_movement, quality_score 0.0-1.0, diagnosis).
    pub fn check_movement(&self) -> (bool, f64, &'static str) {
        if self.frames.len() < 2 {
            return (false, 0.0, "not enough frames");
        }

        let diffs: Vec<f64> = self
            .frames
            .windows(2)
            .map(|pair| frame_diff(&pair[0], &pair[1]))
            .collect();

        let has_movement = diffs.iter().any(|&d| d > 0.005);
        if !has_movement {
            return (false, 0.0, "frozen — no pixel change detected");
        }

        let avg_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let max_diff = diffs.iter().cloned().fold(0.0_f64, f64::max);
        let min_diff = diffs.iter().cloned().fold(f64::MAX, f64::min);
        let variance = max_diff - min_diff;

        // Huge single jump = teleport / broken rendering
        if max_diff > 0.5 && min_diff < 0.01 {
            return (true, 0.2, "teleporting — large frame jump with stalls");
        }

        // High variance = stuttery
        if variance > avg_diff * 3.0 && avg_diff > 0.01 {
            return (true, 0.4, "stuttery — inconsistent frame deltas");
        }

        // Smooth enough
        let score = (1.0 - variance.min(1.0)) * (1.0 - (max_diff - avg_diff).clamp(0.0, 1.0));
        if score > 0.7 {
            (true, score, "smooth")
        } else if score > 0.4 {
            (true, score, "passable — minor jank")
        } else {
            (true, score, "rough — visible stutter")
        }
    }
}

#[cfg(feature = "video")]
impl VideoRecorder for FrameRecorder {
    fn start(&mut self) -> Result<(), String> {
        if self.recording {
            return Err("already recording".into());
        }
        self.frames.clear();
        self.recording = true;
        self.start_time = Some(std::time::Instant::now());

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_clone = stop.clone();

        let handle = std::thread::spawn(move || {
            let mut captured = Vec::new();
            let start = std::time::Instant::now();
            let max_dur = std::time::Duration::from_secs(MAX_DURATION_SECS);
            let interval = std::time::Duration::from_millis(CAPTURE_INTERVAL_MS);

            loop {
                if stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if start.elapsed() >= max_dur {
                    break;
                }

                if let Ok(monitors) = xcap::Monitor::all() {
                    if let Some(primary) = monitors.into_iter().find(|m| m.is_primary().unwrap_or(false)) {
                        if let Ok(img) = primary.capture_image() {
                            captured.push(img);
                        }
                    }
                }

                std::thread::sleep(interval);
            }

            captured
        });

        self.stop_flag = Some(stop);
        self.handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self, out: &Path) -> Result<std::path::PathBuf, String> {
        if !self.recording {
            return Err("not recording".into());
        }
        self.recording = false;

        if let Some(flag) = self.stop_flag.take() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        if let Some(handle) = self.handle.take() {
            self.frames = handle.join().map_err(|_| "capture thread panicked")?;
        }

        if self.frames.len() < MIN_FRAMES {
            return Err(format!(
                "only captured {} frames, need at least {}",
                self.frames.len(),
                MIN_FRAMES
            ));
        }

        // Save frames as numbered PNGs in output dir
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        for (i, frame) in self.frames.iter().enumerate() {
            let path = out.join(format!("frame_{:04}.png", i));
            frame.save(&path).map_err(|e| e.to_string())?;
        }

        Ok(out.to_path_buf())
    }
}

/// Pixel diff ratio between two frames (0.0 = identical, 1.0 = completely different).
#[cfg(feature = "video")]
fn frame_diff(a: &image::RgbaImage, b: &image::RgbaImage) -> f64 {
    if a.dimensions() != b.dimensions() {
        return 1.0;
    }
    let total = a.pixels().count() as f64;
    if total == 0.0 {
        return 0.0;
    }
    let changed = a
        .pixels()
        .zip(b.pixels())
        .filter(|(pa, pb)| {
            let da = (pa[0] as i32 - pb[0] as i32).unsigned_abs();
            let dg = (pa[1] as i32 - pb[1] as i32).unsigned_abs();
            let db = (pa[2] as i32 - pb[2] as i32).unsigned_abs();
            // Threshold: ignore sub-pixel noise (< 10 per channel)
            da > 10 || dg > 10 || db > 10
        })
        .count() as f64;
    changed / total
}

/// Create a recorder for the current platform.
pub fn create_recorder() -> Box<dyn VideoRecorder> {
    #[cfg(feature = "video")]
    {
        Box::new(FrameRecorder::new())
    }
    #[cfg(not(feature = "video"))]
    {
        Box::new(NoopRecorder)
    }
}
