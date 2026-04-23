// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! SSH-based rsync push for the hive module (absorbed from ironhive).

use super::config::{Config, Node};
use std::path::Path;
use std::process::Command;

/// Push a specific path (file or dir) to a single node.
pub fn push_path(cfg: &Config, node: &Node, rel_path: &str) -> Result<(), String> {
    let src = format!("{}/{}", cfg.workspace, rel_path);
    let dst = format!("{}:{}/{}", node.host, cfg.remote_base, rel_path);

    let (src, dst) = if Path::new(&src).is_dir() {
        (format!("{}/", src), format!("{}/", dst))
    } else {
        if let Some(parent) = Path::new(rel_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = Command::new("ssh")
                    .args([
                        "-o", "ConnectTimeout=3",
                        "-o", "BatchMode=yes",
                        &node.host,
                        &format!("mkdir -p {}/{}", cfg.remote_base, parent.display()),
                    ])
                    .output();
            }
        }
        (src, dst)
    };

    let mut args: Vec<String> = vec!["-az".into(), "--delete".into()];
    args.extend(cfg.rsync_excludes());
    args.push(src);
    args.push(dst);

    let output = Command::new("rsync")
        .args(&args)
        .output()
        .map_err(|e| format!("rsync to {} failed: {}", node.name, e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "rsync to {} exit {}: {}",
            node.name,
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Push entire workspace to a single node.
pub fn push_all(cfg: &Config, node: &Node) -> Result<(), String> {
    let src = format!("{}/", cfg.workspace);
    let dst = format!("{}:{}/", node.host, cfg.remote_base);

    let mut args: Vec<String> = vec!["-az".into(), "--delete".into()];
    args.extend(cfg.rsync_excludes());
    args.push(src);
    args.push(dst);

    let output = Command::new("rsync")
        .args(&args)
        .output()
        .map_err(|e| format!("rsync to {} failed: {}", node.name, e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "rsync to {} exit {}: {}",
            node.name,
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Push to all nodes concurrently. Returns vec of (node_name, result).
pub fn push_all_nodes(cfg: &Config) -> Vec<(String, Result<(), String>)> {
    let handles: Vec<_> = cfg
        .nodes
        .iter()
        .map(|node| {
            let cfg = cfg.clone();
            let node = node.clone();
            std::thread::spawn(move || {
                let result = push_all(&cfg, &node);
                (node.name.clone(), result)
            })
        })
        .collect();

    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Push a changed path to all nodes concurrently.
pub fn push_path_all_nodes(cfg: &Config, rel_path: &str) -> Vec<(String, Result<(), String>)> {
    let handles: Vec<_> = cfg
        .nodes
        .iter()
        .map(|node| {
            let cfg = cfg.clone();
            let node = node.clone();
            let path = rel_path.to_string();
            std::thread::spawn(move || {
                let result = push_path(&cfg, &node, &path);
                (node.name.clone(), result)
            })
        })
        .collect();

    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Check if a node is reachable via SSH.
pub fn check_node(node: &Node) -> bool {
    Command::new("ssh")
        .args(["-o", "ConnectTimeout=2", "-o", "BatchMode=yes", &node.host, "true"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
