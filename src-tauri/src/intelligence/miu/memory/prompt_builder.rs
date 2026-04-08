use super::models::{MemoryType, RecalledMemory};

/// 构建注入 LLM 系统 prompt 的记忆上下文
pub fn build_memory_prompt(memories: &[RecalledMemory], entrypoint_content: &str) -> String {
  let mut sections = Vec::new();

  // 核心身份（人格记忆）
  let personality: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::Personality)
    .collect();

  if !personality.is_empty() {
    let mut s = String::from("### Who You Are\n");
    for mem in personality {
      s.push_str(&mem.content);
      s.push('\n');
    }
    sections.push(s);
  }

  // 相关技能
  let skills: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::Skill)
    .collect();

  if !skills.is_empty() {
    let mut s = String::from("### Relevant Skills\n");
    for mem in &skills {
      s.push_str(&format!(
        "**{}** (importance: {})\n",
        mem.header.name, mem.header.importance
      ));
      s.push_str(&truncate_content(&mem.content, 500));
      s.push_str("\n\n");
    }
    sections.push(s);
  }

  // 玩家关系
  let relationships: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::Relationship)
    .collect();

  if !relationships.is_empty() {
    let mut s = String::from("### People You Know\n");
    for mem in &relationships {
      s.push_str(&format!("**{}**\n", mem.header.name));
      s.push_str(&truncate_content(&mem.content, 800));
      s.push_str("\n\n");
    }
    sections.push(s);
  }

  // 世界知识
  let world: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::WorldKnowledge)
    .collect();

  if !world.is_empty() {
    let mut s = String::from("### World Knowledge\n");
    for mem in &world {
      s.push_str(&format!("**{}**\n", mem.header.name));
      s.push_str(&truncate_content(&mem.content, 400));
      s.push_str("\n\n");
    }
    sections.push(s);
  }

  // 活跃计划
  let plans: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::Plan)
    .collect();

  if !plans.is_empty() {
    let mut s = String::from("### Active Plans\n");
    for mem in &plans {
      s.push_str(&format!(
        "- **{}**: {}\n",
        mem.header.name,
        truncate_content(&mem.content, 200)
      ));
    }
    sections.push(s);
  }

  // 最近事件
  let events: Vec<&RecalledMemory> = memories
    .iter()
    .filter(|m| m.header.memory_type == MemoryType::Event)
    .collect();

  if !events.is_empty() {
    let mut s = String::from("### Recent Events\n");
    for mem in events.iter().take(3) {
      s.push_str(&format!(
        "- **{}**: {}\n",
        mem.header.name,
        truncate_content(&mem.content, 150)
      ));
    }
    sections.push(s);
  }

  // 记忆使用指南
  sections.push(build_memory_instructions());

  let body = sections.join("\n---\n\n");
  format!("## Your Memory\n\n{}", body)
}

/// 构建告诉 LLM 如何更新记忆的指令
fn build_memory_instructions() -> String {
  r#"### Memory Instructions

You can save new memories by including a `memory_updates` field in your JSON response. Each update should have:
- `memory_type`: one of "personality", "skill", "world_knowledge", "relationship", "event", "plan"
- `name`: a short descriptive name for the memory
- `content`: the memory content (markdown)
- `importance`: 1-10 (10 = critical)

**When to save:**
- You learn something about a player's preferences or behavior
- You discover an effective technique or strategy
- You find an important location or resource
- A significant event happens (shared victory, near-death, etc.)
- You start or complete a plan

**When NOT to save:**
- Routine observations (current coordinates, nearby blocks)
- Information already in your existing memories
- Temporary tactical decisions"#
    .to_string()
}

/// 截断内容到指定字符数
fn truncate_content(content: &str, max_chars: usize) -> String {
  if content.len() <= max_chars {
    return content.to_string();
  }
  // 在最后一个空格处截断
  let truncated = &content[..max_chars];
  if let Some(last_space) = truncated.rfind(' ') {
    format!("{}...", &truncated[..last_space])
  } else {
    format!("{}...", truncated)
  }
}
