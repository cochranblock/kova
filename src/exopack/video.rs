// Unlicense — public domain — cochranblock.org
//! Screen capture and recording — trait + xcap impl.
//! Use for demo mode: screenshot before/after actions, optional video.

use std::path::Path;

/// t64 = VideoRecorder. Video recorder trait. Implement for platform-specific capture.
pub trait t64: Send + Sync {
    /// Start recording.
    fn start(&mut self) -> Result<(), String>;
    /// Stop and save to path. Returns saved file path.
    fn stop(&mut self, out: &Path) -> Result<std::path::PathBuf, String>;
}

/// f88 = capture_screenshot. Screenshot capture. Returns path to saved PNG.
pub fn f88(out_dir: &Path, name: &str) -> Result<std::path::PathBuf, String> {
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

/// t65 = NoopRecorder. No-op recorder. Use when platform impl not available.
pub struct t65;

impl t64 for t65 {
    fn start(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn stop(&mut self, _out: &Path) -> Result<std::path::PathBuf, String> {
        Err("video recording not implemented on this platform".into())
    }
}

/// f89 = create_recorder. Create a recorder for the current platform.
/// Video encoding (xcap frames → file) deferred; screenshot capture available via f88.
pub fn f89() -> Box<dyn t64> {
    Box::new(t65)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_recorder_start_succeeds() {
        let mut rec = t65;
        assert!(rec.start().is_ok());
    }

    #[test]
    fn noop_recorder_stop_returns_error() {
        let mut rec = t65;
        let out = std::path::Path::new("/tmp/fake.mp4");
        assert!(rec.stop(out).is_err());
    }

    #[test]
    fn create_recorder_returns_noop() {
        let mut rec = f89();
        assert!(rec.start().is_ok());
        assert!(rec.stop(std::path::Path::new("/tmp/fake.mp4")).is_err());
    }
}
