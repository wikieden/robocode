use super::*;

impl SessionEngine {
    pub(super) fn handle_git_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_git_help());
        };
        match subcommand {
            "help" => Ok(self.render_git_help()),
            "status" => self.run_named_tool("git_status", Default::default(), approver),
            "diff" => {
                let mut input = robocode_types::ToolInput::new();
                if let Some(path) = args.get(1) {
                    input.insert("path".to_string(), path.clone());
                }
                self.run_named_tool("git_diff", input, approver)
            }
            "branch" => self.run_named_tool("git_branch", Default::default(), approver),
            "add" => {
                let all = args.iter().any(|arg| arg == "--all" || arg == "-A");
                let paths: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--all" && arg.as_str() != "-A")
                    .cloned()
                    .collect();
                if paths.is_empty() && !all {
                    return Err("Usage: /git add [--all|-A] <path...>".to_string());
                }
                let mut input = robocode_types::ToolInput::new();
                if all {
                    input.insert("all".to_string(), "true".to_string());
                }
                if let Some(path) = paths.first() {
                    input.insert("path".to_string(), path.clone());
                }
                if paths.len() > 1 {
                    input.insert("paths".to_string(), paths.join("\n"));
                }
                self.run_named_tool("git_add", input, approver)
            }
            "restore" => {
                let staged = args.iter().any(|arg| arg == "--staged");
                let worktree = !args.iter().any(|arg| arg == "--worktree=false");
                let mut source = None;
                let mut paths = Vec::new();
                let mut iter = args.iter().skip(1);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--staged" | "--worktree" => {}
                        "--worktree=false" => {}
                        "--source" => {
                            source = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /git restore [--staged] [--source <ref>] <path...>"
                                    .to_string()
                            })?);
                        }
                        other if other.starts_with("--source=") => {
                            source = Some(other.trim_start_matches("--source=").to_string());
                        }
                        other => paths.push(other.to_string()),
                    }
                }
                if paths.is_empty() {
                    return Err(
                        "Usage: /git restore [--staged] [--source <ref>] <path...>".to_string()
                    );
                }
                let mut input = robocode_types::ToolInput::new();
                if staged {
                    input.insert("staged".to_string(), "true".to_string());
                }
                if !worktree {
                    input.insert("worktree".to_string(), "false".to_string());
                }
                if let Some(source) = source {
                    input.insert("source".to_string(), source);
                }
                if let Some(path) = paths.first() {
                    input.insert("path".to_string(), path.clone());
                }
                if paths.len() > 1 {
                    input.insert("paths".to_string(), paths.join("\n"));
                }
                self.run_named_tool("git_restore", input, approver)
            }
            "switch" | "checkout" => {
                let branch = args
                    .get(1)
                    .cloned()
                    .ok_or_else(|| "Usage: /git switch <branch> [--create]".to_string())?;
                let create = args.iter().any(|arg| arg == "--create" || arg == "-c");
                let mut input = robocode_types::ToolInput::new();
                input.insert("branch".to_string(), branch);
                if create {
                    input.insert("create".to_string(), "true".to_string());
                }
                self.run_named_tool("git_switch", input, approver)
            }
            "commit" => {
                let all = args.iter().any(|arg| arg == "--all" || arg == "-a");
                let message_parts: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--all" && arg.as_str() != "-a")
                    .cloned()
                    .collect();
                if message_parts.is_empty() {
                    return Err("Usage: /git commit [--all] <message>".to_string());
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("message".to_string(), message_parts.join(" "));
                if all {
                    input.insert("all".to_string(), "true".to_string());
                }
                self.run_named_tool("git_commit", input, approver)
            }
            "push" => {
                let set_upstream = args
                    .iter()
                    .any(|arg| arg == "--set-upstream" || arg == "-u");
                let positional: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--set-upstream" && arg.as_str() != "-u")
                    .cloned()
                    .collect();
                let mut input = robocode_types::ToolInput::new();
                if set_upstream {
                    input.insert("set_upstream".to_string(), "true".to_string());
                }
                match positional.as_slice() {
                    [] => {}
                    [branch] => {
                        input.insert("branch".to_string(), branch.clone());
                    }
                    [remote, branch] => {
                        input.insert("remote".to_string(), remote.clone());
                        input.insert("branch".to_string(), branch.clone());
                    }
                    _ => {
                        return Err(
                            "Usage: /git push [branch] | [remote branch] [--set-upstream|-u]"
                                .to_string(),
                        );
                    }
                }
                self.run_named_tool("git_push", input, approver)
            }
            "stash" => {
                let Some(action) = args.get(1).map(String::as_str) else {
                    return Ok(self.render_git_stash_help());
                };
                match action {
                    "help" => Ok(self.render_git_stash_help()),
                    "list" => self.run_named_tool("git_stash_list", Default::default(), approver),
                    "push" | "save" => {
                        let mut include_untracked = false;
                        let mut message = None;
                        let mut paths = Vec::new();
                        let mut iter = args.iter().skip(2);
                        while let Some(arg) = iter.next() {
                            match arg.as_str() {
                                "--include-untracked" | "-u" => include_untracked = true,
                                "--message" | "-m" => {
                                    message = Some(iter.next().cloned().ok_or_else(|| {
                                        "Usage: /git stash push [-m <message>] [-u] [path...]"
                                            .to_string()
                                    })?);
                                }
                                other if other.starts_with("--message=") => {
                                    message =
                                        Some(other.trim_start_matches("--message=").to_string());
                                }
                                other => paths.push(other.to_string()),
                            }
                        }
                        let mut input = robocode_types::ToolInput::new();
                        if include_untracked {
                            input.insert("include_untracked".to_string(), "true".to_string());
                        }
                        if let Some(message) = message {
                            input.insert("message".to_string(), message);
                        }
                        if let Some(path) = paths.first() {
                            input.insert("path".to_string(), path.clone());
                        }
                        if paths.len() > 1 {
                            input.insert("paths".to_string(), paths.join("\n"));
                        }
                        self.run_named_tool("git_stash_push", input, approver)
                    }
                    "pop" => {
                        let mut input = robocode_types::ToolInput::new();
                        if let Some(stash) = args.get(2) {
                            input.insert("stash".to_string(), stash.clone());
                        }
                        self.run_named_tool("git_stash_pop", input, approver)
                    }
                    "drop" => {
                        let mut input = robocode_types::ToolInput::new();
                        if let Some(stash) = args.get(2) {
                            input.insert("stash".to_string(), stash.clone());
                        }
                        self.run_named_tool("git_stash_drop", input, approver)
                    }
                    _ => Ok(format!(
                        "Unknown git stash subcommand `{action}`.\n\n{}",
                        self.render_git_stash_help()
                    )),
                }
            }
            "worktree" => {
                let Some(action) = args.get(1).map(String::as_str) else {
                    return Ok(self.render_git_worktree_help());
                };
                match action {
                    "help" => Ok(self.render_git_worktree_help()),
                    "list" => {
                        self.run_named_tool("git_worktree_list", Default::default(), approver)
                    }
                    "add" => {
                        let path = args.get(2).cloned().ok_or_else(|| {
                            "Usage: /git worktree add <path> [branch] [--create]".to_string()
                        })?;
                        let create = args.iter().any(|arg| arg == "--create" || arg == "-b");
                        let branch = args
                            .iter()
                            .skip(3)
                            .find(|arg| arg.as_str() != "--create" && arg.as_str() != "-b")
                            .cloned()
                            .or_else(|| {
                                args.get(3)
                                    .filter(|arg| {
                                        arg.as_str() != "--create" && arg.as_str() != "-b"
                                    })
                                    .cloned()
                            });
                        let mut input = robocode_types::ToolInput::new();
                        input.insert("path".to_string(), path);
                        if let Some(branch) = branch {
                            input.insert("branch".to_string(), branch);
                        }
                        if create {
                            input.insert("create".to_string(), "true".to_string());
                        }
                        self.run_named_tool("git_worktree_add", input, approver)
                    }
                    "remove" => {
                        let path = args.get(2).cloned().ok_or_else(|| {
                            "Usage: /git worktree remove <path> [--force]".to_string()
                        })?;
                        let force = args.iter().any(|arg| arg == "--force" || arg == "-f");
                        let mut input = robocode_types::ToolInput::new();
                        input.insert("path".to_string(), path);
                        if force {
                            input.insert("force".to_string(), "true".to_string());
                        }
                        self.run_named_tool("git_worktree_remove", input, approver)
                    }
                    _ => Ok(format!(
                        "Unknown git worktree subcommand `{action}`.\n\n{}",
                        self.render_git_worktree_help()
                    )),
                }
            }
            _ => Ok(format!(
                "Unknown git subcommand `{subcommand}`.\n\n{}",
                self.render_git_help()
            )),
        }
    }
}
