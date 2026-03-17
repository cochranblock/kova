// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! KovaError — unified error type. Replaces Result<T, String> and raw anyhow across the codebase.

use thiserror::Error;

/// KovaError. One error type for all Kova operations.
#[derive(Debug, Error)]
pub enum KovaError {
    #[error("{0}")]
    Storage(#[from] crate::storage::E0),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("inference: {0}")]
    Inference(String),

    #[error("cluster: {0}")]
    Cluster(String),

    #[error("cargo: {0}")]
    Cargo(String),

    #[error("codegen: {0}")]
    CodeGen(String),

    #[error("config: {0}")]
    Config(String),

    #[error("provider: {0}")]
    Provider(String),

    #[error("tool: {0}")]
    Tool(String),

    #[error("serde: {0}")]
    Serde(String),

    #[error("{0}")]
    Other(String),
}

impl From<String> for KovaError {
    fn from(s: String) -> Self {
        KovaError::Other(s)
    }
}

impl From<&str> for KovaError {
    fn from(s: &str) -> Self {
        KovaError::Other(s.to_string())
    }
}

impl From<serde_json::Error> for KovaError {
    fn from(e: serde_json::Error) -> Self {
        KovaError::Serde(e.to_string())
    }
}

impl From<anyhow::Error> for KovaError {
    fn from(e: anyhow::Error) -> Self {
        KovaError::Other(e.to_string())
    }
}

/// Convenience alias.
pub type KovaResult<T> = Result<T, KovaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string() {
        let e: KovaError = "something broke".into();
        assert!(e.to_string().contains("something broke"));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e: KovaError = io_err.into();
        assert!(e.to_string().contains("missing"));
    }

    #[test]
    fn display_variants() {
        let e = KovaError::Inference("model load failed".into());
        assert_eq!(e.to_string(), "inference: model load failed");

        let e = KovaError::Cargo("check failed".into());
        assert_eq!(e.to_string(), "cargo: check failed");
    }
}
