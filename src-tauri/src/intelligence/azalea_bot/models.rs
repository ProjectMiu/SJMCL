use azalea::prelude::{Client, Component};
use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use crate::intelligence::miu::capabilities::registry::CapabilityRegistry;
use crate::intelligence::miu::memory::MemorySystem;

#[derive(Clone, Component)]
pub struct BotState {
  pub client: Arc<tokio::sync::Mutex<Option<Client>>>,
  pub app_handle: Option<AppHandle>,
  pub exit_notified: Arc<AtomicBool>,
  pub last_action_time: Arc<std::sync::Mutex<Instant>>,
  pub cooldown: Duration,
  /// 能力注册表（全部已注册的 bot 能力）
  pub registry: Arc<CapabilityRegistry>,
  /// 记忆系统
  pub memory: Arc<tokio::sync::Mutex<MemorySystem>>,
}

impl Default for BotState {
  fn default() -> Self {
    Self {
      client: Arc::new(tokio::sync::Mutex::new(None)),
      app_handle: None,
      exit_notified: Arc::new(AtomicBool::new(false)),
      last_action_time: Arc::new(std::sync::Mutex::new(Instant::now())),
      cooldown: Duration::from_secs(6),
      registry: Arc::new(crate::intelligence::miu::capabilities::create_default_registry()),
      memory: Arc::new(tokio::sync::Mutex::new(MemorySystem::new(
        crate::APP_DATA_DIR.get().unwrap(),
      ))),
    }
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BotExitPayload {
  pub reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPortPayload {
  pub port: String,
}
