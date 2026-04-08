use azalea::prelude::Client;

use super::models::{CapabilityResult, CapabilitySpec};

/// 能力协议 trait
///
/// 参照 Claude Code 的 Tool 协议，每个能力实现：
/// - `spec()`: 元数据（名称、描述、参数 schema）— 用于生成 LLM prompt
/// - `validate()`: 参数校验
/// - `execute()`: 实际执行
///
/// 使用 `?Send` 因为 azalea Client 的操作在 `spawn_local` 上下文中执行
#[async_trait::async_trait(?Send)]
pub trait Capability: Send + Sync {
  /// 返回能力元数据（用于 LLM prompt 和注册表）
  fn spec(&self) -> CapabilitySpec;

  /// 校验参数是否合法
  fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
    // 默认实现：不校验
    let _ = params;
    Ok(())
  }

  /// 执行能力
  ///
  /// `client`: azalea bot 客户端
  /// `params`: LLM 输出的参数 JSON
  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult;
}
