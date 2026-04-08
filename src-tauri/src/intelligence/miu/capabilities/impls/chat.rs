use azalea::prelude::*;

use crate::intelligence::miu::capabilities::models::{
  ActionDuration, CapabilityResult, CapabilitySpec,
};
use crate::intelligence::miu::capabilities::traits::Capability;

/// 发送聊天消息
pub struct SendChatCapability;

#[async_trait::async_trait(?Send)]
impl Capability for SendChatCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "chat".to_string(),
      description: "Send a chat message visible to all players on the server. Use to communicate, greet players, ask questions, or share discoveries.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "message": {
            "type": "string",
            "description": "The message to send (max 256 characters)"
          }
        },
        "required": ["message"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
    let msg = params
      .get("message")
      .and_then(|v| v.as_str())
      .ok_or("Missing 'message' parameter")?;
    if msg.is_empty() {
      return Err("Message cannot be empty".to_string());
    }
    if msg.len() > 256 {
      return Err("Message too long (max 256 characters)".to_string());
    }
    Ok(())
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let message = params["message"].as_str().unwrap_or("");

    // 过滤命令前缀，防止意外执行服务器命令
    if message.starts_with('/') {
      return CapabilityResult::fail(
        "Cannot send commands via chat. Use 'send_command' capability instead.",
      );
    }

    client.chat(message);
    CapabilityResult::ok(format!("Sent chat: {}", message))
  }
}

/// 发送服务器命令
pub struct SendCommandCapability;

#[async_trait::async_trait(?Send)]
impl Capability for SendCommandCapability {
  fn spec(&self) -> CapabilitySpec {
    CapabilitySpec {
      name: "send_command".to_string(),
      description: "Execute a server command (without the leading /). Example: 'tp @s 0 64 0'. Only use when you know the command is safe and allowed.".to_string(),
      parameters: serde_json::json!({
        "type": "object",
        "properties": {
          "command": {
            "type": "string",
            "description": "The command to execute (without leading /)"
          }
        },
        "required": ["command"]
      }),
      interruptible: false,
      duration: ActionDuration::Instant,
    }
  }

  fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
    let cmd = params
      .get("command")
      .and_then(|v| v.as_str())
      .ok_or("Missing 'command' parameter")?;
    if cmd.is_empty() {
      return Err("Command cannot be empty".to_string());
    }
    Ok(())
  }

  async fn execute(&self, client: &Client, params: serde_json::Value) -> CapabilityResult {
    let command = params["command"].as_str().unwrap_or("");
    // 移除可能的前导 /
    let cmd = command.strip_prefix('/').unwrap_or(command);

    client.write_command_packet(cmd);
    CapabilityResult::ok(format!("Executed command: /{}", cmd))
  }
}
