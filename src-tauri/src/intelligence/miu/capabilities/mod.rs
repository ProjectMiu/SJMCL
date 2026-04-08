pub mod impls;
pub mod models;
pub mod prompt_builder;
pub mod registry;
pub mod traits;

use azalea::prelude::Client;
use models::{ActionRequest, AgentResponse, CapabilityResult};
use registry::CapabilityRegistry;

/// 创建并注册所有内置能力的注册表
pub fn create_default_registry() -> CapabilityRegistry {
  let mut registry = CapabilityRegistry::new();
  impls::register_all(&mut registry);
  log::info!("Registered {} capabilities", registry.len());
  registry
}

/// 执行管线：解析 LLM 响应 → 校验 → 分发执行
///
/// 参照 Claude Code 的 Tool pipeline:
/// 1. 找到对应 capability
/// 2. 校验参数
/// 3. 执行
/// 4. 返回结果
pub async fn execute_action(
  registry: &CapabilityRegistry,
  client: &Client,
  request: &ActionRequest,
) -> CapabilityResult {
  // 1. 查找能力
  let capability = match registry.get(&request.capability) {
    Some(c) => c,
    None => {
      return CapabilityResult::fail(format!(
        "Unknown capability '{}'. Available: {}",
        request.capability,
        registry
          .specs()
          .iter()
          .map(|s| s.name.as_str())
          .collect::<Vec<_>>()
          .join(", ")
      ));
    }
  };

  // 2. 校验参数
  if let Err(e) = capability.validate(&request.parameters) {
    return CapabilityResult::fail(format!(
      "Invalid parameters for '{}': {}",
      request.capability, e
    ));
  }

  // 3. 执行
  log::info!(
    "Executing capability '{}' with params: {}",
    request.capability,
    request.parameters
  );

  let result = capability.execute(client, request.parameters.clone()).await;

  log::info!(
    "Capability '{}' result: success={}, message={}",
    request.capability,
    result.success,
    result.message
  );

  result
}

/// 解析 LLM 的 JSON 响应为 AgentResponse
///
/// 容错设计：即使 memory_updates 格式不正确，仍然提取 thought + action
pub fn parse_agent_response(raw: &str) -> Result<AgentResponse, String> {
  // 先尝试完整解析
  if let Ok(resp) = serde_json::from_str::<AgentResponse>(raw) {
    return Ok(resp);
  }

  // 降级：解析为通用 JSON，提取核心字段，忽略 memory_updates 解析错误
  let val: serde_json::Value = serde_json::from_str(raw)
    .map_err(|e| format!("Invalid JSON from LLM: {}. Raw: {}", e, truncate(raw, 200)))?;

  let thought = val
    .get("thought")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();

  let action: ActionRequest = val
    .get("action")
    .ok_or_else(|| format!("Missing 'action' field. Raw: {}", truncate(raw, 200)))
    .and_then(|v| {
      serde_json::from_value(v.clone())
        .map_err(|e| format!("Invalid 'action' field: {}. Raw: {}", e, truncate(raw, 200)))
    })?;

  // Best-effort 解析 memory_updates — 逐条尝试，跳过失败的
  let memory_updates = val
    .get("memory_updates")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|item| serde_json::from_value(item.clone()).ok())
        .collect()
    })
    .unwrap_or_default();

  Ok(AgentResponse {
    thought,
    action,
    memory_updates,
  })
}

fn truncate(s: &str, max: usize) -> &str {
  if s.len() <= max {
    s
  } else {
    &s[..max]
  }
}
