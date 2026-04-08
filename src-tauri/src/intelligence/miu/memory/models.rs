use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 记忆存储层级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
  /// 用户级：跨世界、跨实例
  User,
  /// 实例级：当前世界
  Instance,
}

/// 记忆分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
  /// 核心人格与偏好
  Personality,
  /// 技能与经验
  Skill,
  /// 世界知识（生物群系、结构等）
  WorldKnowledge,
  /// 玩家关系
  Relationship,
  /// 重要事件
  Event,
  /// 进行中的计划
  Plan,
}

impl MemoryType {
  /// 返回该类型的子目录名
  pub fn dir_name(&self) -> &str {
    match self {
      MemoryType::Personality => "personality",
      MemoryType::Skill => "skills",
      MemoryType::WorldKnowledge => "world_knowledge",
      MemoryType::Relationship => "relationships",
      MemoryType::Event => "events",
      MemoryType::Plan => "plans",
    }
  }

  /// 人类可读名称
  pub fn display_name(&self) -> &str {
    match self {
      MemoryType::Personality => "Personality",
      MemoryType::Skill => "Skill",
      MemoryType::WorldKnowledge => "World Knowledge",
      MemoryType::Relationship => "Relationship",
      MemoryType::Event => "Event",
      MemoryType::Plan => "Plan",
    }
  }
}

/// 记忆文件头（用于索引和召回）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHeader {
  pub name: String,
  pub description: String,
  pub memory_type: MemoryType,
  pub scope: MemoryScope,
  /// 相对于 scope 根目录的路径
  pub relative_path: String,
  pub importance: u8,
  pub created_at: u64,
  pub updated_at: u64,
}

impl MemoryHeader {
  pub fn new(name: &str, memory_type: MemoryType, scope: MemoryScope, importance: u8) -> Self {
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs();

    let filename = sanitize_filename(name);
    let relative_path = format!("{}/{}.md", memory_type.dir_name(), filename);

    Self {
      name: name.to_string(),
      description: String::new(),
      memory_type,
      scope,
      relative_path,
      importance: importance.clamp(1, 10),
      created_at: now,
      updated_at: now,
    }
  }

  pub fn with_description(mut self, desc: &str) -> Self {
    self.description = desc.to_string();
    self
  }
}

/// 召回的记忆（header + 完整内容）
#[derive(Debug, Clone)]
pub struct RecalledMemory {
  pub header: MemoryHeader,
  pub content: String,
}

/// LLM 输出的记忆更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdateRequest {
  pub memory_type: MemoryType,
  pub name: String,
  pub content: String,
  #[serde(default = "default_importance")]
  pub importance: u8,
  #[serde(default)]
  pub scope: Option<MemoryScope>,
}

fn default_importance() -> u8 {
  5
}

/// 记忆召回上下文
#[derive(Debug, Clone, Default)]
pub struct RecallContext {
  /// 当前生物群系
  pub current_biome: Option<String>,
  /// 附近的玩家名
  pub nearby_players: Vec<String>,
  /// 附近的方块类型
  pub nearby_blocks: Vec<String>,
  /// 附近的实体类型
  pub nearby_entities: Vec<String>,
  /// 当前活动（挖矿/建造/探索等）
  pub current_activity: Option<String>,
  /// 自由文本查询（如来自玩家的对话）
  pub query: Option<String>,
}

/// Frontmatter 解析结果
#[derive(Debug, Clone)]
pub struct ParsedMemoryFile {
  pub header: MemoryHeader,
  pub body: String,
}

/// 将名称转为安全的文件名
fn sanitize_filename(name: &str) -> String {
  name
    .to_lowercase()
    .chars()
    .map(|c| {
      if c.is_alphanumeric() || c == '-' || c == '_' {
        c
      } else if c == ' ' {
        '_'
      } else {
        '_'
      }
    })
    .collect::<String>()
    .trim_matches('_')
    .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sanitize_filename() {
    assert_eq!(
      sanitize_filename("Diamond Mining at Y-54"),
      "diamond_mining_at_y-54"
    );
    assert_eq!(sanitize_filename("玩家 Kilox 的偏好"), "___kilox____");
    assert_eq!(sanitize_filename("hello world"), "hello_world");
  }

  #[test]
  fn test_memory_header_path() {
    let header = MemoryHeader::new("diamond mining", MemoryType::Skill, MemoryScope::User, 7);
    assert_eq!(header.relative_path, "skills/diamond_mining.md");
    assert_eq!(header.importance, 7);
  }
}
