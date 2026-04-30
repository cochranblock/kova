// Unlicense — public domain — cochranblock.org
//! dual_mode — test harness for dual-mode inference routing.
//! Verifies: env-driven dispatch, local/remote/auto modes, fallback behavior.
//! Pure std — no inference dep. Tests the routing contract.

use std::path::Path;

/// t77: Inference mode resolved from environment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum t77 {
    /// Local GGUF model
    Local,
    /// Remote API (Anthropic)
    Remote,
    /// Auto: local if model exists, else remote
    Auto,
}

/// t78: Inference routing decision
#[derive(Debug, Clone)]
pub struct t78 {
    /// s99: requested mode (from env)
    pub s99: t77,
    /// s100: resolved mode (after auto-detection)
    pub s100: t77,
    /// s101: model path exists
    pub s101: bool,
    /// s102: API key present
    pub s102: bool,
    /// s103: detail message
    pub s103: String,
}

/// f133: Resolve inference mode from env var value.
pub fn f133(env_value: &str) -> t77 {
    match env_value.to_lowercase().as_str() {
        "local" => t77::Local,
        "remote" => t77::Remote,
        _ => t77::Auto,
    }
}

/// f134: Determine routing decision given mode, model path, and API key.
/// This tests the routing logic without calling any inference backend.
pub fn f134(mode: t77, model_path: &Path, has_api_key: bool) -> t78 {
    let model_exists = model_path.exists();

    let (resolved, detail) = match &mode {
        t77::Local => {
            if model_exists {
                (t77::Local, "local model found".to_string())
            } else {
                (
                    t77::Local,
                    format!(
                        "local requested but model missing: {}",
                        model_path.display()
                    ),
                )
            }
        }
        t77::Remote => {
            if has_api_key {
                (t77::Remote, "remote API with key".to_string())
            } else {
                (t77::Remote, "remote requested but no API key".to_string())
            }
        }
        t77::Auto => {
            if model_exists {
                (t77::Local, "auto → local (model exists)".to_string())
            } else if has_api_key {
                (
                    t77::Remote,
                    "auto → remote (no model, key present)".to_string(),
                )
            } else {
                (
                    t77::Remote,
                    "auto → remote (no model, no key — will fail)".to_string(),
                )
            }
        }
    };

    t78 {
        s99: mode,
        s100: resolved,
        s101: model_exists,
        s102: has_api_key,
        s103: detail,
    }
}

/// f135: Validate that a routing decision will succeed.
/// Returns Ok(detail) or Err(reason).
pub fn f135(decision: &t78) -> Result<String, String> {
    match &decision.s100 {
        t77::Local => {
            if decision.s101 {
                Ok(decision.s103.clone())
            } else {
                Err("local mode requires model file on disk".into())
            }
        }
        t77::Remote => {
            if decision.s102 {
                Ok(decision.s103.clone())
            } else {
                Err("remote mode requires ANTHROPIC_API_KEY".into())
            }
        }
        t77::Auto => Err("auto should resolve to local or remote, not stay auto".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("exopack_dm_{}_{}", name, std::process::id()))
    }

    #[test]
    fn parse_mode_local() {
        assert_eq!(f133("local"), t77::Local);
        assert_eq!(f133("LOCAL"), t77::Local);
    }

    #[test]
    fn parse_mode_remote() {
        assert_eq!(f133("remote"), t77::Remote);
        assert_eq!(f133("REMOTE"), t77::Remote);
    }

    #[test]
    fn parse_mode_auto() {
        assert_eq!(f133("auto"), t77::Auto);
        assert_eq!(f133(""), t77::Auto);
        assert_eq!(f133("anything"), t77::Auto);
    }

    #[test]
    fn local_with_model() {
        let model = tmp("model.gguf");
        fs::write(&model, b"fake model").unwrap();

        let d = f134(t77::Local, &model, false);
        assert_eq!(d.s100, t77::Local);
        assert!(d.s101);
        assert!(f135(&d).is_ok());

        let _ = fs::remove_file(&model);
    }

    #[test]
    fn local_without_model() {
        let model = tmp("missing.gguf");
        let _ = fs::remove_file(&model);

        let d = f134(t77::Local, &model, false);
        assert_eq!(d.s100, t77::Local);
        assert!(!d.s101);
        assert!(f135(&d).is_err());
    }

    #[test]
    fn remote_with_key() {
        let model = tmp("none.gguf");
        let d = f134(t77::Remote, &model, true);
        assert_eq!(d.s100, t77::Remote);
        assert!(d.s102);
        assert!(f135(&d).is_ok());
    }

    #[test]
    fn remote_without_key() {
        let model = tmp("none.gguf");
        let d = f134(t77::Remote, &model, false);
        assert_eq!(d.s100, t77::Remote);
        assert!(!d.s102);
        assert!(f135(&d).is_err());
    }

    #[test]
    fn auto_with_model() {
        let model = tmp("auto_model.gguf");
        fs::write(&model, b"fake").unwrap();

        let d = f134(t77::Auto, &model, true);
        assert_eq!(d.s99, t77::Auto);
        assert_eq!(d.s100, t77::Local); // resolves to local
        assert!(f135(&d).is_ok());

        let _ = fs::remove_file(&model);
    }

    #[test]
    fn auto_without_model_with_key() {
        let model = tmp("auto_nomodel.gguf");
        let _ = fs::remove_file(&model);

        let d = f134(t77::Auto, &model, true);
        assert_eq!(d.s99, t77::Auto);
        assert_eq!(d.s100, t77::Remote); // resolves to remote
        assert!(f135(&d).is_ok());
    }

    #[test]
    fn auto_without_model_without_key() {
        let model = tmp("auto_nothing.gguf");
        let _ = fs::remove_file(&model);

        let d = f134(t77::Auto, &model, false);
        assert_eq!(d.s100, t77::Remote);
        assert!(f135(&d).is_err()); // will fail at runtime
    }

    #[test]
    fn auto_never_stays_auto() {
        // Auto should always resolve to Local or Remote
        let model = tmp("resolve.gguf");
        for exists in [true, false] {
            if exists {
                fs::write(&model, b"x").unwrap();
            } else {
                let _ = fs::remove_file(&model);
            }
            for key in [true, false] {
                let d = f134(t77::Auto, &model, key);
                assert_ne!(d.s100, t77::Auto, "auto must resolve");
            }
        }
        let _ = fs::remove_file(&model);
    }
}
