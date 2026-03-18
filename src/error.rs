//! T176 — unified error type. Replaces Result<T, String> and raw anyhow across the codebase.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use thiserror::Error;

/// t176=T176. One error type for all Kova operations.
#[derive(Debug, Error)]
pub enum T176 {
    #[error("{0}")]
    Storage(#[from] crate::storage::E0),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("inference: {0}")]
    Inference(String),

    #[error("cluster: {0}")]
    T193(String),

    #[error("cargo: {0}")]
    Cargo(String),

    #[error("codegen: {0}")]
    CodeGen(String),

    #[error("config: {0}")]
    Config(String),

    #[error("provider: {0}")]
    T129(String),

    #[error("tool: {0}")]
    Tool(String),

    #[error("serde: {0}")]
    Serde(String),

    #[error("{0}")]
    Other(String),
}

impl From<String> for T176 {
    fn from(s: String) -> Self {
        T176::Other(s)
    }
}

impl From<&str> for T176 {
    fn from(s: &str) -> Self {
        T176::Other(s.to_string())
    }
}

impl From<serde_json::Error> for T176 {
    fn from(e: serde_json::Error) -> Self {
        T176::Serde(e.to_string())
    }
}

impl From<anyhow::Error> for T176 {
    fn from(e: anyhow::Error) -> Self {
        T176::Other(e.to_string())
    }
}

/// Convenience alias.
pub type T176Result<T> = Result<T, T176>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string() {
        let e: T176 = "something broke".into();
        assert!(e.to_string().contains("something broke"));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::T95::NotFound, "missing");
        let e: T176 = io_err.into();
        assert!(e.to_string().contains("missing"));
    }

    #[test]
    fn display_variants() {
        let e = T176::Inference("model load failed".into());
        assert_eq!(e.to_string(), "inference: model load failed");

        let e = T176::Cargo("check failed".into());
        assert_eq!(e.to_string(), "cargo: check failed");
    }
}