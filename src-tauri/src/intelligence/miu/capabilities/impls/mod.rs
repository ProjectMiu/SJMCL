pub mod chat;
pub mod combat;
pub mod interaction;
pub mod inventory;
pub mod mining;
pub mod movement;
pub mod observation;

use super::registry::CapabilityRegistry;

/// 注册所有内置能力
///
/// 参照 Claude Code 在启动时注册所有 Tool 的模式
pub fn register_all(registry: &mut CapabilityRegistry) {
  // 移动类
  registry.register(movement::GotoCapability);
  registry.register(movement::LookAtCapability);
  registry.register(movement::JumpCapability);
  registry.register(movement::CrouchCapability);
  registry.register(movement::StopCapability);

  // 挖掘
  registry.register(mining::MineBlockCapability);

  // 战斗
  registry.register(combat::AttackNearestCapability);

  // 交互
  registry.register(interaction::InteractBlockCapability);
  registry.register(interaction::UseItemCapability);
  registry.register(interaction::OpenContainerCapability);

  // 背包
  registry.register(inventory::SelectHotbarCapability);
  registry.register(inventory::ScanInventoryCapability);

  // 聊天
  registry.register(chat::SendChatCapability);
  registry.register(chat::SendCommandCapability);

  // 观察
  registry.register(observation::ScanAreaCapability);
  registry.register(observation::GetStatusCapability);
  registry.register(observation::WaitCapability);
}
