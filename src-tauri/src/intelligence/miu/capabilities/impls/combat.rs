use azalea::prelude::*;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 攻击最近的实体
///
/// 自动转向目标并检查攻击冷却，模拟人类行为
pub struct AttackNearestCapability;

#[async_trait::async_trait(?Send)]
impl Capability for AttackNearestCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "attack".to_string(),
      description: "Attack the nearest entity (mob or player). Automatically turns to face the target and respects attack cooldown to avoid anti-cheat detection. Attacks the nearest living entity within range.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "reason": {
            "type": "string",
            "description": "Why you're attacking (for memory/logging)"
          }
        }
      }),
      interruptible: false,
      duration: ActionDuration::Short,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    // 查找最近的可攻击实体
    let target =
      client.nearest_entity_by::<&azalea_entity::Position, ()>(|_: &azalea_entity::Position| true);

    match target {
      Some(entity) => {
        let dist = client.position().distance_to(entity.position());
        if dist > 6.0 {
          return CapabilityResult::fail(format!(
            "Nearest entity is too far ({:.1} blocks away)",
            dist
          ));
        }

        // 先看向目标
        client.look_at(entity.position());

        // 检查攻击冷却
        if client.has_attack_cooldown() {
          return CapabilityResult::fail("Attack is on cooldown, try again next tick");
        }

        client.attack(entity.id());
        CapabilityResult::ok(format!("Attacked entity at distance {:.1} blocks", dist))
      }
      None => CapabilityResult::fail("No entities nearby to attack"),
    }
  }
}
