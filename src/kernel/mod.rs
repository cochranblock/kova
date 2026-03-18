// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! T208 — unified dispatch. All three surfaces (CLI, Serve, GUI) hold Arc<T208>.
//! Every request goes through the kernel. No surface calls inference/cargo/tools directly.

pub mod commands;
pub mod stream;

use std::path::PathBuf;
use std::sync::Arc;

pub use commands::T207;
pub use stream::T206;

/// T208. Central dispatch for all Kova capabilities.
/// All surfaces hold Arc<T208> and call methods on it.
pub struct T208 {
    pub project_dir: PathBuf,
    pub config: T209,
}

/// Kernel configuration. Assembled at startup.
pub struct T209 {
    /// Max fix loop retries.
    pub max_fix_retries: u32,
    /// Run clippy in code gen pipelines.
    pub run_clippy: bool,
    /// Inference model path (local Kalosm).
    pub model_path: Option<PathBuf>,
}

impl Default for T209 {
    fn default() -> Self {
        Self {
            max_fix_retries: crate::config::orchestration_max_fix_retries(),
            run_clippy: crate::config::orchestration_run_clippy(),
            model_path: crate::config::inference_model_path(),
        }
    }
}

impl T208 {
    /// Create a new kernel with default config.
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            config: T209::default(),
        }
    }

    /// Create kernel with explicit config.
    pub fn with_config(project_dir: PathBuf, config: T209) -> Self {
        Self {
            project_dir,
            config,
        }
    }

    /// Wrap in Arc for sharing across surfaces.
    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    // ── Capability methods ──
    // These will be wired in Phase 5 when surfaces are thinned.
    // For now, the kernel exists as a struct that can be passed around.

    /// Run cargo check on the project.
    pub fn cargo_check(&self) -> (bool, String) {
        crate::cargo::cargo_check(&self.project_dir)
    }

    /// Run cargo clippy on the project.
    pub fn cargo_clippy(&self) -> (bool, String) {
        crate::cargo::cargo_clippy(&self.project_dir)
    }

    /// Run cargo test on the project.
    pub fn cargo_test(&self) -> (bool, String) {
        crate::cargo::cargo_test(&self.project_dir)
    }

    /// Get cluster status.
    pub fn cluster_status(&self) -> String {
        crate::inference::cluster::T193::default_hive().status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_default_config() {
        let config = T209::default();
        assert!(config.max_fix_retries > 0);
    }

    #[test]
    fn kernel_new() {
        let kernel = T208::new(PathBuf::from("/tmp/test"));
        assert_eq!(kernel.project_dir, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn kernel_shared() {
        let kernel = T208::new(PathBuf::from("/tmp/test"));
        let shared = kernel.shared();
        assert_eq!(shared.project_dir, PathBuf::from("/tmp/test"));
    }
}
