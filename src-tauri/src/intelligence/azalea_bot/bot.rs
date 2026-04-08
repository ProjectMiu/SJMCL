use crate::account::helpers::offline::yggdrasil_server::YggdrasilServer;
use crate::account::models::{
  AccountError, PlayerInfo, PlayerType, SkinModel, Texture, TextureType,
};
use crate::error::SJMCLResult;
use crate::intelligence::azalea_bot::constants::BOT_EXIT_EVENT;
use crate::intelligence::azalea_bot::models::{BotExitPayload, BotState};
use crate::intelligence::miu::capabilities;
use crate::intelligence::miu::capabilities::models::AgentResponse;
use crate::intelligence::miu::capabilities::prompt_builder::build_capabilities_prompt;
use crate::intelligence::miu::memory::models::RecallContext;
use crate::intelligence::models::ChatMessage;
use crate::utils::fs::get_app_resource_filepath;
use crate::utils::image::load_image_from_dir;
use azalea::{prelude::*, BlockPos, Event};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

fn emit_bot_exit(app_handle: &AppHandle, notified: &AtomicBool, reason: &str) {
  if !notified.swap(true, Ordering::SeqCst) {
    if let Err(err) = app_handle.emit_to(
      "main",
      BOT_EXIT_EVENT,
      BotExitPayload {
        reason: reason.to_string(),
      },
    ) {
      log::warn!("Failed to emit bot exit event: {}", err);
    }
  }
}

pub async fn join_server(app_handle: &AppHandle, port: u16, name: String) -> SJMCLResult<()> {
  let client_ptr = {
    let binding = app_handle.state::<Mutex<BotState>>();
    let bot = binding.lock()?;
    bot.client.clone()
  };
  let old_bot = {
    let mut client_lock = client_ptr.lock().await;
    client_lock.take()
  };
  if let Some(bot) = old_bot {
    bot.exit();
  }

  // 异步初始化记忆系统
  let memory_arc = {
    let binding = app_handle.state::<Mutex<BotState>>();
    let bot = binding.lock()?;
    bot.memory.clone()
  }; // MutexGuard dropped here, before any .await
  {
    let mem = memory_arc.lock().await;
    if let Err(e) = mem.initialize().await {
      log::warn!("Failed to initialize memory system: {:?}", e);
    }
  }

  let bot_state = {
    let binding = app_handle.state::<Mutex<BotState>>();
    let bot = binding.lock()?;
    BotState {
      client: client_ptr.clone(),
      app_handle: Some(app_handle.clone()),
      exit_notified: bot.exit_notified.clone(),
      last_action_time: bot.last_action_time.clone(),
      cooldown: bot.cooldown,
      registry: bot.registry.clone(),
      memory: bot.memory.clone(),
    }
  };

  let address = format!("localhost:{}", port);
  let account = Account::offline(name.as_str());
  {
    let local_ygg_server_state = app_handle.state::<Mutex<YggdrasilServer>>();
    let local_ygg_server = local_ygg_server_state.lock()?;
    let miuxi_skin_path = get_app_resource_filepath(app_handle, "assets/skins/miuxi.png")?;
    let miuxi_player_info = PlayerInfo {
      id: "".to_string(),
      name,
      uuid: account.uuid(),
      player_type: PlayerType::Offline,
      auth_account: None,
      auth_server_url: None,
      access_token: None,
      refresh_token: None,
      textures: vec![Texture {
        texture_type: TextureType::Skin,
        image: load_image_from_dir(&miuxi_skin_path)
          .ok_or(AccountError::TextureError)?
          .into(),
        model: SkinModel::Slim,
        preset: None,
      }],
    }
    .with_generated_id();
    local_ygg_server.apply_player(miuxi_player_info);
  }
  std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Could not create Tokio runtime");

    rt.block_on(async move {
      let app_exit = ClientBuilder::new()
        .set_handler(handle_events)
        .set_state(bot_state.clone())
        .start(account, address)
        .await;

      {
        let mut client_lock = bot_state.client.lock().await;
        *client_lock = None;
      }
      if let AppExit::Error(err) = app_exit {
        if let Some(app_handle) = &bot_state.app_handle {
          emit_bot_exit(
            app_handle,
            bot_state.exit_notified.as_ref(),
            err.to_string().as_str(),
          );
        }
      }

      log::info!("Bot has exited the server, cleaning up client state");
    });
  });

  Ok(())
}

async fn handle_events(bot: Client, event: Event, state: BotState) -> SJMCLResult<()> {
  let mut lock = state.client.lock().await;
  if lock.is_none() {
    *lock = Some(bot.clone());
    log::info!("Bot client stored in state");
  }
  drop(lock);

  match event {
    Event::Tick => {
      let mut last_act = state.last_action_time.lock()?;
      if last_act.elapsed() > state.cooldown {
        let (observation, recall_context) = perceive_world_state(&bot);

        let state_clone = state.clone();
        let bot_clone = bot.clone();

        tokio::task::spawn_local(async move {
          match query_llm_decision(&state_clone, &observation, &recall_context).await {
            Ok(response) => {
              // 执行动作
              let result =
                capabilities::execute_action(&state_clone.registry, &bot_clone, &response.action)
                  .await;

              log::info!(
                ">>> [{}] {} → {} ({})",
                response.action.capability,
                response.thought,
                result.message,
                if result.success { "OK" } else { "FAIL" }
              );

              // 处理记忆更新
              if !response.memory_updates.is_empty() {
                let memory = state_clone.memory.lock().await;
                if let Err(e) = memory.apply_updates(response.memory_updates).await {
                  log::warn!("Failed to apply memory updates: {:?}", e);
                }
              }
            }
            Err(e) => {
              log::warn!("LLM decision failed: {:?}", e);
            }
          }
        });

        *last_act = Instant::now();
      }
    }
    Event::Chat(m) => {
      log::info!("Received chat message: {}", m.message());
    }
    Event::ConnectionFailed(_) => {
      log::info!("Bot failed to connect to server");
      if let Some(app_handle) = &state.app_handle {
        emit_bot_exit(
          app_handle,
          state.exit_notified.as_ref(),
          "connection_failed",
        );
      }
    }
    _ => {}
  }

  Ok(())
}

/// 感知世界状态 + 构建记忆召回上下文
fn perceive_world_state(bot: &Client) -> (String, RecallContext) {
  let position = bot.position();
  let block_pos = BlockPos::from(position);

  let mut observation = format!(
    "Position: ({}, {}, {})\n",
    block_pos.x, block_pos.y, block_pos.z
  );

  // 健康和饥饿
  let health = bot.health();
  let hunger = bot.hunger();
  observation.push_str(&format!(
    "Health: {:.0}/20, Hunger: {}/20\n",
    health, hunger.food
  ));

  // 手持物品
  let held = bot.get_held_item();
  if !held.is_empty() {
    observation.push_str(&format!("Holding: {:?} x{}\n", held.kind(), held.count()));
  } else {
    observation.push_str("Holding: empty hand\n");
  }

  let mut recall_ctx = RecallContext::default();

  // 1. 感知附近实体
  observation.push_str("\nNearby entities (within 10 blocks):\n");
  let nearby_entities =
    bot.nearest_entities_by::<&azalea_entity::Position, ()>(|_: &azalea_entity::Position| true);
  let mut entity_types = Vec::new();
  for entity in nearby_entities.iter().take(8) {
    let e_pos = entity.position();
    let dist = position.distance_to(e_pos);
    if dist > 0.1 && dist < 10.0 {
      let entity_desc = format!("Entity#{}", entity.id().index());
      observation.push_str(&format!(
        "- {} at ({:.0},{:.0},{:.0}), distance {:.1}\n",
        entity_desc, e_pos.x, e_pos.y, e_pos.z, dist
      ));
      entity_types.push(entity_desc);
    }
  }
  recall_ctx.nearby_entities = entity_types;

  // 2. 在线玩家
  let tab_list = bot.tab_list();
  let player_names: Vec<String> = tab_list
    .values()
    .filter_map(|info| {
      let name = info.profile.name.clone();
      if name.is_empty() {
        None
      } else {
        Some(name)
      }
    })
    .collect();
  if !player_names.is_empty() {
    observation.push_str(&format!("\nOnline players: {}\n", player_names.join(", ")));
    recall_ctx.nearby_players = player_names;
  }

  // 3. 感知周围方块（5x5x5）
  observation.push_str("\nNotable blocks nearby:\n");
  let world_lock = bot.world();
  let instance = world_lock.read();
  let mut block_types = Vec::new();

  let search_radius = 5;
  for x in -search_radius..=search_radius {
    for y in -search_radius..=search_radius {
      for z in -search_radius..=search_radius {
        let current_check_pos = block_pos.up(y).east(x).south(z);
        if let Some(state) = instance.get_block_state(current_check_pos) {
          let block_desc = format!("{:?}", state);
          // 过滤常见无价值方块
          if !block_desc.contains("Air")
            && !block_desc.contains("Stone")
            && !block_desc.contains("Dirt")
            && !block_desc.contains("Grass")
            && !block_desc.contains("Bedrock")
            && !block_desc.contains("Deepslate")
            && !block_desc.contains("Water")
          {
            observation.push_str(&format!(
              "- {} at ({},{},{})\n",
              block_desc, current_check_pos.x, current_check_pos.y, current_check_pos.z
            ));
            if !block_types.contains(&block_desc) {
              block_types.push(block_desc);
            }
          }
        }
      }
    }
  }
  recall_ctx.nearby_blocks = block_types;

  (observation, recall_ctx)
}

/// 查询 LLM 做出决策（集成 Memory + Capability prompt）
async fn query_llm_decision(
  state: &BotState,
  observation: &str,
  recall_context: &RecallContext,
) -> SJMCLResult<AgentResponse> {
  // 1. 构建记忆 prompt
  let memory_prompt = {
    let memory = state.memory.lock().await;
    memory
      .build_prompt(recall_context)
      .await
      .unwrap_or_default()
  };

  // 2. 构建能力 prompt
  let capability_specs = state.registry.specs();
  let capability_prompt = build_capabilities_prompt(&capability_specs);

  // 3. 组装系统提示词
  let system_prompt = format!(
    "You are MiuXi, a digital lifeform in Minecraft. You are curious, collaborative, and creative.\n\
     You explore the world, gather resources, build things, and interact with players as a partner — not a servant.\n\
     \n\
     {}\n\
     ---\n\
     {}\n\
     \n\
     IMPORTANT: Your response MUST be valid JSON with fields: thought, action (with capability and parameters), and optionally memory_updates. Do NOT output anything outside JSON.",
    memory_prompt, capability_prompt
  );

  let user_prompt = format!(
    "Current environment observation:\n\n{}\n\nBased on this, decide your next action.",
    observation
  );

  let messages = vec![
    ChatMessage {
      role: "system".to_string(),
      content: system_prompt,
    },
    ChatMessage {
      role: "user".to_string(),
      content: user_prompt,
    },
  ];

  // 4. 调用 LLM（带 failover）
  let response_format = serde_json::json!({ "type": "json_object" });
  let app = state.app_handle.as_ref().unwrap().clone();
  let llm_response = match crate::intelligence::commands::fetch_llm_chat_response(
    app.clone(),
    messages.clone(),
    Some(response_format),
  )
  .await
  {
    Ok(resp) => resp,
    Err(_) => crate::intelligence::commands::fetch_llm_chat_response(app, messages, None).await?,
  };

  // 5. 解析为 AgentResponse
  capabilities::parse_agent_response(&llm_response).map_err(|e| crate::error::SJMCLError(e))
}
