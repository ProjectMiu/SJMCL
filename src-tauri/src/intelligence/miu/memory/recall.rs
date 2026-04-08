use super::models::{MemoryHeader, MemoryScope, MemoryType, RecallContext, RecalledMemory};
use super::storage::MemoryStorage;
use crate::error::SJMCLResult;

/// 基于上下文召回最相关的记忆
///
/// 不使用 LLM sidequery（节省 token），而是基于规则的相关性评分
pub async fn recall_relevant(
  storage: &MemoryStorage,
  context: &RecallContext,
  limit: usize,
) -> SJMCLResult<Vec<RecalledMemory>> {
  // 1. 扫描所有记忆文件头
  let mut all_headers = storage.scan_headers(&MemoryScope::User).await?;

  // 也扫描实例记忆
  if storage.instance_dir().is_some() {
    if let Ok(instance_headers) = storage.scan_headers(&MemoryScope::Instance).await {
      all_headers.extend(instance_headers);
    }
  }

  if all_headers.is_empty() {
    return Ok(vec![]);
  }

  // 2. 对每条记忆计算相关性分数
  let mut scored: Vec<(f64, MemoryHeader)> = all_headers
    .into_iter()
    .map(|h| {
      let score = compute_relevance_score(&h, context);
      (score, h)
    })
    .filter(|(score, _)| *score > 0.0)
    .collect();

  // 3. 按分数降序排列
  scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

  // 4. 取 top-N 并读取完整内容
  let mut result = Vec::new();
  for (_, header) in scored.into_iter().take(limit) {
    match storage
      .read_memory(&header.scope, &header.relative_path)
      .await
    {
      Ok(parsed) => {
        result.push(RecalledMemory {
          header: parsed.header,
          content: parsed.body,
        });
      }
      Err(e) => {
        log::warn!("Failed to read memory {}: {:?}", header.relative_path, e);
      }
    }
  }

  Ok(result)
}

/// 召回特定玩家的关系记忆
pub async fn recall_player(
  storage: &MemoryStorage,
  player_name: &str,
) -> SJMCLResult<Option<RecalledMemory>> {
  let headers = storage.scan_headers(&MemoryScope::User).await?;
  let player_lower = player_name.to_lowercase();

  for header in headers {
    if header.memory_type == MemoryType::Relationship
      && (header.name.to_lowercase().contains(&player_lower)
        || header.relative_path.to_lowercase().contains(&player_lower))
    {
      if let Ok(parsed) = storage
        .read_memory(&header.scope, &header.relative_path)
        .await
      {
        return Ok(Some(RecalledMemory {
          header: parsed.header,
          content: parsed.body,
        }));
      }
    }
  }

  Ok(None)
}

/// 计算单条记忆与当前上下文的相关性分数
fn compute_relevance_score(header: &MemoryHeader, context: &RecallContext) -> f64 {
  let mut score = 0.0;

  // 基础分：importance (1-10) 归一化
  score += header.importance as f64 / 10.0;

  // 时间衰减：越新的记忆分数越高
  let age_hours = {
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs();
    (now.saturating_sub(header.updated_at)) as f64 / 3600.0
  };
  // 半衰期 72 小时
  let recency_bonus = (-age_hours / 72.0).exp();
  score += recency_bonus * 0.3;

  // 类型与上下文匹配
  match header.memory_type {
    MemoryType::Personality => {
      // 人格记忆始终有一定相关性
      score += 0.5;
    }
    MemoryType::Relationship => {
      // 附近有玩家时，关系记忆高度相关
      if !context.nearby_players.is_empty() {
        let player_match = context.nearby_players.iter().any(|p| {
          header.name.to_lowercase().contains(&p.to_lowercase())
            || header
              .relative_path
              .to_lowercase()
              .contains(&p.to_lowercase())
        });
        if player_match {
          score += 2.0; // 强匹配
        } else {
          score += 0.5; // 弱匹配：有玩家但不是已知玩家
        }
      }
    }
    MemoryType::Skill => {
      // 根据当前活动匹配技能
      if let Some(ref activity) = context.current_activity {
        if header
          .name
          .to_lowercase()
          .contains(&activity.to_lowercase())
          || header
            .description
            .to_lowercase()
            .contains(&activity.to_lowercase())
        {
          score += 1.5;
        }
      }
      // 附近有危险实体时，战斗技能更相关
      if !context.nearby_entities.is_empty()
        && (header.name.to_lowercase().contains("combat")
          || header.name.to_lowercase().contains("attack")
          || header.name.to_lowercase().contains("fight"))
      {
        score += 1.0;
      }
    }
    MemoryType::WorldKnowledge => {
      // 当前生物群系匹配
      if let Some(ref biome) = context.current_biome {
        if header.name.to_lowercase().contains(&biome.to_lowercase())
          || header
            .description
            .to_lowercase()
            .contains(&biome.to_lowercase())
        {
          score += 1.5;
        }
      }
    }
    MemoryType::Event => {
      // 事件记忆：较低的基础相关性，但最近事件更重要
      score += recency_bonus * 0.5;
    }
    MemoryType::Plan => {
      // 活跃计划始终相关
      score += 1.0;
    }
  }

  // 自由文本查询匹配
  if let Some(ref query) = context.query {
    let query_lower = query.to_lowercase();
    let name_lower = header.name.to_lowercase();
    let desc_lower = header.description.to_lowercase();

    if name_lower.contains(&query_lower) || desc_lower.contains(&query_lower) {
      score += 2.0;
    }
    // 按词匹配
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let word_matches = query_words
      .iter()
      .filter(|w| name_lower.contains(*w) || desc_lower.contains(*w))
      .count();
    score += word_matches as f64 * 0.3;
  }

  score
}
