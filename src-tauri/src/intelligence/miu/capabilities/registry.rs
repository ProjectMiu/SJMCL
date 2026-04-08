use std::collections::HashMap;

use super::models::CapabilitySpec;
use super::traits::Capability;

/// 能力注册表
///
/// 参照 Claude Code 的 Tool 注册机制：
/// - 启动时注册所有能力
/// - 运行时按名称查找和执行
/// - 生成能力列表供 LLM prompt 使用
pub struct CapabilityRegistry {
  capabilities: HashMap<String, Box<dyn Capability>>,
  /// 注册顺序（用于稳定的 prompt 输出）
  order: Vec<String>,
}

impl CapabilityRegistry {
  pub fn new() -> Self {
    Self {
      capabilities: HashMap::new(),
      order: Vec::new(),
    }
  }

  /// 注册一个能力
  pub fn register(&mut self, capability: impl Capability + 'static) {
    let name = capability.spec().name.clone();
    self.capabilities.insert(name.clone(), Box::new(capability));
    if !self.order.contains(&name) {
      self.order.push(name);
    }
  }

  /// 按名称获取能力
  pub fn get(&self, name: &str) -> Option<&dyn Capability> {
    self.capabilities.get(name).map(|c| c.as_ref())
  }

  /// 获取所有能力的 spec（按注册顺序）
  pub fn specs(&self) -> Vec<CapabilitySpec> {
    self
      .order
      .iter()
      .filter_map(|name| self.capabilities.get(name))
      .map(|c| c.spec())
      .collect()
  }

  /// 已注册的能力数量
  pub fn len(&self) -> usize {
    self.capabilities.len()
  }
}

impl Default for CapabilityRegistry {
  fn default() -> Self {
    Self::new()
  }
}
