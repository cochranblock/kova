// Unlicense — public domain — cochranblock.org
//! Demo mode: action script recording for UI testing.
//! Record user actions + function invocations; replay to iterate Kova dev cycles.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// t66 = DemoAction. Single recorded action. Web: click, input, api_call. Egui: click, key, intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum t66 {
    /// Web: element click. selector, timestamp.
    WebClick { selector: String, ts_ms: u64 },
    /// Web: input change. id, value.
    WebInput { id: String, value: String },
    /// Web or egui: API/intent invoked. method, path_or_intent, body_summary.
    ApiCall {
        method: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        body_summary: Option<String>,
    },
    /// Egui: chat message sent.
    EguiSend { text: String },
    /// Egui: intent confirmed (y/n).
    EguiConfirm { intent: String },
    /// Screenshot captured. path relative to demo dir.
    Screenshot { path: String },
}

/// t67 = DemoRecord. Demo recording session. Written to ~/.kova/demos/{name}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct t67 {
    pub name: String,
    pub source: String, // "web" | "egui"
    pub actions: Vec<t66>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

impl t67 {
    pub fn new(name: impl Into<String>, source: &str) -> Self {
        Self {
            name: name.into(),
            source: source.to_string(),
            actions: Vec::new(),
            started_at: Some(chrono_now()),
        }
    }

    pub fn push(&mut self, action: t66) {
        self.actions.push(action);
    }

    pub fn save(&self, out_dir: &Path) -> Result<std::path::PathBuf, String> {
        std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
        let safe = self.name.replace(['/', '\\', ':'], "_");
        let path = out_dir.join(format!("{}.json", safe));
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", t)
}

/// f90 = demo_dir. Demo dir: ~/.kova/demos or KOVA_DEMO_DIR
pub fn f90() -> std::path::PathBuf {
    std::env::var("KOVA_DEMO_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".kova")
                .join("demos")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn demo_record_round_trip() {
        let dir = test_dir("exopack_test_demo_rt");

        let mut rec = t67::new("test-demo", "web");
        rec.push(t66::WebClick {
            selector: "#btn".into(),
            ts_ms: 100,
        });
        rec.push(t66::ApiCall {
            method: "POST".into(),
            path: "/api/test".into(),
            body_summary: Some("hello".into()),
        });

        let path = rec.save(&dir).unwrap();
        assert!(path.exists());

        let loaded = t67::load(&path).unwrap();
        assert_eq!(loaded.name, "test-demo");
        assert_eq!(loaded.source, "web");
        assert_eq!(loaded.actions.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn demo_action_serialize_tagged() {
        let action = t66::EguiSend {
            text: "hello kova".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"kind\":\"egui_send\""));
        assert!(json.contains("\"text\":\"hello kova\""));
    }

    #[test]
    fn demo_record_sanitizes_filename() {
        let dir = test_dir("exopack_test_demo_safe");

        let rec = t67::new("test/bad:name", "egui");
        let path = rec.save(&dir).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(!filename.contains('/'));
        assert!(!filename.contains(':'));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
