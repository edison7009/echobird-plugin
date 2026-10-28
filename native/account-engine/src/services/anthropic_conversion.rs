fn anthropic_tool_result_content(
    tool_result: &serde_json::Value,
) -> (String, Vec<serde_json::Value>) {
    let Some(content) = tool_result.get("content") else {
        return (String::new(), Vec::new());
    };
    if let Some(text) = content.as_str() {
        return (text.to_string(), Vec::new());
    }

    let mut text = String::new();
    let mut attachments = Vec::new();
    if let Some(blocks) = content.as_array() {
        for block in blocks {
            if block.get("type").and_then(|value| value.as_str()) == Some("text") {
                if let Some(part) = block.get("text").and_then(|value| value.as_str()) {
                    text.push_str(part);
                }
            } else if block.get("type").and_then(|value| value.as_str()) == Some("tool_reference") {
                if let Some(name) = block
                    .get("tool_name")
                    .and_then(|value| value.as_str())
                    .filter(|name| !name.is_empty())
                {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&format!("Tool {name} is loaded and can be called now."));
                }
            } else if let Some(part) = anthropic_user_content_part(block) {
                attachments.push(part);
            }
        }
    }
    (text, attachments)
}
pub(crate) fn anthropic_to_openai(body: &serde_json::Value) -> serde_json::Value {
    let mut messages = Vec::new();

    // System message
    if let Some(system) = body.get("system") {
        let system_text = if let Some(s) = system.as_str() {
            s.to_string()
        } else if let Some(arr) = system.as_array() {
            arr.iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        } else {
            String::new()
        };
        if !system_text.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": system_text}));
        }
    }

    // Conversation messages �� handle text, tool_use, and tool_result blocks
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");

            match msg.get("content") {
                // Simple string content
                Some(c) if c.is_string() => {
                    messages.push(
                        serde_json::json!({"role": role, "content": c.as_str().unwrap_or("")}),
                    );
                }
                // Array of content blocks
                Some(c) if c.is_array() => {
                    let blocks = c.as_array().unwrap();

                    let text_parts: Vec<&str> = blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect();

                    let tool_use_blocks: Vec<&serde_json::Value> = blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                        .collect();

                    let tool_result_blocks: Vec<&serde_json::Value> = blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
                        .collect();

                    if !tool_use_blocks.is_empty() {
                        let tool_calls: Vec<serde_json::Value> = tool_use_blocks.iter().map(|tu| {
                            let id = tu.get("id").and_then(|v| v.as_str()).unwrap_or("call_0");
                            let name = tu.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let input = tu.get("input").cloned().unwrap_or(serde_json::json!({}));
                            serde_json::json!({
                                "id": id, "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string())
                                }
                            })
                        }).collect();
                        let text_content = text_parts.join("");
                        let mut assistant_msg = serde_json::json!({
                            "role": "assistant",
                            "tool_calls": tool_calls
                        });
                        if !text_content.is_empty() {
                            assistant_msg["content"] = serde_json::json!(text_content);
                        }
                        messages.push(assistant_msg);
                    } else if !tool_result_blocks.is_empty() {
                        let mut follow_up_content = Vec::new();
                        for tr in &tool_result_blocks {
                            let tool_call_id =
                                tr.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
                            let (mut result_content, attachments) =
                                anthropic_tool_result_content(tr);
                            if result_content.is_empty() && !attachments.is_empty() {
                                result_content =
                                    "Attachments are included in the following user message."
                                        .to_string();
                            }
                            messages.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_call_id,
                                "content": result_content
                            }));
                            if !attachments.is_empty() {
                                follow_up_content.push(serde_json::json!({
                                    "type": "text",
                                    "text": format!("Attachments from tool result {tool_call_id}:")
                                }));
                                follow_up_content.extend(attachments);
                            }
                        }
                        follow_up_content.extend(
                            blocks
                                .iter()
                                .filter(|block| {
                                    !matches!(
                                        block.get("type").and_then(|value| value.as_str()),
                                        Some("tool_result" | "tool_use")
                                    )
                                })
                                .filter_map(anthropic_user_content_part),
                        );
                        if !follow_up_content.is_empty() {
                            messages.push(serde_json::json!({
                                "role": role,
                                "content": openai_user_content(follow_up_content)
                            }));
                        }
                    } else {
                        let content: Vec<serde_json::Value> = blocks
                            .iter()
                            .filter_map(anthropic_user_content_part)
                            .collect();
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": openai_user_content(content)
                        }));
                    }
                }
                _ => {
                    messages.push(serde_json::json!({"role": role, "content": ""}));
                }
            }
        }
    }

    let mut req = serde_json::json!({
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or("local-model"),
        "messages": messages,
        "max_tokens": body.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(4096),
        "stream": body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false),
    });

    if let Some(temperature) = body.get("temperature") {
        req["temperature"] = temperature.clone();
    }

    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        if !tools.is_empty() {
            let openai_tools: Vec<serde_json::Value> = tools.iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        "parameters": t.get("input_schema").cloned()
                            .unwrap_or(serde_json::json!({"type": "object", "properties": {}}))
                    }
                })
            }).collect();
            req["tools"] = serde_json::json!(openai_tools);
            if let Some(tc) = body.get("tool_choice") {
                let choice_type = tc.get("type").and_then(|v| v.as_str()).unwrap_or("auto");
                req["tool_choice"] = match choice_type {
                    "any" => serde_json::json!("required"),
                    "none" => serde_json::json!("none"),
                    "tool" => serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": tc.get("name").and_then(|v| v.as_str()).unwrap_or("")
                        }
                    }),
                    _ => serde_json::json!("auto"),
                };
                if tc
                    .get("disable_parallel_tool_use")
                    .and_then(|v| v.as_bool())
                    == Some(true)
                {
                    req["parallel_tool_calls"] = serde_json::json!(false);
                }
            }
        }
    }

    if let Some(stop) = body.get("stop_sequences") {
        req["stop"] = stop.clone();
    }
    if let Some(top_p) = body.get("top_p") {
        req["top_p"] = top_p.clone();
    }

    req
}
fn anthropic_user_content_part(block: &serde_json::Value) -> Option<serde_json::Value> {
    match block.get("type").and_then(|value| value.as_str()) {
        Some("text") => block
            .get("text")
            .and_then(|value| value.as_str())
            .map(|text| serde_json::json!({"type": "text", "text": text})),
        Some("image") => {
            let source = block.get("source")?;
            let url = match source.get("type").and_then(|value| value.as_str()) {
                Some("base64") => {
                    let media_type = source.get("media_type")?.as_str()?;
                    let data = source.get("data")?.as_str()?;
                    format!("data:{media_type};base64,{data}")
                }
                Some("url") => source.get("url")?.as_str()?.to_string(),
                _ => return None,
            };
            Some(serde_json::json!({
                "type": "image_url",
                "image_url": {"url": url}
            }))
        }
        Some("document") => {
            let source = block.get("source")?;
            match source.get("type").and_then(|value| value.as_str()) {
                Some("base64") => {
                    let data = source.get("data")?.as_str()?;
                    let filename = block
                        .get("title")
                        .and_then(|value| value.as_str())
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or("document.pdf");
                    Some(serde_json::json!({
                        "type": "file",
                        "file": {"file_data": data, "filename": filename}
                    }))
                }
                Some("text") => source
                    .get("data")
                    .and_then(|value| value.as_str())
                    .map(|text| serde_json::json!({"type": "text", "text": text})),
                Some("url") => source
                    .get("url")
                    .and_then(|value| value.as_str())
                    .map(|url| {
                        serde_json::json!({
                            "type": "text",
                            "text": format!("Document URL: {url}")
                        })
                    }),
                Some("file") => {
                    source
                        .get("file_id")
                        .and_then(|value| value.as_str())
                        .map(|file_id| {
                            serde_json::json!({
                                "type": "text",
                                "text": format!("Anthropic file reference: {file_id}")
                            })
                        })
                }
                _ => None,
            }
        }
        _ => None,
    }
}
fn openai_user_content(parts: Vec<serde_json::Value>) -> serde_json::Value {
    let text_parts: Option<Vec<&str>> = parts
        .iter()
        .map(|part| {
            if part.get("type").and_then(|value| value.as_str()) == Some("text") {
                part.get("text").and_then(|value| value.as_str())
            } else {
                None
            }
        })
        .collect();

    match text_parts {
        Some(text_parts) => serde_json::Value::String(text_parts.concat()),
        None => serde_json::Value::Array(parts),
    }
}
