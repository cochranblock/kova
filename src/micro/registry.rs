// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, Mattbusel (registry pattern)
//! registry — Central registry of all micro-model templates.
//! Maps compression tokens (f79, f80, etc) to their MicroTemplate.
//! Inspired by Mattbusel/tokio-prompt-orchestrator's task registry pattern.

use std::collections::HashMap;
use std::path::Path;

use super::template::{builtin_templates, MicroTemplate};

/// The micro-model registry. Holds all known templates indexed by ID.
pub struct MicroRegistry {
    templates: HashMap<String, MicroTemplate>,
}

impl Default for MicroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MicroRegistry {
    /// Build registry from built-in templates.
    pub fn new() -> Self {
        let mut templates = HashMap::new();
        for t in builtin_templates() {
            templates.insert(t.id.clone(), t);
        }
        MicroRegistry { templates }
    }

    /// Load additional templates from a directory of TOML files.
    /// Files are named `<id>.toml` (e.g., `f79.toml`).
    pub fn load_dir(&mut self, dir: &Path) -> Result<usize, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                match MicroTemplate::from_toml(&path) {
                    Ok(t) => {
                        self.templates.insert(t.id.clone(), t);
                        count += 1;
                    }
                    Err(e) => {
                        eprintln!("[micro] failed to load {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(count)
    }

    /// Get a template by compression token.
    pub fn get(&self, id: &str) -> Option<&MicroTemplate> {
        self.templates.get(id)
    }

    /// Get a template by human name.
    pub fn get_by_name(&self, name: &str) -> Option<&MicroTemplate> {
        self.templates.values().find(|t| t.name == name)
    }

    /// All registered template IDs.
    pub fn ids(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// All registered templates.
    pub fn all(&self) -> Vec<&MicroTemplate> {
        self.templates.values().collect()
    }

    /// Count of registered templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// List templates grouped by tier.
    pub fn by_tier(&self) -> HashMap<&str, Vec<&MicroTemplate>> {
        let mut groups: HashMap<&str, Vec<&MicroTemplate>> = HashMap::new();
        for t in self.templates.values() {
            groups.entry(t.tier.as_str()).or_default().push(t);
        }
        groups
    }

    /// Print a status table of all registered micro-models.
    pub fn status(&self) -> String {
        let mut out = String::new();
        out.push_str("Micro-Model Registry\n");
        out.push_str("─────────────────────────────────────────────────────────────────\n");
        out.push_str(&format!(
            "{:<12} {:<18} {:<8} {:<22} {:<6} {}\n",
            "ID", "Name", "Tier", "Model", "Ctx", "Purpose"
        ));
        out.push_str("─────────────────────────────────────────────────────────────────\n");

        let mut templates: Vec<_> = self.templates.values().collect();
        templates.sort_by_key(|t| match t.tier.as_str() {
            "router" => 0,
            "light" => 1,
            "mid" => 2,
            "heavy" => 3,
            _ => 4,
        });

        for t in templates {
            out.push_str(&format!(
                "{:<12} {:<18} {:<8} {:<22} {:<6} {}\n",
                t.id, t.name, t.tier, t.model, t.num_ctx, t.purpose
            ));
        }

        out.push_str(&format!(
            "\n{} micro-models registered\n",
            self.templates.len()
        ));
        out
    }
}
