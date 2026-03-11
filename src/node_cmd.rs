// Copyright (c) 2026 The Cochran Block. All rights reserved.
//! Tokenized node commands. §13 compressed output for AI context.
//! nN=node, cN=command, oN=output field.
//! Like Rust macros: token in → execute → compress out.

#![allow(non_camel_case_types)]

use clap::ValueEnum;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

// ── Node Map ──────────────────────────────────────────────

/// nN → SSH hostname.
const NODE_MAP: &[(&str, &str)] = &[
    ("n0", "lf"),
    ("n1", "gd"),
    ("n2", "bt"),
    ("n3", "st"),
];

/// Resolve nN token or pass through raw hostname.
fn resolve_node(s: &str) -> &str {
    NODE_MAP.iter().find(|(k, _)| *k == s).map(|(_, v)| *v).unwrap_or(s)
}

/// Resolve comma-separated node list. None = all.
fn resolve_nodes(input: Option<&str>) -> Vec<String> {
    match input {
        Some(s) => s.split(',')
            .map(|x| resolve_node(x.trim()).to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        None => crate::c2::default_nodes().into_iter().map(String::from).collect(),
    }
}

/// Reverse: hostname → nN token.
fn to_token(hostname: &str) -> &str {
    NODE_MAP.iter().find(|(_, v)| *v == hostname).map(|(k, _)| *k).unwrap_or(hostname)
}

// ── Command Enum (t96) ───────────────────────────────────

/// t96=NodeCmd. Tokenized node command variants.
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum t96 {
    /// c1: nstat — hostname, uptime, load.
    #[value(name = "c1")]
    C1,
    /// c2: nspec — cpu, ram, disk, rust version.
    #[value(name = "c2")]
    C2,
    /// c3: nsvc — running services.
    #[value(name = "c3")]
    C3,
    /// c4: nrust — check/install Rust toolchain.
    #[value(name = "c4")]
    C4,
    /// c5: nsync — rsync project to nodes.
    #[value(name = "c5")]
    C5,
    /// c6: nbuild — remote cargo build.
    #[value(name = "c6")]
    C6,
    /// c7: nlog — tail journalctl.
    #[value(name = "c7")]
    C7,
    /// c8: nkill — kill process by name.
    #[value(name = "c8")]
    C8,
    /// c9: ndeploy — sync + build + restart.
    #[value(name = "c9")]
    C9,
    /// ci: compact inspect — one-line-per-node.
    #[value(name = "ci")]
    Ci,
}

// ── Result Type (t97) ────────────────────────────────────

/// t97=NodeResult. Per-node command output.
pub struct t97 {
    /// s14: node token (n0..n3).
    pub s14: String,
    /// s15: success.
    pub s15: bool,
    /// s16: output field pairs (oN, value).
    pub s16: Vec<(&'static str, String)>,
}

// ── SSH Helpers ──────────────────────────────────────────

const SSH_OPTS: &[&str] = &[
    "-o", "ConnectTimeout=5",
    "-o", "StrictHostKeyChecking=accept-new",
    "-o", "BatchMode=yes",
];

/// Run SSH command on a single node. Returns (success, stdout).
fn ssh_exec(node: &str, cmd: &str) -> (bool, String) {
    let mut args: Vec<&str> = SSH_OPTS.to_vec();
    args.push(node);
    args.push(cmd);
    match Command::new("ssh").args(&args).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if out.status.success() {
                (true, stdout)
            } else {
                (false, stderr)
            }
        }
        Err(e) => (false, e.to_string()),
    }
}


// ── Per-Command Functions ────────────────────────────────

/// f122=nstat. Hostname, uptime, load.
fn f122(nodes: &[String]) -> Vec<t97> {
    let cmd = r#"h=$(hostname); u=$(uptime -p 2>/dev/null || uptime | sed 's/.*up //' | cut -d, -f1-2); l=$(cat /proc/loadavg | cut -d' ' -f1); echo "$h"; echo "$u"; echo "$l""#;
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.to_string();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            if ok {
                let lines: Vec<&str> = out.lines().collect();
                let host = lines.first().unwrap_or(&"—").to_string();
                let up = lines.get(1).unwrap_or(&"—").to_string()
                    .replace(" days", "d").replace(" day", "d")
                    .replace(" hours", "h").replace(" hour", "h")
                    .replace(" minutes", "m").replace(" minute", "m")
                    .replace("up ", "").replace(", ", "").replace(' ', "");
                let load = lines.get(2).unwrap_or(&"—").to_string();
                let _ = tx.send(t97 {
                    s14: token, s15: true,
                    s16: vec![("o1", host), ("o2", up), ("o3", load)],
                });
            } else {
                let _ = tx.send(t97 {
                    s14: token, s15: false,
                    s16: vec![("o10", "err".into()), ("o11", out)],
                });
            }
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f123=nspec. CPU cores, RAM, disk, rust version.
fn f123(nodes: &[String]) -> Vec<t97> {
    let cmd = r#"echo $(nproc); free -g 2>/dev/null | awk '/Mem:/{print $2}' || echo 0; df -BG / 2>/dev/null | tail -1 | awk '{gsub("G",""); print $4}' || echo 0; rustc --version 2>/dev/null | awk '{print $2}' || echo —"#;
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.to_string();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            if ok {
                let lines: Vec<&str> = out.lines().collect();
                let cpu = lines.first().unwrap_or(&"—").to_string();
                let ram = lines.get(1).unwrap_or(&"—").to_string();
                let disk = lines.get(2).unwrap_or(&"—").to_string();
                let rust = lines.get(3).unwrap_or(&"—").to_string();
                let _ = tx.send(t97 {
                    s14: token, s15: true,
                    s16: vec![("o4", cpu), ("o5", format!("{}G", ram)), ("o6", format!("{}G", disk)), ("o8", rust)],
                });
            } else {
                let _ = tx.send(t97 { s14: token, s15: false, s16: vec![("o10", "err".into())] });
            }
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f124=nsvc. Running services, filtered.
fn f124(nodes: &[String]) -> Vec<t97> {
    let cmd = r#"systemctl list-units --type=service --state=running --no-pager --no-legend 2>/dev/null | awk '{print $1}' | sed 's/.service//' | grep -v -E '^(systemd|dbus|user@|getty|ssh[d]?|cron|rsyslog|snapd|unattended|multipathd|polkit|upower|accounts|network|ModemManager|wpa_supplicant|thermald|irqbalance|packagekit|udisks|fwupd|power-profiles|switcheroo|rtkit|avahi|bluetooth|colord|cups)' | sort"#;
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.to_string();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            let svcs = if ok { out.lines().collect::<Vec<_>>().join(",") } else { "err".into() };
            let _ = tx.send(t97 { s14: token, s15: ok, s16: vec![("o9", svcs)] });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f125=nrust. Check Rust toolchain. Install with --install.
fn f125(nodes: &[String], install: bool) -> Vec<t97> {
    let cmd = if install {
        r#"if command -v rustc >/dev/null 2>&1; then rustc --version | awk '{print $2}'; else curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y 2>&1 | tail -1 && . "$HOME/.cargo/env" && rustc --version | awk '{print $2}'; fi"#
    } else {
        r#"rustc --version 2>/dev/null | awk '{print $2}' || echo none; cargo --version 2>/dev/null | awk '{print $2}' || echo none"#
    };
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.to_string();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            let lines: Vec<&str> = out.lines().collect();
            let rust_v = lines.first().unwrap_or(&"—").to_string();
            let cargo_v = lines.get(1).unwrap_or(&"—").to_string();
            let _ = tx.send(t97 {
                s14: token, s15: ok,
                s16: vec![("o8", rust_v), ("o11", cargo_v)],
            });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f126=nsync. Rsync project to nodes. Delegates to c2::sync_parallel.
fn f126(nodes: &[String], project: &std::path::Path) -> Vec<t97> {
    let node_strs: Vec<String> = nodes.to_vec();
    let name = project.file_name().unwrap_or_default().to_string_lossy().to_string();
    // Use rsync directly: simpler for single project sync.
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = node_strs.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let src = project.to_path_buf();
        let name = name.clone();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let dst = format!("{}:/tmp/kova-build/{}/", node, name);
            // Ensure dir exists.
            let _ = ssh_exec(&node, &format!("mkdir -p /tmp/kova-build/{}", name));
            let status = Command::new("rsync")
                .args(["-az", "--delete", "--exclude", "target/", "--exclude", ".git/"])
                .arg(format!("{}/", src.display()))
                .arg(&dst)
                .status();
            let ok = status.map(|s| s.success()).unwrap_or(false);
            let _ = tx.send(t97 {
                s14: token, s15: ok,
                s16: vec![("o10", if ok { "ok" } else { "err" }.into()), ("o11", name)],
            });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f127=nbuild. Remote cargo build.
fn f127(nodes: &[String], project_name: &str, release: bool) -> Vec<t97> {
    let flag = if release { " --release" } else { "" };
    let cmd = format!(
        "cd /tmp/kova-build/{} && $HOME/.cargo/bin/cargo build{} 2>&1 | tail -3",
        project_name, flag
    );
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.clone();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            let _ = tx.send(t97 {
                s14: token, s15: ok,
                s16: vec![("o10", if ok { "ok" } else { "err" }.into()), ("o11", out.trim().to_string())],
            });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f128=nlog. Tail journalctl.
fn f128(nodes: &[String], unit: Option<&str>, lines: u32) -> Vec<t97> {
    let unit_flag = unit.map(|u| format!("-u {}", u)).unwrap_or_default();
    let cmd = format!(
        "journalctl {} --no-pager -n {} --output=short-iso 2>/dev/null || tail -{} /var/log/syslog 2>/dev/null || echo no_logs",
        unit_flag, lines, lines
    );
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.clone();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            let _ = tx.send(t97 {
                s14: token, s15: ok,
                s16: vec![("o11", out.trim().to_string())],
            });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f129=nkill. Kill process by name.
fn f129(nodes: &[String], proc_name: &str) -> Vec<t97> {
    let cmd = format!(
        "pids=$(pgrep -f '{}' 2>/dev/null); if [ -n \"$pids\" ]; then kill $pids && echo killed; else echo not_found; fi",
        proc_name
    );
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.clone();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            let _ = tx.send(t97 {
                s14: token, s15: ok,
                s16: vec![("o10", out.trim().to_string())],
            });
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

/// f130=ndeploy. Sync + build + optional restart.
fn f130(nodes: &[String], project: &std::path::Path, release: bool, service: Option<&str>) -> Vec<t97> {
    let name = project.file_name().unwrap_or_default().to_string_lossy().to_string();

    // Phase 1: sync
    eprintln!("[f130] sync {}...", name);
    let sync_results = f126(nodes, project);
    let failed: Vec<_> = sync_results.iter().filter(|r| !r.s15).map(|r| r.s14.clone()).collect();
    if !failed.is_empty() {
        eprintln!("[f130] sync failed on: {}", failed.join(","));
    }

    // Phase 2: build
    eprintln!("[f130] build {}{}...", name, if release { " --release" } else { "" });
    let build_results = f127(nodes, &name, release);

    // Phase 3: restart (if service specified)
    if let Some(svc) = service {
        eprintln!("[f130] restart {}...", svc);
        let cmd = format!("sudo systemctl restart {} 2>&1 || echo restart_failed", svc);
        let (tx, rx) = mpsc::channel::<t97>();
        let handles: Vec<_> = nodes.iter().map(|node| {
            let tx = tx.clone();
            let node = node.clone();
            let cmd = cmd.clone();
            thread::spawn(move || {
                let token = to_token(&node).to_string();
                let (ok, out) = ssh_exec(&node, &cmd);
                let _ = tx.send(t97 {
                    s14: token, s15: ok,
                    s16: vec![("o10", if ok { "ok" } else { "err" }.into()), ("o11", out.trim().to_string())],
                });
            })
        }).collect();
        drop(tx);
        let results: Vec<t97> = rx.into_iter().collect();
        for h in handles { let _ = h.join(); }
        return results;
    }

    build_results
}

/// f131=nci. Compact inspect — one-line-per-node summary.
fn f131(nodes: &[String]) -> Vec<t97> {
    let cmd = r#"echo "$(nproc) $(free -g|awk '/Mem:/{print $3}')G $(cat /proc/loadavg|cut -d' ' -f1) $(hostname)""#;
    let (tx, rx) = mpsc::channel::<t97>();
    let handles: Vec<_> = nodes.iter().map(|node| {
        let tx = tx.clone();
        let node = node.clone();
        let cmd = cmd.to_string();
        thread::spawn(move || {
            let token = to_token(&node).to_string();
            let (ok, out) = ssh_exec(&node, &cmd);
            if ok {
                let parts: Vec<&str> = out.split_whitespace().collect();
                let cpu = parts.first().unwrap_or(&"—").to_string();
                let mem = parts.get(1).unwrap_or(&"—").to_string();
                let load = parts.get(2).unwrap_or(&"—").to_string();
                let host = parts.get(3).unwrap_or(&"—").to_string();
                let _ = tx.send(t97 {
                    s14: token, s15: true,
                    s16: vec![("o4", cpu), ("o5", mem), ("o3", load), ("o1", host)],
                });
            } else {
                let _ = tx.send(t97 { s14: token, s15: false, s16: vec![("o10", "err".into())] });
            }
        })
    }).collect();
    drop(tx);
    let results: Vec<t97> = rx.into_iter().collect();
    for h in handles { let _ = h.join(); }
    results
}

// ── Output Formatting ────────────────────────────────────

/// Column headers per command.
fn headers_for(cmd: &t96) -> &'static [&'static str] {
    match cmd {
        t96::C1 => &["o0", "o1", "o2", "o3"],
        t96::C2 => &["o0", "o4", "o5", "o6", "o8"],
        t96::C3 => &["o0", "o9"],
        t96::C4 => &["o0", "o8", "o11"],
        t96::C5 | t96::C6 | t96::C9 => &["o0", "o10", "o11"],
        t96::C7 => &["o0", "o11"],
        t96::C8 => &["o0", "o10"],
        t96::Ci => &["o0", "o4", "o5", "o3", "o1"],
    }
}

/// Expand oN → human-readable name.
fn expand_header(token: &str) -> &str {
    match token {
        "o0" => "node",
        "o1" => "host",
        "o2" => "up",
        "o3" => "load",
        "o4" => "cpu",
        "o5" => "mem",
        "o6" => "disk",
        "o7" => "disk_total",
        "o8" => "rust",
        "o9" => "svcs",
        "o10" => "status",
        "o11" => "msg",
        _ => token,
    }
}

/// Print compressed table output.
fn print_compressed(cmd: &t96, results: &[t97], expand: bool) {
    let hdrs = headers_for(cmd);
    // For c7/nlog, print per-node blocks instead of table.
    if matches!(cmd, t96::C7) {
        for r in results {
            eprintln!("[{}]", r.s14);
            if let Some((_, v)) = r.s16.iter().find(|(k, _)| *k == "o11") {
                eprintln!("{}", v);
            }
        }
        return;
    }

    // Header row.
    let header: Vec<&str> = hdrs.iter().map(|h| {
        if expand { expand_header(h) } else { *h }
    }).collect();
    eprintln!("{}", header.join("\t"));

    // Data rows.
    for r in results {
        let mut row: Vec<String> = Vec::new();
        for h in hdrs {
            if *h == "o0" {
                row.push(r.s14.clone());
            } else if let Some((_, v)) = r.s16.iter().find(|(k, _)| k == h) {
                row.push(v.clone());
            } else if !r.s15 {
                row.push("err".into());
            } else {
                row.push("—".into());
            }
        }
        eprintln!("{}", row.join("\t"));
    }
}

// ── Dispatcher (f132) ────────────────────────────────────

/// f132=node_cmd_dispatch. Central dispatcher for c1-c9/ci tokens.
pub fn f132(
    cmd: t96,
    nodes: Option<String>,
    extra: Option<String>,
    release: bool,
    lines: u32,
    expand: bool,
) -> anyhow::Result<()> {
    let node_list = resolve_nodes(nodes.as_deref());

    let results = match cmd {
        t96::C1 => f122(&node_list),
        t96::C2 => f123(&node_list),
        t96::C3 => f124(&node_list),
        t96::C4 => {
            let install = extra.as_deref() == Some("install");
            f125(&node_list, install)
        }
        t96::C5 => {
            let path = extra.as_deref().unwrap_or(".");
            let project = crate::c2::resolve_project(Some(std::path::PathBuf::from(path)));
            f126(&node_list, &project)
        }
        t96::C6 => {
            let project = crate::c2::resolve_project(extra.map(std::path::PathBuf::from));
            let name = project.file_name().unwrap_or_default().to_string_lossy().to_string();
            f127(&node_list, &name, release)
        }
        t96::C7 => f128(&node_list, extra.as_deref(), lines),
        t96::C8 => {
            let proc_name = extra.as_deref()
                .ok_or_else(|| anyhow::anyhow!("c8 requires --extra <process_name>"))?;
            f129(&node_list, proc_name)
        }
        t96::C9 => {
            let project = crate::c2::resolve_project(extra.as_ref().map(std::path::PathBuf::from));
            f130(&node_list, &project, release, None)
        }
        t96::Ci => f131(&node_list),
    };

    print_compressed(&cmd, &results, expand);

    let any_err = results.iter().any(|r| !r.s15);
    if any_err && results.iter().all(|r| !r.s15) {
        anyhow::bail!("All nodes failed");
    }
    Ok(())
}
