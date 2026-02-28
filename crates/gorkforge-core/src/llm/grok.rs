use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde_json::{self, Value};
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LlmTurn {
    pub content: Option<String>,
    pub tool_calls: Vec<GrokTool>,
}

#[derive(Clone, Debug)]
pub struct GrokTool {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub struct GrokClient {
    api_key: String,
    client: Client,
    model: String,
}

impl GrokClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            model,
        }
    }

    pub async fn complete(
        &self,
        system_prompt: &str,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
    ) -> Result<LlmTurn> {
        let mut payload_messages: Vec<RequestMessage> = Vec::with_capacity(messages.len() + 1);
        payload_messages.push(RequestMessage::System {
            content: system_prompt.to_string(),
        });

        for msg in messages {
            match msg.role.as_str() {
                "user" => payload_messages.push(RequestMessage::User {
                    content: msg.content.clone(),
                }),
                "assistant" => payload_messages.push(RequestMessage::Assistant {
                    content: Some(msg.content.clone()),
                }),
                "tool" => payload_messages.push(RequestMessage::Tool {
                    content: msg.content.clone(),
                    tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
                }),
                _ => payload_messages.push(RequestMessage::Assistant {
                    content: Some(msg.content.clone()),
                }),
            }
        }

        let request_tools = tools
            .iter()
            .map(|tool| RequestTool {
                type_: "function".to_string(),
                function: RequestFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                },
            })
            .collect();

        let request = RequestEnvelope {
            model: self.model.clone(),
            messages: payload_messages,
            tools: request_tools,
            tool_choice: "auto".to_string(),
        };

        let mut last_err = None;
        for attempt in 1..=3u8 {
            let response = self
                .client
                .post("https://api.x.ai/v1/chat/completions")
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    if !response.status().is_success() {
                        let code = response.status();
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "<unable to read body>".to_string());

                        if code.is_server_error()
                            || code == StatusCode::TOO_MANY_REQUESTS
                            || code == StatusCode::REQUEST_TIMEOUT
                        {
                            last_err = Some(anyhow!("xAI API returned {}: {}", code, body));
                            if attempt < 3 {
                                sleep(Duration::from_millis(200 * u64::from(attempt))).await;
                                continue;
                            }
                        }

                        return Err(anyhow!("xAI API returned {}: {}", code, body));
                    }

                    let payload = response
                        .json::<ResponseEnvelope>()
                        .await
                        .context("invalid /chat/completions response")?;

                    let choice = payload
                        .choices
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow!("xAI API returned no completion choice"))?;

                    let mut turn = LlmTurn {
                        content: choice.message.content,
                        tool_calls: Vec::new(),
                    };

                    if let Some(calls) = choice.message.tool_calls {
                        for call in calls {
                            let args = serde_json::from_str::<Value>(&call.function.arguments)
                                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                            turn.tool_calls.push(GrokTool {
                                id: call.id,
                                name: call.function.name,
                                arguments: args,
                            });
                        }
                    }

                    return Ok(turn);
                }
                Err(err) => {
                    last_err = Some(anyhow!(err));
                    if attempt < 3 {
                        sleep(Duration::from_millis(200 * u64::from(attempt))).await;
                        continue;
                    }
                }
            }
        }

        Err(anyhow!(
            "xAI API request failed after retries: {}",
            last_err.unwrap_or_else(|| anyhow!("unknown error"))
        ))
    }
}

#[derive(Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum RequestMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
    },
    Tool {
        content: String,
        tool_call_id: String,
    },
}

#[derive(Serialize)]
struct RequestEnvelope {
    model: String,
    messages: Vec<RequestMessage>,
    tools: Vec<RequestTool>,
    tool_choice: String,
}

#[derive(Serialize)]
struct RequestFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Serialize)]
struct RequestTool {
    #[serde(rename = "type")]
    type_: String,
    function: RequestFunction,
}

#[derive(serde::Deserialize)]
struct ResponseEnvelope {
    choices: Vec<ResponseChoice>,
}

#[derive(serde::Deserialize)]
struct ResponseChoice {
    message: ResponseMessage,
}

#[derive(serde::Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(serde::Deserialize)]
struct ResponseToolCall {
    id: String,
    function: ResponseFunction,
}

#[derive(serde::Deserialize)]
struct ResponseFunction {
    name: String,
    arguments: String,
}
