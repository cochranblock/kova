// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Hive — lean bidirectional file sync over SSH (absorbed from standalone `ironhive`).
//! Watch local workspace, push deltas to all configured nodes.
//!
//! Config at `~/.ironhive.toml` (preserved for backwards compat with existing setups).

pub mod config;
pub mod sync;
pub mod watcher;

use std::path::Path;
use std::process::Command;

const PID_FILE: &str = ".ironhive.pid";

#[derive(clap::Args)]
pub struct HiveArgs {
    #[command(subcommand)]
    pub cmd: HiveCmd,
}

#[derive(clap::Subcommand)]
pub enum HiveCmd {
    /// Watch workspace and sync changes to all nodes
    Watch,
    /// One-shot full sync to all nodes
    Push,
    /// Show node connectivity
    Status,
    /// Kill daemon (reads .ironhive.pid in workspace)
    Stop,
    /// Show current config
    Config,
}

pub fn dispatch(args: HiveArgs) -> Result<(), String> {
    let cfg = config::Config::load();

    match args.cmd {
        HiveCmd::Watch => {
            let pid_file = Path::new(&cfg.workspace).join(PID_FILE);
            let _ = std::fs::write(&pid_file, std::process::id().to_string());
            let res = watcher::watch_and_sync(&cfg);
            let _ = std::fs::remove_file(pid_file);
            res
        }
        HiveCmd::Push => {
            println!("[hive] pushing to {} nodes...", cfg.nodes.len());
            let results = sync::push_all_nodes(&cfg);
            for (name, result) in &results {
                match result {
                    Ok(()) => println!("  [ok] {}", name),
                    Err(e) => println!("  [XX] {}: {}", name, e),
                }
            }
            let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
            println!("[hive] {}/{} synced", ok, results.len());
            Ok(())
        }
        HiveCmd::Stop => {
            let pid_file = Path::new(&cfg.workspace).join(PID_FILE);
            if !pid_file.exists() {
                println!("[hive] no daemon (no .ironhive.pid)");
                return Ok(());
            }
            let pid_s = std::fs::read_to_string(&pid_file).unwrap_or_default();
            let pid: u32 = pid_s.trim().parse().unwrap_or(0);
            if pid == 0 {
                println!("[hive] invalid pid file");
                let _ = std::fs::remove_file(pid_file);
                return Ok(());
            }
            #[cfg(unix)]
            let _ = Command::new("kill").arg(pid.to_string()).status();
            #[cfg(not(unix))]
            let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).status();
            let _ = std::fs::remove_file(pid_file);
            println!("[hive] stop sent to pid {}", pid);
            Ok(())
        }
        HiveCmd::Status => {
            println!("[hive] checking {} nodes...", cfg.nodes.len());
            for node in &cfg.nodes {
                let status = if sync::check_node(node) { "online" } else { "offline" };
                println!("  {} ({}) — {}", node.name, node.host, status);
            }
            Ok(())
        }
        HiveCmd::Config => {
            println!("config: {}", config::Config::config_path().display());
            println!("workspace: {}", cfg.workspace);
            println!("remote_base: {}", cfg.remote_base);
            println!("excludes: {:?}", cfg.excludes);
            println!("nodes:");
            for node in &cfg.nodes {
                println!("  {} -> {}", node.name, node.host);
            }
            Ok(())
        }
    }
}
