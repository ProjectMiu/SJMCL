use azalea::prelude::*;
use azalea::BlockPos;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 交互方块：右键点击指定方块（开门、按按钮、打开工作台等）
pub struct InteractBlockCapability;

#[async_trait::async_trait(?Send)]
impl Capability for InteractBlockCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "interact_block".to_string(),
      description: "Right-click/interact with a block. Use to open doors, press buttons, flip levers, use crafting tables, furnaces, etc. The block must be within reach.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "x": { "type": "integer", "description": "Block X coordinate" },
          "y": { "type": "integer", "description": "Block Y coordinate" },
          "z": { "type": "integer", "description": "Block Z coordinate" }
        },
        "required": ["x", "y", "z"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let x = params["x"].as_i64().unwrap_or(0) as i32;
    let y = params["y"].as_i64().unwrap_or(0) as i32;
    let z = params["z"].as_i64().unwrap_or(0) as i32;
    let pos = BlockPos::new(x, y, z);

    client.block_interact(pos);
    CapabilityResult::ok(format!("Interacted with block at ({}, {}, {})", x, y, z))
  }
}

/// 使用手持物品
pub struct UseItemCapability;

#[async_trait::async_trait(?Send)]
impl Capability for UseItemCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "use_item".to_string(),
      description: "Use the currently held item (right-click action). For food: eat it. For a bow: start drawing. For a bucket: place/collect liquid. For an ender pearl: throw it.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {}
      }),
      interruptible: false,
      duration: ActionDuration::Short,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    client.start_use_item();
    CapabilityResult::ok("Using held item")
  }
}

/// 打开容器（箱子、熔炉等）
pub struct OpenContainerCapability;

#[async_trait::async_trait(?Send)]
impl Capability for OpenContainerCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "open_container".to_string(),
      description: "Open a container block (chest, barrel, furnace, etc.) at the specified position. Must be within reach (~4.5 blocks).".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "x": { "type": "integer", "description": "Container X coordinate" },
          "y": { "type": "integer", "description": "Container Y coordinate" },
          "z": { "type": "integer", "description": "Container Z coordinate" }
        },
        "required": ["x", "y", "z"]
      }),
      interruptible: false,
      duration: ActionDuration::Short,
    }
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let x = params["x"].as_i64().unwrap_or(0) as i32;
    let y = params["y"].as_i64().unwrap_or(0) as i32;
    let z = params["z"].as_i64().unwrap_or(0) as i32;
    let pos = BlockPos::new(x, y, z);

    match client.open_container_at(pos).await {
      Some(_handle) => CapabilityResult::ok(format!("Opened container at ({}, {}, {})", x, y, z)),
      None => CapabilityResult::fail(format!(
        "Failed to open container at ({}, {}, {}). No container there or out of reach.",
        x, y, z
      )),
    }
  }
}
