use azalea::prelude::*;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 切换快捷栏槽位
pub struct SelectHotbarCapability;

#[async_trait::async_trait(?Send)]
impl Capability for SelectHotbarCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "select_slot".to_string(),
      description: "Switch the active hotbar slot (0-8). Use to equip different tools or items before mining, fighting, or using items.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "slot": {
            "type": "integer",
            "description": "Hotbar slot index (0-8)",
            "minimum": 0,
            "maximum": 8
          }
        },
        "required": ["slot"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
    let slot = params
      .get("slot")
      .and_then(|v| v.as_u64())
      .ok_or("Missing or invalid 'slot' parameter")?;
    if slot > 8 {
      return Err("Slot must be 0-8".to_string());
    }
    Ok(())
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let slot = params["slot"].as_u64().unwrap() as u8;
    client.set_selected_hotbar_slot(slot);
    CapabilityResult::ok(format!("Switched to hotbar slot {}", slot))
  }
}

/// 检查背包内容（信息查询型能力）
pub struct ScanInventoryCapability;

#[async_trait::async_trait(?Send)]
impl Capability for ScanInventoryCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "scan_inventory".to_string(),
      description: "Inspect your inventory contents. Returns a summary of items in your hotbar and main inventory. Use to check what tools and resources you have.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {}
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    let menu = client.menu();

    let mut items = Vec::new();
    // 使用 Player menu 的结构获取物品
    if let Some(player) = menu.try_as_player() {
      // hotbar + main inventory = 36 slots
      for (idx, slot) in player.inventory.iter().enumerate() {
        if !slot.is_empty() {
          items.push(serde_json::json!({
            "slot": idx,
            "item": format!("{:?}", slot.kind()),
            "count": slot.count(),
          }));
        }
      }
    }

    let held = client.get_held_item();
    let held_desc = if held.is_empty() {
      "empty hand".to_string()
    } else {
      format!("{:?} x{}", held.kind(), held.count())
    };

    let data = serde_json::json!({
      "held_item": held_desc,
      "selected_slot": client.selected_hotbar_slot(),
      "items": items,
      "total_items": items.len(),
    });

    CapabilityResult::ok_with_data(
      format!(
        "Inventory: {} item stacks, holding {}",
        items.len(),
        held_desc
      ),
      data,
    )
  }
}
