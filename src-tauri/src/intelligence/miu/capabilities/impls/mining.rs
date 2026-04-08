use azalea::prelude::*;
use azalea::BlockPos;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 挖掘方块：自动选择最佳工具并挖掘指定坐标的方块
pub struct MineBlockCapability;

#[async_trait::async_trait(?Send)]
impl Capability for MineBlockCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "mine_block".to_string(),
      description: "Mine/break a block at the specified coordinates. Automatically selects the best tool from your hotbar. The block must be within reach (~4.5 blocks).".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "x": { "type": "integer", "description": "Block X coordinate" },
          "y": { "type": "integer", "description": "Block Y coordinate" },
          "z": { "type": "integer", "description": "Block Z coordinate" }
        },
        "required": ["x", "y", "z"]
      }),
      interruptible: true,
      duration: ActionDuration::Long,
    }
  }

  fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
    for key in &["x", "y", "z"] {
      params
        .get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("Missing or invalid parameter: {}", key))?;
    }
    Ok(())
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let x = params["x"].as_i64().unwrap() as i32;
    let y = params["y"].as_i64().unwrap() as i32;
    let z = params["z"].as_i64().unwrap() as i32;
    let pos = BlockPos::new(x, y, z);

    // 异步挖掘：在 spawn_local 中运行，避免阻塞决策循环
    let bot = client.clone();
    tokio::task::spawn_local(async move {
      bot.mine_with_auto_tool(pos).await;
    });

    CapabilityResult::ok(format!("Started mining block at ({}, {}, {})", x, y, z))
  }
}
