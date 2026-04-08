use azalea::pathfinder::goals::BlockPosGoal;
use azalea::prelude::*;
use azalea::BlockPos;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 寻路导航：使用 Baritone 算法自动导航到目标坐标
pub struct GotoCapability;

#[async_trait::async_trait(?Send)]
impl Capability for GotoCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "goto".to_string(),
      description: "Navigate to target coordinates using A* pathfinding. Handles obstacles, jumping, and swimming automatically. Use this to move to distant locations.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "x": { "type": "integer", "description": "Target X coordinate" },
          "y": { "type": "integer", "description": "Target Y coordinate" },
          "z": { "type": "integer", "description": "Target Z coordinate" }
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

    let goal = BlockPosGoal(BlockPos::new(x, y, z));
    client.start_goto(goal);

    CapabilityResult::ok(format!("Started pathfinding to ({}, {}, {})", x, y, z))
  }
}

/// 注视：转动视角看向指定坐标
pub struct LookAtCapability;

#[async_trait::async_trait(?Send)]
impl Capability for LookAtCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "look_at".to_string(),
      description:
        "Turn to face a specific position. Useful before interacting or to observe a location."
          .to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "x": { "type": "number", "description": "Target X coordinate" },
          "y": { "type": "number", "description": "Target Y coordinate" },
          "z": { "type": "number", "description": "Target Z coordinate" }
        },
        "required": ["x", "y", "z"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let x = params["x"].as_f64().unwrap_or(0.0);
    let y = params["y"].as_f64().unwrap_or(0.0);
    let z = params["z"].as_f64().unwrap_or(0.0);

    client.look_at(azalea::Vec3::new(x, y, z));
    CapabilityResult::ok(format!("Now looking at ({:.1}, {:.1}, {:.1})", x, y, z))
  }
}

/// 跳跃
pub struct JumpCapability;

#[async_trait::async_trait(?Send)]
impl Capability for JumpCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "jump".to_string(),
      description: "Jump once. Useful for getting over small obstacles or reaching higher blocks."
        .to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {}
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    client.jump();
    CapabilityResult::ok("Jumped")
  }
}

/// 蹲伏切换
pub struct CrouchCapability;

#[async_trait::async_trait(?Send)]
impl Capability for CrouchCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "crouch".to_string(),
      description: "Toggle crouching/sneaking. Crouching prevents falling off edges and makes you harder to detect.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "enabled": { "type": "boolean", "description": "true to start crouching, false to stop" }
        },
        "required": ["enabled"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let enabled = params["enabled"].as_bool().unwrap_or(true);
    client.set_crouching(enabled);
    let msg = if enabled {
      "Started crouching"
    } else {
      "Stopped crouching"
    };
    CapabilityResult::ok(msg)
  }
}

/// 停止当前运动（寻路/挖掘）
pub struct StopCapability;

#[async_trait::async_trait(?Send)]
impl Capability for StopCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "stop".to_string(),
      description:
        "Stop all current movement and pathfinding. Use when you need to pause or change plans."
          .to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {}
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  async fn execute(&self, client: &Client, _params: serde_json::Value) -> CapabilityResult {
    client.stop_pathfinding();
    CapabilityResult::ok("Stopped all movement")
  }
}
