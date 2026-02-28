use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tree_sitter::Parser;
use tree_sitter_rust::LANGUAGE;

use crate::agent::TaskContext;
use crate::sandbox::{run_command, Sandbox};

fn default_permission_state() -> String {
    "allow".to_string()
}

fn default_max_iterations() -> Option<u32> {
    Some(20)
}

fn default_max_tokens_per_run() -> Option<u64> {
    Some(500_000)
}

fn default_permissions() -> PermissionBlock {
    PermissionBlock {
        file_read: default_permission_state(),
        file_write: default_permission_state(),
        shell_safe: default_permission_state(),
    }
}

fn default_agent_policy() -> AgentPolicy {
    AgentPolicy {
        max_iterations: default_max_iterations(),
        max_tokens_per_run: default_max_tokens_per_run(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionBlock {
    #[serde(default = "default_permission_state")]
    pub file_read: String,
    #[serde(default = "default_permission_state")]
    pub file_write: String,
    #[serde(default = "default_permission_state")]
    pub shell_safe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPolicy {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: Option<u32>,
    #[serde(default = "default_max_tokens_per_run")]
    pub max_tokens_per_run: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default = "default_agent_policy")]
    pub agent: AgentPolicy,
    #[serde(default = "default_permissions")]
    pub permissions: PermissionBlock,
}

impl PolicyConfig {
    fn default_config() -> Self {
        Self {
            agent: default_agent_policy(),
            permissions: default_permissions(),
        }
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default_config());
        }
        let txt = std::fs::read_to_string(path).context("read policy")?;
        let cfg: Self = toml::from_str(&txt).context("parse policy toml")?;
        Ok(cfg)
    }

    pub fn max_iterations(&self, fallback: u32) -> u32 {
        self.agent.max_iterations.unwrap_or(fallback)
    }

    pub fn read_allowed(&self) -> bool {
        matches!(self.permissions.file_read.as_str(), "allow" | "learn_once")
    }

    pub fn write_allowed(&self) -> bool {
        matches!(self.permissions.file_write.as_str(), "allow" | "learn_once")
    }

    pub fn shell_allowed(&self) -> bool {
        self.permissions.shell_safe != "deny"
    }
}

#[derive(Clone, Copy)]
enum GatesMode {
    Fast,
    Full,
}

impl GatesMode {
    fn from_env(default_fast: bool) -> Self {
        match std::env::var("GORKFORGE_GATES_MODE")
            .ok()
            .map(|v| v.to_ascii_lowercase())
            .as_deref()
        {
            Some("full") | Some("strict") => Self::Full,
            Some("fast") => Self::Fast,
            _ if default_fast => Self::Fast,
            _ => Self::Full,
        }
    }

    fn is_full(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Clone)]
pub struct ToolSet {
    pub sandbox: Sandbox,
    pub policy: PolicyConfig,
    pub core_self_approved: bool,
    pub push_self_approved: bool,
}

impl ToolSet {
    pub fn new(sandbox: Sandbox, policy: PolicyConfig) -> Self {
        Self {
            sandbox,
            policy,
            core_self_approved: std::env::var("SELF_APPROVED").is_ok_and(|v| v == "YES"),
            push_self_approved: std::env::var("PUSH_APPROVED").is_ok_and(|v| v == "YES"),
        }
    }

    pub fn tool_specs(&self) -> Vec<(String, String, Value)> {
        vec![
            (
                "parse_rust_file".to_string(),
                "Parse a Rust file and summarize functions and structs by name. Args: {path}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties":{"path":{"type":"string"}},
                    "required":["path"],
                    "additionalProperties":false
                }),
            ),
            (
                "read_file".to_string(),
                "Read file content from overlay workspace by path. Args: {path}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties": {"path":{"type":"string"}},
                    "required":["path"],
                    "additionalProperties":false
                }),
            ),
            (
                "edit_file".to_string(),
                "Search-replace edit. Args: {path, find, replace, all}. all optional false default.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties": {
                        "path":{"type":"string"},
                        "find":{"type":"string"},
                        "replace":{"type":"string"},
                        "all":{"type":"boolean"}
                    },
                    "required":["path","find","replace"],
                    "additionalProperties":false
                }),
            ),
            (
                "write_file".to_string(),
                "Create or overwrite file content in overlay. Args: {path, content}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties": {
                        "path":{"type":"string"},
                        "content":{"type":"string"}
                    },
                    "required":["path","content"],
                    "additionalProperties":false
                }),
            ),
            (
                "run_cargo".to_string(),
                "Run cargo gates in overlay dir. Fast mode: fmt --check + check. Full mode: clippy -- -D warnings, test, build. Controlled by GORKFORGE_GATES_MODE.".to_string(),
                serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            ),
            (
                "git_status".to_string(),
                "Show git status on workspace root.".to_string(),
                serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            ),
            (
                "git_commit".to_string(),
                "Commit overlay changes with normalized message. Args: {message}.".to_string(),
                serde_json::json!({"type":"object","properties":{"message":{"type":"string"}},"additionalProperties":false}),
            ),
            (
                "git_push".to_string(),
                "Push current branch to origin. Requires PUSH_APPROVED=YES and safe branch.".to_string(),
                serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            ),
            (
                "open_pull_request".to_string(),
                "Create a GitHub pull request for the current branch using gh. Requires PUSH_APPROVED=YES. If this fails, fall back to shell_safe with manual gh command if needed. Args: {title, body?, base?, head?, draft?, labels?, issue_numbers?}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties":{
                        "title":{"type":"string"},
                        "body":{"type":"string"},
                        "base":{"type":"string"},
                        "head":{"type":"string"},
                        "draft":{"type":"boolean"},
                        "labels":{"type":"array","items":{"type":"string"}},
                        "issue_numbers":{"type":"array","items":{"type":"integer"}}
                    },
                    "required":["title"],
                    "additionalProperties":false
                }),
            ),
            (
                "list_github_issues".to_string(),
                "List GitHub issues in the current repository. Args: {state?, limit?, labels?}. Args are optional.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties":{
                        "state":{"type":"string"},
                        "limit":{"type":"integer"},
                        "labels":{"type":"string"}
                    },
                    "required":[],
                    "additionalProperties":false
                }),
            ),
            (
                "read_github_issue".to_string(),
                "Read a GitHub issue by number for review. Args: {number}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties":{
                        "number":{"type":"integer"}
                    },
                    "required":["number"],
                    "additionalProperties":false
                }),
            ),
            (
                "create_issue".to_string(),
                "Create a GitHub issue for deferred/future work using gh. Requires PUSH_APPROVED=YES. Args: {title, body?, labels?, assignees?}.".to_string(),
                serde_json::json!({
                    "type":"object",
                    "properties":{
                        "title":{"type":"string"},
                        "body":{"type":"string"},
                        "labels":{"type":"array","items":{"type":"string"}},
                        "assignees":{"type":"array","items":{"type":"string"}}
                    },
                    "required":["title"],
                    "additionalProperties":false
                }),
            ),
            (
                "git_create_feature_branch".to_string(),
                "Create a new local feature branch. Args: {name}.".to_string(),
                serde_json::json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"],"additionalProperties":false}),
            ),
            (
                "list_dir".to_string(),
                "List directory entries. Args: {path}.".to_string(),
                serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"additionalProperties":false}),
            ),
            (
                "grep".to_string(),
                "Search literal pattern in source files. Args: {path, pattern}.".to_string(),
                serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"pattern":{"type":"string"}},"required":["pattern"],"additionalProperties":false}),
            ),
            (
                "shell_safe".to_string(),
                "Run safe shell command. Args: {command}.".to_string(),
                serde_json::json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false}),
            ),
        ]
    }

    pub async fn execute(&self, name: &str, args: &Value, ctx: &TaskContext) -> Result<String> {
        let result = match name {
            "parse_rust_file" => self.parse_rust_file(args),
            "read_file" => self.read_file(args),
            "edit_file" => self.edit_file(args),
            "write_file" => self.write_file(args),
            "run_cargo" => self.run_cargo(ctx),
            "git_status" => self.git_status(),
            "git_commit" => self.git_commit(
                args.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("gorkforge overlay commit"),
            ),
            "git_push" => self.git_push(),
            "open_pull_request" => self.open_pull_request(args),
            "list_github_issues" => self.list_github_issues(args),
            "read_github_issue" => self.read_github_issue(args),
            "create_issue" => self.create_issue(args),
            "git_create_feature_branch" => self.git_create_feature_branch(args),
            "list_dir" => self.list_dir(args),
            "grep" => self.grep(args),
            "shell_safe" => self.shell_safe(args),
            _ => return Err(anyhow!("unknown tool: {}", name)),
        };
        result
    }

    fn ensure_no_path_traversal(path: &str) -> Result<()> {
        if Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(anyhow!("path traversal is not allowed: {}", path));
        }
        Ok(())
    }

    fn overlay_or_workspace_path(base: &Path, rel: &str) -> Result<PathBuf> {
        Self::ensure_no_path_traversal(rel)?;
        Ok(base.join(rel))
    }

    fn ensure_overlay_copy(&self, rel: &str) -> Result<PathBuf> {
        let overlay_path = ToolSet::overlay_or_workspace_path(&self.sandbox.overlay_root, rel)?;
        if overlay_path.exists() {
            return Ok(overlay_path);
        }

        let source_path = ToolSet::overlay_or_workspace_path(&self.sandbox.workspace_root, rel)?;
        if source_path.exists() {
            if let Some(parent) = overlay_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &overlay_path).with_context(|| {
                format!(
                    "copy {} -> {}",
                    source_path.display(),
                    overlay_path.display()
                )
            })?;
        }

        Ok(overlay_path)
    }

    fn read_file(&self, args: &Value) -> Result<String> {
        if !self.policy.read_allowed() {
            return Err(anyhow!("policy denies file_read"));
        }

        let rel = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("read_file: path required"))?;
        let target = self.ensure_overlay_copy(rel)?;

        std::fs::read_to_string(&target).with_context(|| format!("read file {}", target.display()))
    }

    fn parse_rust_file(&self, args: &Value) -> Result<String> {
        if !self.policy.read_allowed() {
            return Err(anyhow!("policy denies file_read"));
        }

        let rel = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("parse_rust_file: path required"))?;

        let overlay_target = self.sandbox.overlay_root.join(rel);
        let target = if overlay_target.exists() {
            overlay_target
        } else {
            ToolSet::overlay_or_workspace_path(&self.sandbox.workspace_root, rel)?
        };
        let src = std::fs::read_to_string(&target)
            .with_context(|| format!("parse_rust_file: failed reading {}", target.display()))?;

        let mut parser = Parser::new();
        let language = LANGUAGE.into();
        if let Err(err) = parser.set_language(&language) {
            return Err(anyhow!("parse_rust_file: language setup failed: {:?}", err));
        }
        let tree = parser
            .parse(&src, None)
            .ok_or_else(|| anyhow!("parse_rust_file: parse failed"))?;

        let mut functions = Vec::new();
        let mut structs = Vec::new();
        collect_rust_decls(
            tree.root_node(),
            src.as_bytes(),
            &mut functions,
            &mut structs,
        );

        let functions = functions
            .into_iter()
            .map(|name| format!(r#""{}""#, name))
            .collect::<Vec<_>>()
            .join(", ");
        let structs = structs
            .into_iter()
            .map(|name| format!(r#""{}""#, name))
            .collect::<Vec<_>>()
            .join(", ");

        Ok(format!(
            "Functions: [{}]\nStructs: [{}]",
            functions, structs
        ))
    }

    fn edit_file(&self, args: &Value) -> Result<String> {
        if !self.policy.write_allowed() {
            return Err(anyhow!("policy denies file_write"));
        }

        let rel = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("edit_file: path required"))?;
        let find = args
            .get("find")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("edit_file: find required"))?;
        let replace = args
            .get("replace")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("edit_file: replace required"))?;
        let do_all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);

        if (rel.starts_with("crates/gorkforge-core/")
            || rel.starts_with("crates\\gorkforge-core\\"))
            && !self.core_self_approved
        {
            return Err(anyhow!("core edits require SELF_APPROVED=YES"));
        }

        let target = self.ensure_overlay_copy(rel)?;
        let existing = std::fs::read_to_string(&target)
            .with_context(|| format!("read target file {}", target.display()))?;

        let replacement = if do_all {
            existing.replace(find, replace)
        } else {
            existing.replacen(find, replace, 1)
        };

        if replacement == existing {
            return Ok(format!("no change for {}", rel));
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, replacement)?;
        self.sandbox
            .log(&format!("edit_file {}", target.display()))?;
        self.run_cargo_checks("edit_file", true, GatesMode::from_env(true))?;
        self.auto_commit_and_push("edit_file")?;

        Ok(format!("edited {}", rel))
    }

    fn write_file(&self, args: &Value) -> Result<String> {
        if !self.policy.write_allowed() {
            return Err(anyhow!("policy denies file_write"));
        }

        let rel = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("write_file: path required"))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("write_file: content required"))?;
        Self::ensure_no_path_traversal(rel)?;

        if (rel.starts_with("crates/gorkforge-core/")
            || rel.starts_with("crates\\gorkforge-core\\"))
            && !self.core_self_approved
        {
            return Err(anyhow!("core edits require SELF_APPROVED=YES"));
        }

        let target = self.sandbox.overlay_root.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&target, content)?;
        self.sandbox
            .log(&format!("write_file {}", target.display()))?;
        self.run_cargo_checks("write_file", true, GatesMode::from_env(true))?;
        self.auto_commit_and_push("write_file")?;
        Ok(format!("wrote {}", rel))
    }

    fn list_dir(&self, args: &Value) -> Result<String> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let target = ToolSet::overlay_or_workspace_path(&self.sandbox.workspace_root, path)?;
        let mut lines = Vec::new();

        for ent in std::fs::read_dir(target)? {
            let e = ent?;
            lines.push(e.file_name().to_string_lossy().to_string());
        }

        lines.sort_unstable();
        Ok(lines.join("\n"))
    }

    fn grep(&self, args: &Value) -> Result<String> {
        if !self.policy.read_allowed() {
            return Err(anyhow!("policy denies read for grep"));
        }

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("grep: pattern required"))?;
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let target = ToolSet::overlay_or_workspace_path(&self.sandbox.workspace_root, path)?;

        let mut output = Vec::new();
        gather_files_for_scan(&target, &mut |entry: &PathBuf| {
            let rel = entry
                .strip_prefix(&self.sandbox.workspace_root)
                .unwrap_or(entry);
            let source = ToolSet::overlay_or_workspace_path(
                &self.sandbox.overlay_root,
                rel.to_string_lossy().as_ref(),
            )
            .ok();
            let txt = match source.and_then(|p| std::fs::read_to_string(p).ok()) {
                Some(v) => v,
                None => match std::fs::read_to_string(entry) {
                    Ok(v) => v,
                    Err(_) => return,
                },
            };
            for (line_no, line) in txt.lines().enumerate() {
                if line.contains(pattern) {
                    output.push(format!(
                        "{}:{}:{}",
                        entry.display(),
                        line_no + 1,
                        line.trim()
                    ));
                }
            }
        })?;

        Ok(output.join("\n"))
    }

    fn shell_safe(&self, args: &Value) -> Result<String> {
        if !self.policy.shell_allowed() {
            return Err(anyhow!("policy denies shell_safe"));
        }

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("shell_safe: command required"))?;
        if command.contains('|') || command.contains('&') || command.contains('$') {
            return Err(anyhow!("unsafe shell syntax blocked"));
        }

        self.sandbox.log(&format!("shell_safe: {}", command))?;
        run_command_allowlist(&self.sandbox.overlay_root, command)
    }

    fn run_cargo(&self, ctx: &TaskContext) -> Result<String> {
        let context_task = ctx.task.to_ascii_lowercase();
        let mode = GatesMode::from_env(
            context_task.contains("self-improve")
                || context_task.contains("self improve")
                || context_task.contains("self_improve"),
        );
        self.run_cargo_checks("run_cargo", false, mode)
    }

    fn run_cargo_checks(
        &self,
        reason: &str,
        auto_fix_format: bool,
        gates_mode: GatesMode,
    ) -> Result<String> {
        if auto_fix_format {
            if let Err(err) = self.run_cargo_step(&["fmt", "--check"]) {
                self.sandbox.log(&format!(
                    "cargo fmt check failed during {}: {}",
                    reason, err
                ))?;
                self.run_cargo_step(&["fmt"])?;
            }
        } else {
            self.run_cargo_step(&["fmt", "--check"])?;
        }

        if !gates_mode.is_full() {
            self.run_cargo_step(&["check"])?;
            return Ok(format!("run_cargo ok: {}", reason));
        }

        self.run_cargo_step(&["clippy", "--", "-D", "warnings"])?;
        self.run_cargo_step(&["test"])?;
        self.run_cargo_step(&["build"])?;

        Ok(format!("run_cargo ok: {}", reason))
    }

    fn run_cargo_step(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("cargo")
            .args(args)
            .current_dir(&self.sandbox.overlay_root)
            .output()
            .with_context(|| format!("run cargo {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("cargo {} failed\n{}", args.join(" "), stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn git_status(&self) -> Result<String> {
        run_command(&self.sandbox.workspace_root, "git", &["status", "--short"])
    }

    fn has_workspace_changes(&self) -> Result<bool> {
        let status = self.git_status()?;
        Ok(!status.trim().is_empty())
    }

    pub(crate) fn auto_commit_and_push(&self, reason: &str) -> Result<String> {
        if !self.has_workspace_changes()? {
            return Ok("no workspace changes to commit".to_string());
        }

        let commit_msg = format!("gorkforge: {}", reason);
        let mut output = self.git_commit(&commit_msg)?;

        if self.push_self_approved {
            output.push('\n');
            output.push_str(&self.git_push()?);
        }

        Ok(output)
    }

    fn git_commit(&self, message: &str) -> Result<String> {
        let message = if message.starts_with("gorkforge: ") {
            message.to_string()
        } else {
            format!("gorkforge: {}", message)
        };
        self.sandbox.commit(&message)
    }

    fn git_create_feature_branch(&self, args: &Value) -> Result<String> {
        let raw = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("git_create_feature_branch: name required"))?
            .trim();

        if raw.is_empty() {
            return Err(anyhow!(
                "git_create_feature_branch: branch name cannot be empty"
            ));
        }
        if raw.contains(' ') || raw.contains('\n') || raw.contains('\t') || raw.contains('\\') {
            return Err(anyhow!(
                "git_create_feature_branch: invalid branch name '{}'",
                raw
            ));
        }
        if raw.contains("..") {
            return Err(anyhow!(
                "git_create_feature_branch: suspicious branch path '{}'",
                raw
            ));
        }

        let branch = if raw.starts_with("feature/") {
            raw.to_string()
        } else {
            format!("feature/{}", raw)
        };
        let protected = ["main", "master", "develop", "trunk"];
        if protected.iter().any(|b| *b == branch) {
            return Err(anyhow!(
                "git_create_feature_branch: branch '{}' is protected",
                branch
            ));
        }

        run_command(
            &self.sandbox.workspace_root,
            "git",
            &["checkout", "-b", &branch],
        )?;
        self.sandbox
            .log(&format!("git_create_feature_branch: {}", branch))?;
        Ok(format!("created feature branch '{}'", branch))
    }

    fn open_pull_request(&self, args: &Value) -> Result<String> {
        if !self.push_self_approved {
            return Err(anyhow!(
                "open_pull_request blocked: PUSH_APPROVED=YES required"
            ));
        }

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("open_pull_request: title required"))?
            .trim()
            .to_string();
        if title.is_empty() {
            return Err(anyhow!("open_pull_request: title cannot be empty"));
        }

        let body = args
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let base = args
            .get("base")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .trim()
            .to_string();

        let head = if let Some(raw) = args.get("head").and_then(|v| v.as_str()) {
            raw.trim().to_string()
        } else {
            run_command(
                &self.sandbox.workspace_root,
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
            )?
            .trim()
            .to_string()
        };

        let draft = args.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);

        let issue_numbers = args
            .get("issue_numbers")
            .and_then(|v| v.as_array())
            .map(|vals| {
                vals.iter()
                    .filter_map(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let labels = args
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|vals| {
                vals.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if head.is_empty() {
            return Err(anyhow!("open_pull_request: head branch cannot be empty"));
        }
        if head == "main" || head == "master" || head == "develop" || head == "trunk" {
            return Err(anyhow!(
                "open_pull_request: branch '{}' cannot be used as PR source",
                head
            ));
        }
        if base.is_empty() {
            return Err(anyhow!("open_pull_request: base branch cannot be empty"));
        }
        if head.contains('\\') || base.contains('\\') {
            return Err(anyhow!(
                "open_pull_request: invalid branch separator in '{}' or '{}'",
                head,
                base
            ));
        }
        if head.contains("..") || base.contains("..") {
            return Err(anyhow!(
                "open_pull_request: suspicious branch reference in '{}' or '{}'",
                head,
                base
            ));
        }
        if head.contains('\n') || head.contains('\t') || base.contains('\n') || base.contains('\t')
        {
            return Err(anyhow!(
                "open_pull_request: invalid branch/newline in '{}' or '{}'",
                head,
                base
            ));
        }
        if head.contains(' ') || base.contains(' ') {
            return Err(anyhow!(
                "open_pull_request: branch names cannot contain spaces: '{}' / '{}'",
                head,
                base
            ));
        }

        let mut cli_args = vec![
            "pr".to_string(),
            "create".to_string(),
            "--title".to_string(),
            title,
        ];
        if !base.is_empty() {
            cli_args.push("--base".to_string());
            cli_args.push(base);
        }
        if !head.is_empty() {
            cli_args.push("--head".to_string());
            cli_args.push(head);
        }
        if draft {
            cli_args.push("--draft".to_string());
        }
        for label in labels {
            cli_args.push("--label".to_string());
            cli_args.push(label);
        }
        if !body.is_empty() {
            let mut full_body = body;
            if !issue_numbers.is_empty() {
                let closes = issue_numbers
                    .iter()
                    .map(|n| format!("Closes #{}", n))
                    .collect::<Vec<_>>()
                    .join("\n");
                full_body = format!("{}\n\n{}", full_body, closes);
            }
            cli_args.push("--body".to_string());
            cli_args.push(full_body);
        } else if !issue_numbers.is_empty() {
            let closes = issue_numbers
                .iter()
                .map(|n| format!("Closes #{}", n))
                .collect::<Vec<_>>()
                .join("\n");
            cli_args.push("--body".to_string());
            cli_args.push(closes);
        }

        let cli_refs: Vec<&str> = cli_args.iter().map(String::as_str).collect();
        run_command(&self.sandbox.workspace_root, "gh", &cli_refs)
    }

    fn list_github_issues(&self, args: &Value) -> Result<String> {
        let state = args
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("open")
            .trim()
            .to_string();
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
        let labels = args
            .get("labels")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let mut cli_args = vec![
            "issue".to_string(),
            "list".to_string(),
            "--json".to_string(),
            "number,title,url,state,labels,assignees".to_string(),
        ];
        if !state.is_empty() {
            cli_args.push("--state".to_string());
            cli_args.push(state);
        }
        if !labels.is_empty() {
            cli_args.push("--label".to_string());
            cli_args.push(labels);
        }
        cli_args.push("--limit".to_string());
        cli_args.push(format!("{}", limit));

        let cli_refs: Vec<&str> = cli_args.iter().map(String::as_str).collect();
        run_command(&self.sandbox.workspace_root, "gh", &cli_refs)
    }

    fn read_github_issue(&self, args: &Value) -> Result<String> {
        let number = args
            .get("number")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("read_github_issue: number required"))?;
        if number == 0 {
            return Err(anyhow!("read_github_issue: number must be positive"));
        }

        let number = number.to_string();
        let cli_args = [
            "issue",
            "view",
            number.as_str(),
            "--json",
            "number,title,body,state,labels,assignees,url",
        ];
        run_command(&self.sandbox.workspace_root, "gh", &cli_args)
    }

    fn create_issue(&self, args: &Value) -> Result<String> {
        if !self.push_self_approved {
            return Err(anyhow!("create_issue blocked: PUSH_APPROVED=YES required"));
        }

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("create_issue: title required"))?
            .trim()
            .to_string();
        if title.is_empty() {
            return Err(anyhow!("create_issue: title cannot be empty"));
        }

        let body = args
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let labels = args
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|vals| {
                vals.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let assignees = args
            .get("assignees")
            .and_then(|v| v.as_array())
            .map(|vals| {
                vals.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut cli_args = vec![
            "issue".to_string(),
            "create".to_string(),
            "--title".to_string(),
            title,
        ];
        if !body.is_empty() {
            cli_args.push("--body".to_string());
            cli_args.push(body);
        }
        for label in labels {
            cli_args.push("--label".to_string());
            cli_args.push(label);
        }
        for assignee in assignees {
            cli_args.push("--assignee".to_string());
            cli_args.push(assignee);
        }

        let cli_refs: Vec<&str> = cli_args.iter().map(String::as_str).collect();
        run_command(&self.sandbox.workspace_root, "gh", &cli_refs)
    }

    fn git_push(&self) -> Result<String> {
        if !self.push_self_approved {
            return Err(anyhow!("push blocked: PUSH_APPROVED=YES required"));
        }

        let branch = run_command(
            &self.sandbox.workspace_root,
            "git",
            &["rev-parse", "--abbrev-ref", "HEAD"],
        )?;
        let branch = branch.trim();
        let allow_main_push = std::env::var("GORKFORGE_ALLOW_MAIN_PUSH")
            .ok()
            .is_some_and(|v| v == "YES");

        if (branch == "main" || branch == "master" || branch == "develop" || branch == "trunk")
            && !allow_main_push
        {
            return Err(anyhow!("push blocked: branch '{}' is protected", branch));
        }
        if branch.is_empty() || branch == "HEAD" || branch.contains("..") {
            return Err(anyhow!("push blocked: invalid branch '{}'", branch));
        }

        self.sandbox.push("origin", branch)
    }
}

fn gather_files_for_scan<F: FnMut(&PathBuf)>(dir: &Path, mut f: F) -> Result<()> {
    let mut skipped = HashSet::new();
    skipped.insert(dir.join(".git"));
    skipped.insert(dir.join("target"));
    skipped.insert(dir.join(".gorkforge"));
    gather_files_internal(dir, &skipped, &mut f)
}

fn collect_rust_decls(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    functions: &mut Vec<String>,
    structs: &mut Vec<String>,
) {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        match current.kind() {
            "function_item" => {
                if let Some(name_node) = current.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        functions.push(name.to_string());
                    }
                }
            }
            "struct_item" => {
                if let Some(name_node) = current.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        structs.push(name.to_string());
                    }
                }
            }
            _ => {}
        }

        let mut i = 0u32;
        let child_count = current.named_child_count() as u32;
        while i < child_count {
            if let Some(child) = current.named_child(i) {
                stack.push(child);
            }
            i += 1;
        }
    }
}

fn gather_files_internal<F: FnMut(&PathBuf)>(
    cur: &Path,
    skipped: &HashSet<PathBuf>,
    f: &mut F,
) -> Result<()> {
    if skipped.contains(cur) {
        return Ok(());
    }

    for entry in std::fs::read_dir(cur)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "target" || name == "runs" {
            continue;
        }

        let path = entry.path();
        if path.is_dir() {
            gather_files_internal(&path, skipped, f)?;
            continue;
        }

        f(&path);
    }

    Ok(())
}

fn run_command_allowlist(cwd: &Path, command: &str) -> Result<String> {
    let mut parts = command.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(anyhow!("shell_safe: empty command"));
    }

    let allowed = [
        "cargo", "git", "ls", "dir", "echo", "pwd", "rg", "findstr", "python", "python3", "node",
        "gh",
    ];
    let program = parts.remove(0);
    if !allowed.contains(&program) {
        return Err(anyhow!("shell_safe disallows program {}", program));
    }
    if program == "git" && parts.first().is_some_and(|v| *v == "merge") {
        return Err(anyhow!("shell_safe: blocked for branch merge safety"));
    }

    let output = Command::new(program)
        .args(parts)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("run shell_safe {}", command))?;

    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        return Err(anyhow!("shell_safe failed: {}", command));
    }

    Ok(text)
}
