// Unlicense — public domain — cochranblock.org
//! perm_gate — test harness for tool permission gates.
//! Verifies: open/guarded modes, exec tool rename, git mutation detection.
//! Pure std — no runtime dep. Tests the permission contract.

/// t79: Permission mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum t79 {
    /// All tools run without prompts
    Open,
    /// exec + git mutations require approval
    Guarded,
}

/// t80: Permission check result
#[derive(Debug, Clone)]
pub struct t80 {
    /// s104: tool name
    pub s104: String,
    /// s105: allowed (true = proceed, false = needs approval)
    pub s105: bool,
    /// s106: reason
    pub s106: String,
}

/// Git mutation patterns that require approval in guarded mode
const GIT_MUTATIONS: &[&str] = &[
    "git commit",
    "git push",
    "--force",
    "--no-verify",
    "git reset --hard",
    "git checkout --",
    "git clean -f",
];

/// f136: Parse permission mode from env var value.
pub fn f136(env_value: &str) -> t79 {
    match env_value.to_lowercase().as_str() {
        "guarded" => t79::Guarded,
        _ => t79::Open,
    }
}

/// f137: Check if a tool call is allowed under the given permission mode.
/// In open mode, everything is allowed.
/// In guarded mode, "exec" requires approval, and git mutations are flagged.
pub fn f137(mode: &t79, tool_name: &str, command: Option<&str>) -> t80 {
    match mode {
        t79::Open => t80 {
            s104: tool_name.to_string(),
            s105: true,
            s106: "open mode — all allowed".into(),
        },
        t79::Guarded => {
            // exec tool (or legacy "bash" name) requires approval
            if tool_name == "exec" || tool_name == "bash" {
                if let Some(cmd) = command {
                    // Check for git mutations
                    for pattern in GIT_MUTATIONS {
                        if cmd.contains(pattern) {
                            return t80 {
                                s104: tool_name.to_string(),
                                s105: false,
                                s106: format!("git mutation detected: {}", pattern),
                            };
                        }
                    }
                }
                t80 {
                    s104: tool_name.to_string(),
                    s105: false,
                    s106: "exec requires approval in guarded mode".into(),
                }
            } else {
                // Non-exec tools (read, write, edit, glob, grep) are always allowed
                t80 {
                    s104: tool_name.to_string(),
                    s105: true,
                    s106: format!("{} allowed in guarded mode", tool_name),
                }
            }
        }
    }
}

/// f138: Check backward compatibility — "bash" maps to "exec".
pub fn f138(tool_name: &str) -> &str {
    if tool_name == "bash" {
        "exec"
    } else {
        tool_name
    }
}

/// f139: Detect git mutations in a command string. Returns all matched patterns.
pub fn f139(command: &str) -> Vec<&'static str> {
    GIT_MUTATIONS
        .iter()
        .filter(|p| command.contains(**p))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_open_mode() {
        assert_eq!(f136("open"), t79::Open);
        assert_eq!(f136(""), t79::Open);
        assert_eq!(f136("anything"), t79::Open);
    }

    #[test]
    fn parse_guarded_mode() {
        assert_eq!(f136("guarded"), t79::Guarded);
        assert_eq!(f136("GUARDED"), t79::Guarded);
    }

    #[test]
    fn open_allows_everything() {
        let mode = t79::Open;
        assert!(f137(&mode, "exec", Some("rm -rf /")).s105);
        assert!(f137(&mode, "read_file", None).s105);
        assert!(f137(&mode, "exec", Some("git push --force")).s105);
    }

    #[test]
    fn guarded_blocks_exec() {
        let mode = t79::Guarded;
        let r = f137(&mode, "exec", Some("ls -la"));
        assert!(!r.s105, "exec should need approval: {}", r.s106);
    }

    #[test]
    fn guarded_blocks_bash_alias() {
        let mode = t79::Guarded;
        let r = f137(&mode, "bash", Some("ls -la"));
        assert!(!r.s105, "bash (legacy) should need approval");
    }

    #[test]
    fn guarded_allows_read_tools() {
        let mode = t79::Guarded;
        for tool in &["read_file", "write_file", "edit_file", "glob", "grep"] {
            let r = f137(&mode, tool, None);
            assert!(r.s105, "{} should be allowed: {}", tool, r.s106);
        }
    }

    #[test]
    fn guarded_detects_git_mutations() {
        let mode = t79::Guarded;
        let blocked_cmds = vec![
            "git commit -m 'test'",
            "git push origin main",
            "git push --force",
            "git reset --hard HEAD~1",
            "git checkout -- .",
            "git clean -f",
            "git commit --no-verify",
        ];

        for cmd in blocked_cmds {
            let r = f137(&mode, "exec", Some(cmd));
            assert!(!r.s105, "should block: {}", cmd);
            assert!(
                r.s106.contains("git mutation"),
                "should cite git mutation for '{}': {}",
                cmd,
                r.s106
            );
        }
    }

    #[test]
    fn guarded_allows_safe_git() {
        let mode = t79::Guarded;
        // git status, git log, git diff are safe — still blocked as exec, but not as git mutation
        let r = f137(&mode, "exec", Some("git status"));
        assert!(!r.s105); // blocked as exec
        assert!(
            !r.s106.contains("git mutation"),
            "git status is not a mutation"
        );
    }

    #[test]
    fn bash_to_exec_rename() {
        assert_eq!(f138("bash"), "exec");
        assert_eq!(f138("exec"), "exec");
        assert_eq!(f138("read_file"), "read_file");
    }

    #[test]
    fn detect_multiple_mutations() {
        let cmd = "git commit --no-verify && git push --force";
        let mutations = f139(cmd);
        assert!(mutations.contains(&"git commit"));
        assert!(mutations.contains(&"--no-verify"));
        assert!(mutations.contains(&"git push"));
        assert!(mutations.contains(&"--force"));
        assert_eq!(mutations.len(), 4);
    }

    #[test]
    fn no_mutations_in_safe_commands() {
        assert!(f139("git status").is_empty());
        assert!(f139("git log --oneline -10").is_empty());
        assert!(f139("git diff HEAD").is_empty());
        assert!(f139("cargo build").is_empty());
    }
}
