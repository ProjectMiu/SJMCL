use azalea::prelude::*;
use azalea::BlockPos;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 扩大范围扫描周围方块
pub struct ScanAreaCapability;

#[async_trait::async_trait(?Send)]
impl Capability for ScanAreaCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "scan_area".to_string(),
      description: "Scan the surrounding area for notable blocks in a larger radius (10 blocks). Returns a list of interesting blocks (ores, chests, crafting tables, etc.). Use when you want a broader view of your surroundings.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "radius": {
            "type": "integer",
            "description": "Scan radius in blocks (1-16, default 10)",
            "minimum": 1,
            "maximum": 16
          }
        }
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let radius = params
      .get("radius")
      .and_then(|v| v.as_i64())
      .unwrap_or(10)
      .clamp(1, 16) as i32;

    let position = client.position();
    let block_pos = BlockPos::from(position);

    let world_lock = client.world();
    let instance = world_lock.read();

    let mut notable_blocks = Vec::new();

    for x in -radius..=radius {
      for y in -radius..=radius {
        for z in -radius..=radius {
          let check_pos = block_pos.up(y).east(x).south(z);
          if let Some(state) = instance.get_block_state(check_pos) {
            let block_desc = format!("{:?}", state);
            // 过滤常见无价值方块
            if is_notable_block(&block_desc) {
              notable_blocks.push(serde_json::json!({
                "block": block_desc,
                "x": check_pos.x,
                "y": check_pos.y,
                "z": check_pos.z,
              }));
            }
          }
        }
      }
    }

    // 限制返回数量，避免 token 爆炸
    notable_blocks.truncate(30);

    let data = serde_json::json!({
      "center": { "x": block_pos.x, "y": block_pos.y, "z": block_pos.z },
      "radius": radius,
      "blocks": notable_blocks,
      "total_notable": notable_blocks.len(),
    });

    CapabilityResult::ok_with_data(
      format!(
        "Scanned {}³ area, found {} notable blocks",
        radius * 2 + 1,
        notable_blocks.len()
      ),
      data,
    )
  }
}

/// 查询玩家自身状态
pub struct GetStatusCapability;

#[async_trait::async_trait(?Send)]
impl Capability for GetStatusCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "get_status".to_string(),
      description: "Check your current status: health, hunger, position, and online players. Use to assess your current situation.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {}
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    let pos = client.position();
    let health = client.health();
    let hunger = client.hunger();

    // 在线玩家列表
    let tab_list = client.tab_list();
    let players: Vec<String> = tab_list
      .values()
      .filter_map(|info| info.profile.name.clone().into())
      .collect();

    let data = serde_json::json!({
      "position": { "x": pos.x as i32, "y": pos.y as i32, "z": pos.z as i32 },
      "health": health,
      "hunger": {
        "food": hunger.food,
        "saturation": hunger.saturation,
      },
      "alive": client.is_alive(),
      "online_players": players,
    });

    CapabilityResult::ok_with_data(
      format!(
        "HP: {:.0}/20, Hunger: {}/20, Pos: ({:.0}, {:.0}, {:.0}), {} players online",
        health,
        hunger.food,
        pos.x,
        pos.y,
        pos.z,
        players.len()
      ),
      data,
    )
  }
}

/// 等待/空闲
pub struct WaitCapability;

#[async_trait::async_trait(?Send)]
impl Capability for WaitCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "wait".to_string(),
      description: "Do nothing this cycle. Use when no action is needed, when waiting for something, or when observing the environment.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "reason": {
            "type": "string",
            "description": "Why you're waiting (for logging)"
          }
        }
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, _client: &Client, params: serde_json::Value) -> CapabilityResult {
    let reason = params
      .get("reason")
      .and_then(|v| v.as_str())
      .unwrap_or("observing");
    CapabilityResult::ok(format!("Waiting: {}", reason))
  }
}

/// 判断方块是否值得报告
fn is_notable_block(desc: &str) -> bool {
  // 排除常见无价值方块
  const BORING: &[&str] = &[
    "Air",
    "Stone",
    "Dirt",
    "Grass",
    "Bedrock",
    "Sand",
    "Gravel",
    "Water",
    "Lava",
    "Deepslate",
    "Tuff",
    "Dripstone",
    "Netherrack",
    "Endstone",
    "Cobblestone",
    "Andesite",
    "Diorite",
    "Granite",
    "Clay",
    "Snow",
    "Ice",
    "Terracotta",
    "Sandstone",
    "Calcite",
    "SmoothBasalt",
    "Mud",
  ];
  !BORING.iter().any(|b| desc.contains(b))
}
