use crate::agent::{TaskContext, TaskResult, TaskStatus};
use crate::llm::{GrokClient, LlmMessage, ToolDefinition};
use crate::tools::ToolSet;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct ReActReasoner {
    pub client: GrokClient,
    pub toolset: ToolSet,
    pub max_iter: u32,
}

impl ReActReasoner {
    pub fn new(api_key: String, model: String, toolset: ToolSet, max_iter: u32) -> Self {
        Self {
            client: GrokClient::new(api_key, model),
            toolset,
            max_iter,
        }
    }

    pub async fn run_task(&mut self, context: &TaskContext) -> Result<TaskResult> {
        let task_flags = context.task.to_ascii_lowercase();
        let normalized_task = task_flags
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect::<String>();
        let referenced_issue_numbers = Self::extract_referenced_issue_numbers(&task_flags);

        self.toolset.core_self_approved =
            self.toolset.core_self_approved || normalized_task.contains("self_approved:yes");
        self.toolset.push_self_approved =
            self.toolset.push_self_approved || normalized_task.contains("push_approved:yes");

        let requires_tooling = task_flags.contains("add")
            || task_flags.contains("create")
            || task_flags.contains("edit")
            || task_flags.contains("update")
            || task_flags.contains("append")
            || task_flags.contains("tree-sitter")
            || task_flags.contains("parse_rust_file");
        let requires_mutation = task_flags.contains("add")
            || task_flags.contains("create")
            || task_flags.contains("edit")
            || task_flags.contains("update")
            || task_flags.contains("append")
            || (task_flags.contains("tree-sitter")
                && task_flags.contains("file")
                && task_flags.contains("add"));
        let requires_issue_review = !referenced_issue_numbers.is_empty()
            || task_flags.contains("review issue")
            || task_flags.contains("review issues")
            || task_flags.contains("triage issue")
            || task_flags.contains("issue triage")
            || task_flags.contains("github issue")
            || task_flags.contains("github issues")
            || task_flags.contains("read_github_issue")
            || task_flags.contains("list_github_issues")
            || task_flags.contains("pull request")
            || task_flags.contains("open_pull_request")
            || !referenced_issue_numbers.is_empty();
        let requires_issue_listing = task_flags.contains("list issue")
            || task_flags.contains("review issue")
            || task_flags.contains("review issues")
            || task_flags.contains("triage issue")
            || task_flags.contains("issue triage");
        let requires_linked_pr =
            task_flags.contains("pull request") || task_flags.contains("open_pull_request");
        let mut used_mutating_tool = false;
        let mut seen_tool_use = false;
        let mut tooling_reminder_emitted = false;
        let mut list_issues_called = false;
        let mut read_issue_called = false;
        let mut read_issue_numbers = HashSet::new();
        let mut linked_pr_called = false;
        let mut linked_issue_numbers: HashSet<u64> = HashSet::new();
        let tool_specs = self
            .toolset
            .tool_specs()
            .into_iter()
            .map(|(name, description, parameters)| ToolDefinition {
                name,
                description,
                parameters,
            })
            .collect::<Vec<_>>();
        let system_prompt = self.system_prompt(context);
        let mut messages = vec![LlmMessage {
            role: "user".to_string(),
            content: context.task.clone(),
            tool_call_id: None,
        }];
        let mut consecutive_tool_failures = 0u32;
        let mut last_tool_error = None::<String>;

        for iter in 0..self.max_iter {
            let turn = self
                .client
                .complete(&system_prompt, &messages, &tool_specs)
                .await
                .context("llm request")?;

            let has_content = turn.content.as_ref().is_some_and(|c| !c.trim().is_empty());
            if turn.tool_calls.is_empty() {
                if requires_tooling && !seen_tool_use {
                    if !tooling_reminder_emitted {
                        messages.push(LlmMessage {
                            role: "assistant".to_string(),
                            content:
                                "TASK REQUIRES TOOL USE: you must emit at least one valid tool call before completion text."
                                    .to_string(),
                            tool_call_id: None,
                        });
                        tooling_reminder_emitted = true;
                        continue;
                    }

                    return Ok(TaskResult {
                        status: TaskStatus::Failed,
                        output: "completion blocked: task requires tool usage".to_string(),
                    });
                }

                if requires_issue_review {
                    if !referenced_issue_numbers.is_empty() {
                        for issue in &referenced_issue_numbers {
                            if !read_issue_numbers.contains(issue) {
                                return Ok(TaskResult {
                                    status: TaskStatus::Failed,
                                    output: format!(
                                        "completion blocked: referenced issue #{} was not loaded with read_github_issue",
                                        issue
                                    ),
                                });
                            }
                        }
                    }

                    if requires_issue_listing && !list_issues_called {
                        return Ok(TaskResult {
                            status: TaskStatus::Failed,
                            output:
                                "completion blocked: issue workflow requires list_github_issues before implementation PR work"
                                    .to_string(),
                        });
                    }

                    if !read_issue_called {
                        return Ok(TaskResult {
                            status: TaskStatus::Failed,
                            output:
                                "completion blocked: issue workflow requires read_github_issue before completion"
                                    .to_string(),
                        });
                    }

                    if requires_linked_pr && !linked_pr_called {
                        return Ok(TaskResult {
                            status: TaskStatus::Failed,
                            output:
                                "completion blocked: issue workflow requires open_pull_request with issue_numbers"
                                    .to_string(),
                        });
                    }

                    if linked_pr_called
                        && !referenced_issue_numbers.is_empty()
                        && linked_issue_numbers.is_empty()
                    {
                        return Ok(TaskResult {
                            status: TaskStatus::Failed,
                            output:
                                "completion blocked: issue workflow requires open_pull_request to include issue_numbers"
                                    .to_string(),
                        });
                    }

                    if requires_linked_pr && !referenced_issue_numbers.is_empty() {
                        for issue in &referenced_issue_numbers {
                            if !linked_issue_numbers.contains(issue) {
                                return Ok(TaskResult {
                                    status: TaskStatus::Failed,
                                    output: format!(
                                        "completion blocked: referenced issue #{} missing from open_pull_request.issue_numbers",
                                        issue
                                    ),
                                });
                            }
                        }
                    }
                }

                if requires_mutation && !used_mutating_tool {
                    return Ok(TaskResult {
                        status: TaskStatus::Failed,
                        output: "completion blocked: task requires mutating tool usage (edit_file or write_file)"
                            .to_string(),
                    });
                }

                if has_content {
                    return Ok(TaskResult {
                        status: TaskStatus::Completed,
                        output: turn
                            .content
                            .unwrap_or_else(|| "agent returned empty completion".to_string()),
                    });
                }

                if let Some(err) = last_tool_error {
                    return Ok(TaskResult {
                        status: TaskStatus::Failed,
                        output: format!(
                            "assistant returned no tool calls after prior tool error: {}",
                            err
                        ),
                    });
                }

                return Err(anyhow::anyhow!(
                    "assistant returned no tool calls and no terminal content"
                ));
            }

            if turn.tool_calls.len() > 12 {
                return Ok(TaskResult {
                    status: TaskStatus::Failed,
                    output: format!(
                        "tool burst blocked: {} tool calls in one turn",
                        turn.tool_calls.len()
                    ),
                });
            }

            if has_content {
                messages.push(LlmMessage {
                    role: "assistant".to_string(),
                    content: turn.content.clone().unwrap_or_default(),
                    tool_call_id: None,
                });
            }

            for call in turn.tool_calls {
                if matches!(call.name.as_str(), "edit_file" | "write_file") {
                    used_mutating_tool = true;
                }
                match call.name.as_str() {
                    "list_github_issues" => {
                        list_issues_called = true;
                    }
                    "read_github_issue" => {
                        read_issue_called = true;
                        if let Some(number) = call.arguments.get("number").and_then(|v| v.as_u64())
                        {
                            read_issue_numbers.insert(number);
                        }
                    }
                    "open_pull_request" => {
                        linked_pr_called = true;
                        let mut inserted = false;
                        if let Some(list) = call
                            .arguments
                            .get("issue_numbers")
                            .and_then(|v| v.as_array())
                        {
                            for n in list {
                                if let Some(issue) = n.as_u64() {
                                    linked_issue_numbers.insert(issue);
                                    inserted = true;
                                }
                            }
                        }
                        if !inserted {
                            for issue in &read_issue_numbers {
                                linked_issue_numbers.insert(*issue);
                            }
                        }
                    }
                    _ => {}
                }
                seen_tool_use = true;
                let result = self
                    .toolset
                    .execute(&call.name, &call.arguments, context)
                    .await;
                match result {
                    Ok(v) => {
                        consecutive_tool_failures = 0;
                        last_tool_error = None;
                        messages.push(LlmMessage {
                            role: "tool".to_string(),
                            content: format!("{}:{}", call.name, v),
                            tool_call_id: Some(call.id),
                        });
                    }
                    Err(err) => {
                        consecutive_tool_failures += 1;
                        let msg = format!("{}: {}", call.name, err);
                        last_tool_error = Some(msg.clone());
                        messages.push(LlmMessage {
                            role: "tool".to_string(),
                            content: format!("error: {msg}"),
                            tool_call_id: Some(call.id),
                        });
                    }
                }
            }

            if consecutive_tool_failures >= 2 {
                return Ok(TaskResult {
                    status: TaskStatus::Failed,
                    output: format!(
                        "aborting after {consecutive_tool_failures} consecutive tool failures: {}",
                        last_tool_error.unwrap_or_else(|| "unknown".to_string())
                    ),
                });
            }

            tracing::info!(
                iteration = iter,
                consecutive_tool_failures,
                "ReAct step complete"
            );
        }

        Ok(TaskResult {
            status: TaskStatus::Failed,
            output: {
                if requires_issue_review {
                    if let Some(err) = &last_tool_error {
                        format!(
                            "max iterations reached: issue workflow could not complete required checks ({err})"
                        )
                    } else if requires_mutation && !used_mutating_tool {
                        "max iterations reached: issue workflow required mutating tool usage but none was used"
                            .to_string()
                    } else {
                        "max iterations reached: issue workflow could not complete required checks"
                            .to_string()
                    }
                } else if let Some(err) = &last_tool_error {
                    format!(
                        "max iterations reached: {} (last tool error: {err})",
                        self.max_iter
                    )
                } else {
                    format!("max iterations reached: {}", self.max_iter)
                }
            },
        })
    }

    pub async fn self_improve(&mut self) -> Result<TaskResult> {
        let special = std::fs::read_to_string("special.md")
            .unwrap_or_else(|_| "special.md missing".to_string());
        let base = String::from(
            "Run a self-improve cycle. Return explicit PLAN and PATCH. Use only tool calls.\n\n",
        );

        let mut last_output = String::new();
        for attempt in 1..=2 {
            let mut task = base.clone();
            task.push_str(&special);
            task.push_str("\n\nDo not modify .env or secrets.\n");
            task.push_str("FORMAT RULES:\nPLAN:\n...\nPATCH:\n...\nNo extra prose before PLAN.\n");
            if attempt > 1 {
                task.push_str(&format!(
                    "\n\nPrevious attempt failed format validation:\n{}\n\nRetry with exact PLAN/PATCH sections only.\n",
                    last_output
                ));
            }

            let context = TaskContext {
                task,
                spec_file: Some("special.md".to_string()),
                policy_file: Some(".gorkforge/policy.toml".to_string()),
                max_iter: Some(self.max_iter),
            };

            let result = self.run_task(&context).await?;
            if !matches!(result.status, TaskStatus::Completed) {
                return Ok(result);
            }

            if Self::self_improve_format_ok(&result.output) {
                if let Err(err) = self.toolset.auto_commit_and_push("self-improve completed") {
                    return Ok(TaskResult {
                        status: TaskStatus::Failed,
                        output: format!("auto commit/push failed after completion: {}", err),
                    });
                }

                return Ok(result);
            }

            last_output = result.output;
        }

        Ok(TaskResult {
            status: TaskStatus::Failed,
            output: format!(
                "self-improve output missing explicit PLAN/PATCH after 2 attempts\n{}",
                last_output
            ),
        })
    }

    fn system_prompt(&self, context: &TaskContext) -> String {
        let mut prompt = String::from(
            "You are goRkForge Phase 1: a local ReAct reasoner for safe code edits.\n",
        );
        prompt.push_str("Work only inside the repository.\n");
        prompt.push_str("Use tools in Plan/Execute/Verify/Reflect loops.\n");
        prompt.push_str(
            "Return PLAN and PATCH text for requested edits, but do not claim work is done without tool calls.\n",
        );
        prompt.push_str(
            "For issue-driven work, use `list_github_issues` and `read_github_issue` to select a concrete issue, then implement and open PR with `open_pull_request` including `issue_numbers` so it links automatically.\n",
        );
        prompt.push_str(
            "Issue-driven tasks are validated as: list_github_issues (when listed) -> read_github_issue -> open_pull_request with issue_numbers for implementation.\n",
        );

        if let Some(policy_file) = &context.policy_file {
            prompt.push_str(&format!("Policy file: {}\n", policy_file));
        }

        if let Some(task_iter) = context.max_iter {
            prompt.push_str(&format!("Max iterations for this run: {}\n", task_iter));
        }

        let full_context = std::env::var("GORKFORGE_CONTEXT_FULL").is_ok_and(|v| v == "YES");
        if let Ok(extra) = collect_code_context(".", full_context) {
            prompt.push_str("\nCode context:\n");
            prompt.push_str(&extra);
        }

        prompt
    }

    fn self_improve_format_ok(output: &str) -> bool {
        let out = output.to_ascii_lowercase();
        out.contains("plan:") && out.contains("patch:")
    }

    fn extract_referenced_issue_numbers(task: &str) -> Vec<u64> {
        let bytes = task.as_bytes();
        let mut values = Vec::new();
        let mut i = 0usize;

        while i < bytes.len() {
            if bytes[i] == b'#' {
                let mut j = i + 1;
                let mut value = 0u64;
                let mut found_digit = false;

                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    found_digit = true;
                    value = value
                        .saturating_mul(10)
                        .saturating_add((bytes[j] - b'0') as u64);
                    j += 1;
                }

                if found_digit {
                    values.push(value);
                }
            }
            i += 1;
        }

        values.sort_unstable();
        values.dedup();
        values
    }
}

fn collect_code_context(root: &str, include_full: bool) -> Result<String> {
    let mut snapshot_paths = vec![
        "special.md",
        "Cargo.toml",
        "gorkforge.config.toml",
        "crates/gorkforge-core/src/agent/reasoner.rs",
        "crates/gorkforge-core/src/tools/mod.rs",
        "crates/gorkforge-core/src/config.rs",
        "crates/gorkforge-core/src/llm/grok.rs",
    ];

    if include_full {
        snapshot_paths.extend([
            ".gorkforge/policy.toml",
            "crates/gorkforge-core/Cargo.toml",
            "crates/gorkforge-core/src/llm/grok.rs",
            "crates/gorkforge-core/src/agent/mod.rs",
            "crates/gorkforge-core/src/agent/reasoner.rs",
            "crates/gorkforge-core/src/platform.rs",
            "crates/gorkforge-core/src/sandbox.rs",
            "crates/gorkforge-core/src/tools/mod.rs",
        ]);
    }

    let mut out = String::new();
    let root = Path::new(root);

    for rel in snapshot_paths {
        let path = root.join(rel);
        if let Ok(content) = fs::read_to_string(&path) {
            out.push_str(&format!("\n### {}\n{}\n", path.display(), content));
        }
    }

    if out.is_empty() {
        Ok("<context-unavailable>".to_string())
    } else {
        Ok(out)
    }
}
