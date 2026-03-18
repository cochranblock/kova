// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Backlog types + loading. f25=load_backlog from disk. Formerly in kova-core.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::intent::{t0, t1};

/// t8=t8. One item in backlog.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct t8 {
    pub intent: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub approuter_dir: Option<String>,
    #[serde(default)]
    pub cmd: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
}

/// t9=Backlog. items = Vec<t8>.
#[derive(Debug, Default, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct t9 {
    pub items: Vec<t8>,
}

/// f293=f293. Map backlog entry to intent. Returns None if unsupported.
pub fn f293(entry: &t8) -> Option<t0> {
    let intent = entry.intent.to_lowercase();
    let project_hint = entry.project.as_ref().and_then(|p| {
        std::path::Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    });
    let with_hint = |mut i: t0| {
        i.s1 = project_hint.clone();
        i
    };
    match intent.as_str() {
        "full-pipeline" => Some(with_hint(t0::f20())),
        "tunnel-update" => Some(with_hint(t0::f21())),
        "setup-roguerepo" => Some(with_hint(t0::f22())),
        "cloudflare-purge" => Some(with_hint(t0::f23())),
        "test" => Some(with_hint(t0::f19())),
        "custom" => {
            let cmd = entry.cmd.clone()?;
            let args = entry.args.clone().unwrap_or_default();
            Some(t0 {
                s0: t1::Custom { cmd, args },
                s1: project_hint,
                s2: vec![],
            })
        }
        _ => None,
    }
}

/// f25=load_backlog. Parse backlog.json from disk.
pub fn f25(p0: &Path) -> anyhow::Result<t9> {
    let v0 = std::fs::read_to_string(p0)?;
    let v1: t9 = serde_json::from_str(&v0)?;
    Ok(v1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{t0, t1};

    #[test]
    fn f293_full_pipeline() {
        let e = t8 {
            intent: "full-pipeline".into(),
            project: None,
            approuter_dir: None,
            cmd: None,
            args: None,
        };
        assert!(matches!(f293(&e), Some(t0 { s0: t1::FullPipeline, .. })));
    }

    #[test]
    fn f293_custom() {
        let e = t8 {
            intent: "custom".into(),
            project: None,
            approuter_dir: None,
            cmd: Some("cargo".into()),
            args: Some(vec!["check".into()]),
        };
        let got = f293(&e).unwrap();
        match &got.s0 {
            t1::Custom { cmd, args } => {
                assert_eq!(cmd, "cargo");
                assert_eq!(args, &["check"]);
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn f293_unknown_returns_none() {
        let e = t8 {
            intent: "unknown-intent".into(),
            project: None,
            approuter_dir: None,
            cmd: None,
            args: None,
        };
        assert!(f293(&e).is_none());
    }
}
