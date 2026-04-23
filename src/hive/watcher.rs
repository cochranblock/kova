// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! File watcher for the hive module (absorbed from ironhive).

use super::config::Config;
use super::sync;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Debounce interval — batch changes within this window.
const DEBOUNCE_MS: u64 = 500;

/// Start watching the workspace and syncing changes to all nodes.
/// Blocks until Ctrl+C or an error occurs.
pub fn watch_and_sync(cfg: &Config) -> Result<(), String> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))
        .map_err(|e| format!("ctrlc: {}", e))?;
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default())
        .map_err(|e| format!("watcher init failed: {}", e))?;

    let workspace = Path::new(&cfg.workspace);
    watcher
        .watch(workspace, RecursiveMode::Recursive)
        .map_err(|e| format!("watch failed: {}", e))?;

    println!("[hive] watching {} → {} nodes", cfg.workspace, cfg.nodes.len());
    for node in &cfg.nodes {
        let status = if sync::check_node(node) { "online" } else { "offline" };
        println!("  {} ({}) — {}", node.name, node.host, status);
    }

    let mut pending_paths: HashSet<String> = HashSet::new();
    let mut last_sync = Instant::now();

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)) {
            Ok(Ok(event)) => {
                for path in &event.paths {
                    if let Ok(rel) = path.strip_prefix(workspace) {
                        let rel_str = rel.to_string_lossy().to_string();
                        if cfg.excludes.iter().any(|e| rel_str.starts_with(e)) {
                            continue;
                        }
                        pending_paths.insert(rel_str);
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("[hive] watch error: {}", e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !pending_paths.is_empty()
                    && last_sync.elapsed() >= Duration::from_millis(DEBOUNCE_MS)
                {
                    let count = pending_paths.len();
                    if count > 20 {
                        println!("[hive] {} changes — full sync", count);
                        let results = sync::push_all_nodes(cfg);
                        for (name, result) in &results {
                            if let Err(e) = result {
                                eprintln!("[hive] {} sync error: {}", name, e);
                            }
                        }
                    } else {
                        let dirs: HashSet<String> = pending_paths
                            .iter()
                            .map(|p| p.split('/').next().unwrap_or(p).to_string())
                            .collect();
                        for dir in &dirs {
                            let results = sync::push_path_all_nodes(cfg, dir);
                            let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
                            println!("[hive] synced {} → {}/{} nodes", dir, ok, results.len());
                        }
                    }
                    pending_paths.clear();
                    last_sync = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("watcher channel disconnected".into());
            }
        }
    }
    Ok(())
}
