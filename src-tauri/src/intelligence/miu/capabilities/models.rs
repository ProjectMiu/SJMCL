use serde::{Deserialize, Serialize};

use crate::intelligence::miu::memory::models::MemoryUpdateRequest;

/// 能力元数据：描述一个能力的名称、用途和参数格式
///
/// 参照 Claude Code 的 Tool.inputSchema，使用 JSON Schema 描述参数
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySpec {
  /// 唯一标识符（如 "goto", "mine_block"）
  pub name: String,
  /// 面向 LLM 的描述（英文，让 LLM 理解何时使用）
  pub description: String,
  /// 参数 JSON Schema
  pub parameters: serde_json::Value,
  /// 是否可被中断（如寻路可中断，攻击不可）
  pub interruptible: bool,
  /// 预估耗时类别
  pub duration: ActionDuration,
}

/// 动作耗时类别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionDuration {
  /// 立即完成（look_at, chat, jump）
  Instant,
  /// 短耗时（attack, interact）
  Short,
  /// 持续性（mine, goto — 可能跨多个 tick）
  Long,
}

/// 能力执行结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityResult {
  pub success: bool,
  pub message: String,
  /// 可选返回数据（如 scan_inventory 返回物品列表）
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<serde_json::Value>,
}

impl CapabilityResult {
  pub fn ok(message: impl Into<String>) -> Self {
    Self {
      success: true,
      message: message.into(),
      data: None,
    }
  }

  pub fn ok_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
    Self {
      success: true,
      message: message.into(),
      data: Some(data),
    }
  }

  pub fn fail(message: impl Into<String>) -> Self {
    Self {
      success: false,
      message: message.into(),
      data: None,
    }
  }
}

/// LLM 输出的动作请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
  /// 能力名称（对应 CapabilitySpec.name）
  pub capability: String,
  /// 参数 JSON
  #[serde(default)]
  pub parameters: serde_json::Value,
}

/// LLM 完整响应（思考 + 动作 + 记忆更新）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
  /// 思考过程（chain-of-thought）
  pub thought: String,
  /// 要执行的动作
  pub action: ActionRequest,
  /// 可选的记忆更新
  #[serde(default)]
  pub memory_updates: Vec<MemoryUpdateRequest>,
}
