// Unlicense — public domain — cochranblock.org
//! checkpoint — test harness for checkpoint/undo patterns.
//! Verifies the contract: snapshot before write, restore on undo.
//! Pure std — no sled dependency. Tests the file-level behavior.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// t73: A file checkpoint store backed by in-memory HashMap.
/// Mirrors kova f383/f384 contract without requiring sled.
#[derive(Debug, Default)]
pub struct t73 {
    /// s87: snapshots keyed by filepath → Vec of (timestamp_ms, content)
    s87: HashMap<PathBuf, Vec<(u128, Vec<u8>)>>,
}

/// t74: Result of a checkpoint or undo operation
#[derive(Debug, Clone)]
pub struct t74 {
    /// s88: operation succeeded
    pub s88: bool,
    /// s89: detail message
    pub s89: String,
    /// s90: bytes affected
    pub s90: usize,
}

impl t73 {
    /// f120: Create a new checkpoint store
    pub fn new() -> Self {
        Self::default()
    }

    /// f121: Snapshot file contents before write/edit.
    /// Returns None if file doesn't exist (new files have nothing to restore).
    pub fn f121(&mut self, path: &Path) -> Option<t74> {
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let size = content.len();
        self.s87
            .entry(path.to_path_buf())
            .or_default()
            .push((ts, content));

        Some(t74 {
            s88: true,
            s89: format!("checkpoint: {} ({} bytes)", path.display(), size),
            s90: size,
        })
    }

    /// f122: Restore file from last checkpoint (undo).
    /// Returns error if no checkpoint exists for this path.
    pub fn f122(&self, path: &Path) -> t74 {
        let snapshots = match self.s87.get(path) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return t74 {
                    s88: false,
                    s89: format!("no checkpoint for {}", path.display()),
                    s90: 0,
                }
            }
        };

        let (_, content) = &snapshots[snapshots.len() - 1];

        match std::fs::write(path, content) {
            Ok(_) => t74 {
                s88: true,
                s89: format!("restored {} ({} bytes)", path.display(), content.len()),
                s90: content.len(),
            },
            Err(e) => t74 {
                s88: false,
                s89: format!("restore failed: {}", e),
                s90: 0,
            },
        }
    }

    /// f123: Restore file from Nth checkpoint (0 = oldest, -1 = latest).
    /// Allows undo to any prior state.
    pub fn f123(&self, path: &Path, index: i32) -> t74 {
        let snapshots = match self.s87.get(path) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return t74 {
                    s88: false,
                    s89: format!("no checkpoint for {}", path.display()),
                    s90: 0,
                }
            }
        };

        let resolved = if index < 0 {
            let idx = snapshots.len() as i32 + index;
            if idx < 0 {
                return t74 {
                    s88: false,
                    s89: format!("index {} out of range (have {})", index, snapshots.len()),
                    s90: 0,
                };
            }
            idx as usize
        } else {
            index as usize
        };

        if resolved >= snapshots.len() {
            return t74 {
                s88: false,
                s89: format!("index {} out of range (have {})", resolved, snapshots.len()),
                s90: 0,
            };
        }

        let (_, content) = &snapshots[resolved];
        match std::fs::write(path, content) {
            Ok(_) => t74 {
                s88: true,
                s89: format!(
                    "restored {} to checkpoint {} ({} bytes)",
                    path.display(),
                    resolved,
                    content.len()
                ),
                s90: content.len(),
            },
            Err(e) => t74 {
                s88: false,
                s89: format!("restore failed: {}", e),
                s90: 0,
            },
        }
    }

    /// f124: Count checkpoints for a path
    pub fn f124(&self, path: &Path) -> usize {
        self.s87.get(path).map(|s| s.len()).unwrap_or(0)
    }

    /// f125: Verify file matches its latest checkpoint (no drift)
    pub fn f125(&self, path: &Path) -> t74 {
        let snapshots = match self.s87.get(path) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return t74 {
                    s88: false,
                    s89: "no checkpoint to verify against".into(),
                    s90: 0,
                }
            }
        };

        let (_, expected) = &snapshots[snapshots.len() - 1];
        let actual = match std::fs::read(path) {
            Ok(c) => c,
            Err(e) => {
                return t74 {
                    s88: false,
                    s89: format!("read failed: {}", e),
                    s90: 0,
                }
            }
        };

        if actual == *expected {
            t74 {
                s88: true,
                s89: "file matches checkpoint".into(),
                s90: actual.len(),
            }
        } else {
            t74 {
                s88: false,
                s89: format!(
                    "drift: file is {} bytes, checkpoint is {} bytes",
                    actual.len(),
                    expected.len()
                ),
                s90: actual.len(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("exopack_ckpt_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn checkpoint_and_undo_cycle() {
        let dir = tmp("cycle");
        let file = dir.join("test.txt");

        fs::write(&file, "version 1").unwrap();

        let mut store = t73::new();
        let result = store.f121(&file);
        assert!(result.is_some());
        assert!(result.unwrap().s88);

        // Modify file
        fs::write(&file, "version 2").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "version 2");

        // Undo
        let undo = store.f122(&file);
        assert!(undo.s88, "undo should succeed: {}", undo.s89);
        assert_eq!(fs::read_to_string(&file).unwrap(), "version 1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_checkpoint_for_new_file() {
        let dir = tmp("new");
        let file = dir.join("nonexistent.txt");

        let mut store = t73::new();
        assert!(store.f121(&file).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn multiple_checkpoints_undo_to_latest() {
        let dir = tmp("multi");
        let file = dir.join("test.txt");

        let mut store = t73::new();

        fs::write(&file, "v1").unwrap();
        store.f121(&file);

        fs::write(&file, "v2").unwrap();
        store.f121(&file);

        fs::write(&file, "v3").unwrap();
        store.f121(&file);

        assert_eq!(store.f124(&file), 3);

        // Undo restores to v3 (latest checkpoint)
        fs::write(&file, "v4 modified").unwrap();
        let undo = store.f122(&file);
        assert!(undo.s88);
        assert_eq!(fs::read_to_string(&file).unwrap(), "v3");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_to_specific_index() {
        let dir = tmp("index");
        let file = dir.join("test.txt");

        let mut store = t73::new();

        fs::write(&file, "v1").unwrap();
        store.f121(&file);
        fs::write(&file, "v2").unwrap();
        store.f121(&file);
        fs::write(&file, "v3").unwrap();
        store.f121(&file);

        // Undo to oldest (index 0)
        let undo = store.f123(&file, 0);
        assert!(undo.s88);
        assert_eq!(fs::read_to_string(&file).unwrap(), "v1");

        // Undo to middle (index 1)
        let undo = store.f123(&file, 1);
        assert!(undo.s88);
        assert_eq!(fs::read_to_string(&file).unwrap(), "v2");

        // Undo to latest via negative index
        let undo = store.f123(&file, -1);
        assert!(undo.s88);
        assert_eq!(fs::read_to_string(&file).unwrap(), "v3");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_out_of_range() {
        let dir = tmp("range");
        let file = dir.join("test.txt");

        let mut store = t73::new();
        fs::write(&file, "v1").unwrap();
        store.f121(&file);

        let undo = store.f123(&file, 5);
        assert!(!undo.s88);
        assert!(undo.s89.contains("out of range"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_detects_drift() {
        let dir = tmp("drift");
        let file = dir.join("test.txt");

        let mut store = t73::new();
        fs::write(&file, "original").unwrap();
        store.f121(&file);

        // Verify matches
        let v = store.f125(&file);
        assert!(v.s88, "should match: {}", v.s89);

        // Modify without checkpoint
        fs::write(&file, "modified").unwrap();
        let v = store.f125(&file);
        assert!(!v.s88, "should detect drift");
        assert!(v.s89.contains("drift"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_no_checkpoint_fails() {
        let store = t73::new();
        let file = PathBuf::from("/tmp/exopack_ckpt_nope");
        let undo = store.f122(&file);
        assert!(!undo.s88);
        assert!(undo.s89.contains("no checkpoint"));
    }

    #[test]
    fn binary_file_checkpoint() {
        let dir = tmp("binary");
        let file = dir.join("data.bin");

        let binary_data: Vec<u8> = (0..=255).collect();
        fs::write(&file, &binary_data).unwrap();

        let mut store = t73::new();
        store.f121(&file);

        fs::write(&file, b"overwritten").unwrap();
        let undo = store.f122(&file);
        assert!(undo.s88);
        assert_eq!(fs::read(&file).unwrap(), binary_data);

        let _ = fs::remove_dir_all(&dir);
    }
}
