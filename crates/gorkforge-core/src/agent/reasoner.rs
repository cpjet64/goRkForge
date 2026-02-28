use crate::agent::{TaskContext, TaskResult, TaskStatus};
use crate::llm::{GrokClient, LlmMessage, ToolDefinition};
use crate::tools::ToolSet;
use anyhow::{Context, Result};
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
        self.toolset.core_self_approved =
            self.toolset.core_self_approved || task_flags.contains("self_approved: yes");
        self.toolset.push_self_approved =
            self.toolset.push_self_approved || task_flags.contains("push_approved: yes");

        let requires_tooling = task_flags.contains("add")
            || task_flags.contains("create")
            || task_flags.contains("edit")
            || task_flags.contains("update")
            || task_flags.contains("append")
            || task_flags.contains("tree-sitter")
            || task_flags.contains("parse_rust_file");

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
                if requires_tooling {
                    return Ok(TaskResult {
                        status: TaskStatus::Failed,
                        output: "completion blocked: task requires tool usage".to_string(),
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
            output: match last_tool_error {
                Some(err) => format!(
                    "max iterations reached: {} (last tool error: {err})",
                    self.max_iter
                ),
                None => format!("max iterations reached: {}", self.max_iter),
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

            if let Err(err) = self.toolset.auto_commit_and_push("self-improve completed") {
                return Ok(TaskResult {
                    status: TaskStatus::Failed,
                    output: format!("auto commit/push failed after completion: {}", err),
                });
            }

            if Self::self_improve_format_ok(&result.output) {
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
