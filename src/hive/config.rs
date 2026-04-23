// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Configuration for the hive module (absorbed from ironhive).
//! Loaded from ~/.ironhive.toml (path preserved for backwards compat).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub workspace: String,
    pub remote_base: String,
    pub excludes: Vec<String>,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub host: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workspace: "/Users/mcochran".into(),
            remote_base: "/home/mcochran".into(),
            excludes: vec![
                "target".into(),
                ".git".into(),
                "node_modules".into(),
                ".DS_Store".into(),
            ],
            nodes: vec![
                Node { name: "n0".into(), host: "lf".into() },
                Node { name: "n1".into(), host: "gd".into() },
                Node { name: "n2".into(), host: "bt".into() },
                Node { name: "n3".into(), host: "st".into() },
            ],
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/mcochran".into());
        PathBuf::from(home).join(".ironhive.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&text).unwrap_or_default()
        } else {
            let cfg = Config::default();
            if let Ok(text) = toml::to_string_pretty(&cfg) {
                let _ = std::fs::write(&path, text);
            }
            cfg
        }
    }

    pub fn rsync_excludes(&self) -> Vec<String> {
        self.excludes
            .iter()
            .flat_map(|e| vec!["--exclude".into(), e.clone()])
            .collect()
    }
}
