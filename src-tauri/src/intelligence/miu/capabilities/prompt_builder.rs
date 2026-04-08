use super::models::CapabilitySpec;

/// 将能力列表构建为 LLM 系统 prompt 的一部分
///
/// 参照 Claude Code 将 Tool descriptions 注入 system prompt 的方式
pub fn build_capabilities_prompt(specs: &[CapabilitySpec]) -> String {
  if specs.is_empty() {
    return String::new();
  }

  let mut lines = Vec::new();
  lines.push("## Available Actions\n".to_string());
  lines.push("You can perform ONE action per decision cycle. Choose the most appropriate action based on your current situation.\n".to_string());
  lines.push("Respond with a JSON object containing:".to_string());
  lines.push("- `thought`: your reasoning (1-2 sentences)".to_string());
  lines.push("- `action`: an object with `capability` (action name) and `parameters`".to_string());
  lines.push("- `memory_updates`: (optional) array of memories to save\n".to_string());

  for spec in specs {
    lines.push(format!("### `{}`", spec.name));
    lines.push(spec.description.clone());
    // 参数说明
    if let Some(props) = spec.parameters.get("properties") {
      let required: Vec<String> = spec
        .parameters
        .get("required")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_default();

      lines.push("**Parameters:**".to_string());
      if let Some(obj) = props.as_object() {
        for (key, schema) in obj {
          let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("any");
          let desc = schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
          let req = if required.contains(key) {
            " (required)"
          } else {
            " (optional)"
          };
          lines.push(format!("- `{}` ({}{}): {}", key, type_str, req, desc));
        }
      }
    }
    lines.push(String::new());
  }

  // 响应格式示例
  lines.push("### Response Format Example".to_string());
  lines.push("```json".to_string());
  lines.push(
    r#"{
  "thought": "I see diamond ore nearby, I should mine it",
  "action": {
    "capability": "mine_block",
    "parameters": { "x": 10, "y": -54, "z": 20 }
  },
  "memory_updates": [
    {
      "memory_type": "event",
      "name": "found diamond ore",
      "content": "Discovered diamond ore at (10, -54, 20)",
      "importance": 7
    }
  ]
}"#
      .to_string(),
  );
  lines.push("```".to_string());

  lines.join("\n")
}
