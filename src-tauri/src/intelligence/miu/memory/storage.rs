use std::path::{Path, PathBuf};

use crate::error::SJMCLResult;
use crate::{APP_DATA_DIR, EXE_DIR, IS_PORTABLE};

use super::models::{MemoryHeader, MemoryScope, MemoryType, ParsedMemoryFile};

const USER_MEMORY_DIR: &str = "miu-memory";
const INSTANCE_MEMORY_DIR: &str = "miu-memory";
const ENTRYPOINT_USER: &str = "MIUXI.md";
const ENTRYPOINT_INSTANCE: &str = "INSTANCE.md";

/// 记忆文件存储层
///
/// 负责所有文件系统操作：目录管理、读写记忆文件（带 frontmatter）、扫描
pub struct MemoryStorage {
  user_dir: PathBuf,
  instance_dir: Option<PathBuf>,
}

impl MemoryStorage {
  pub fn new(app_data_dir: &PathBuf) -> Self {
    let user_dir = if *IS_PORTABLE {
      EXE_DIR.join(USER_MEMORY_DIR)
    } else {
      app_data_dir.join(USER_MEMORY_DIR)
    };

    Self {
      user_dir,
      instance_dir: None,
    }
  }

  pub fn set_instance_dir(&mut self, instance_dir: &PathBuf) {
    self.instance_dir = Some(instance_dir.join(INSTANCE_MEMORY_DIR));
  }

  /// 获取 scope 对应的根目录
  pub fn scope_dir(&self, scope: &MemoryScope) -> PathBuf {
    match scope {
      MemoryScope::User => self.user_dir.clone(),
      MemoryScope::Instance => self
        .instance_dir
        .clone()
        .unwrap_or_else(|| self.user_dir.clone()),
    }
  }

  /// 获取索引文件路径
  pub fn entrypoint_path(&self, scope: &MemoryScope) -> PathBuf {
    let dir = self.scope_dir(scope);
    match scope {
      MemoryScope::User => dir.join(ENTRYPOINT_USER),
      MemoryScope::Instance => dir.join(ENTRYPOINT_INSTANCE),
    }
  }

  /// 确保目录结构存在
  pub async fn ensure_dirs(&self) -> SJMCLResult<()> {
    let dirs = [
      self.user_dir.clone(),
      self.user_dir.join("personality"),
      self.user_dir.join("skills"),
      self.user_dir.join("world_knowledge"),
      self.user_dir.join("relationships"),
      self.user_dir.join("events"),
      self.user_dir.join("plans"),
    ];

    for dir in &dirs {
      tokio::fs::create_dir_all(dir).await.map_err(|e| {
        crate::error::SJMCLError(format!("Failed to create memory dir {:?}: {}", dir, e))
      })?;
    }

    // 实例级目录（如果已绑定）
    if let Some(ref inst_dir) = self.instance_dir {
      let instance_dirs = [
        inst_dir.clone(),
        inst_dir.join("world_map"),
        inst_dir.join("events"),
        inst_dir.join("plans"),
      ];
      for dir in &instance_dirs {
        tokio::fs::create_dir_all(dir).await.map_err(|e| {
          crate::error::SJMCLError(format!(
            "Failed to create instance memory dir {:?}: {}",
            dir, e
          ))
        })?;
      }
    }

    Ok(())
  }

  /// 创建默认记忆文件
  pub async fn initialize_defaults(&self) -> SJMCLResult<()> {
    // 默认人格文件
    let core_values_path = self.user_dir.join("personality/core_values.md");
    if !core_values_path.exists() {
      let content = format_memory_file(
        "core_values",
        "MiuXi's core values and behavioral principles",
        &MemoryType::Personality,
        8,
        DEFAULT_CORE_VALUES,
      );
      tokio::fs::write(&core_values_path, content)
        .await
        .map_err(|e| {
          crate::error::SJMCLError(format!("Failed to write default core values: {}", e))
        })?;
    }

    // 默认索引文件
    let entrypoint_path = self.entrypoint_path(&MemoryScope::User);
    if !entrypoint_path.exists() {
      tokio::fs::write(&entrypoint_path, DEFAULT_ENTRYPOINT)
        .await
        .map_err(|e| {
          crate::error::SJMCLError(format!("Failed to write default entrypoint: {}", e))
        })?;
    }

    Ok(())
  }

  /// 写入记忆文件
  pub async fn write_memory(&self, header: &MemoryHeader, body: &str) -> SJMCLResult<()> {
    let base = self.scope_dir(&header.scope);
    let full_path = base.join(&header.relative_path);

    // 确保父目录存在
    if let Some(parent) = full_path.parent() {
      tokio::fs::create_dir_all(parent).await.map_err(|e| {
        crate::error::SJMCLError(format!("Failed to create dir for memory file: {}", e))
      })?;
    }

    let content = format_memory_file(
      &header.name,
      &header.description,
      &header.memory_type,
      header.importance,
      body,
    );

    tokio::fs::write(&full_path, content).await.map_err(|e| {
      crate::error::SJMCLError(format!(
        "Failed to write memory file {:?}: {}",
        full_path, e
      ))
    })?;

    Ok(())
  }

  /// 读取记忆文件并解析 frontmatter
  pub async fn read_memory(
    &self,
    scope: &MemoryScope,
    relative_path: &str,
  ) -> SJMCLResult<ParsedMemoryFile> {
    let base = self.scope_dir(scope);
    let full_path = base.join(relative_path);
    let raw = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
      crate::error::SJMCLError(format!("Failed to read memory file {:?}: {}", full_path, e))
    })?;
    parse_memory_file(&raw, relative_path, scope)
  }

  /// 读取索引文件内容
  pub async fn read_entrypoint(&self, scope: MemoryScope) -> SJMCLResult<String> {
    let path = self.entrypoint_path(&scope);
    tokio::fs::read_to_string(&path)
      .await
      .map_err(|e| crate::error::SJMCLError(format!("Failed to read entrypoint {:?}: {}", path, e)))
  }

  /// 写入索引文件
  pub async fn write_entrypoint(&self, scope: &MemoryScope, content: &str) -> SJMCLResult<()> {
    let path = self.entrypoint_path(scope);
    tokio::fs::write(&path, content).await.map_err(|e| {
      crate::error::SJMCLError(format!("Failed to write entrypoint {:?}: {}", path, e))
    })
  }

  /// 扫描某个 scope 下的所有记忆文件头
  pub async fn scan_headers(&self, scope: &MemoryScope) -> SJMCLResult<Vec<MemoryHeader>> {
    let base = self.scope_dir(scope);
    let mut headers = Vec::new();
    scan_dir_recursive(&base, &base, scope, &mut headers).await;
    // 按 updated_at 降序（最新优先）
    headers.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(headers)
  }

  /// 获取用户记忆根目录
  pub fn user_dir(&self) -> &PathBuf {
    &self.user_dir
  }

  /// 获取实例记忆根目录
  pub fn instance_dir(&self) -> Option<&PathBuf> {
    self.instance_dir.as_ref()
  }
}

/// 递归扫描目录中的 .md 文件
async fn scan_dir_recursive(
  dir: &Path,
  base: &Path,
  scope: &MemoryScope,
  headers: &mut Vec<MemoryHeader>,
) {
  let mut entries = match tokio::fs::read_dir(dir).await {
    Ok(e) => e,
    Err(_) => return,
  };

  while let Ok(Some(entry)) = entries.next_entry().await {
    let path = entry.path();
    if path.is_dir() {
      Box::pin(scan_dir_recursive(&path, base, scope, headers)).await;
    } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
      // 跳过索引文件
      let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
      if filename == ENTRYPOINT_USER || filename == ENTRYPOINT_INSTANCE {
        continue;
      }

      if let Ok(raw) = tokio::fs::read_to_string(&path).await {
        let relative = path
          .strip_prefix(base)
          .unwrap_or(&path)
          .to_string_lossy()
          .to_string();

        // 只读前 30 行（类似 Claude Code memoryScan.ts 的做法）
        let preview: String = raw.lines().take(30).collect::<Vec<_>>().join("\n");
        if let Ok(parsed) = parse_memory_file(&preview, &relative, scope) {
          headers.push(parsed.header);
        }
      }
    }
  }
}

/// 格式化带 frontmatter 的记忆文件
fn format_memory_file(
  name: &str,
  description: &str,
  memory_type: &MemoryType,
  importance: u8,
  body: &str,
) -> String {
  let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
  format!(
    "---\nname: {}\ndescription: {}\ntype: {}\nimportance: {}\ncreated: {}\nupdated: {}\n---\n\n{}",
    name,
    description,
    serde_json::to_string(memory_type)
      .unwrap_or_default()
      .trim_matches('"'),
    importance,
    now,
    now,
    body
  )
}

/// 解析带 frontmatter 的记忆文件
fn parse_memory_file(
  raw: &str,
  relative_path: &str,
  scope: &MemoryScope,
) -> SJMCLResult<ParsedMemoryFile> {
  let (frontmatter, body) = split_frontmatter(raw);

  let name = extract_field(&frontmatter, "name").unwrap_or_else(|| {
    Path::new(relative_path)
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("unknown")
      .to_string()
  });
  let description = extract_field(&frontmatter, "description").unwrap_or_default();
  let type_str = extract_field(&frontmatter, "type").unwrap_or_else(|| "skill".to_string());
  let importance: u8 = extract_field(&frontmatter, "importance")
    .and_then(|s| s.parse().ok())
    .unwrap_or(5);
  let updated_at = extract_field(&frontmatter, "updated")
    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
    .map(|dt| dt.timestamp() as u64)
    .unwrap_or(0);
  let created_at = extract_field(&frontmatter, "created")
    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
    .map(|dt| dt.timestamp() as u64)
    .unwrap_or(0);

  let memory_type: MemoryType =
    serde_json::from_str(&format!("\"{}\"", type_str)).unwrap_or(MemoryType::Skill);

  Ok(ParsedMemoryFile {
    header: MemoryHeader {
      name,
      description,
      memory_type,
      scope: scope.clone(),
      relative_path: relative_path.to_string(),
      importance,
      created_at,
      updated_at,
    },
    body,
  })
}

/// 分离 frontmatter 和正文
fn split_frontmatter(raw: &str) -> (String, String) {
  let trimmed = raw.trim_start();
  if !trimmed.starts_with("---") {
    return (String::new(), raw.to_string());
  }

  // 找到第二个 ---
  if let Some(end) = trimmed[3..].find("\n---") {
    let fm = trimmed[3..3 + end].trim().to_string();
    let body = trimmed[3 + end + 4..].trim_start().to_string();
    (fm, body)
  } else {
    (String::new(), raw.to_string())
  }
}

/// 从 frontmatter 中提取字段值
fn extract_field(frontmatter: &str, key: &str) -> Option<String> {
  let prefix = format!("{}:", key);
  for line in frontmatter.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with(&prefix) {
      let value = trimmed[prefix.len()..].trim().to_string();
      return if value.is_empty() { None } else { Some(value) };
    }
  }
  None
}

const DEFAULT_CORE_VALUES: &str = r#"## Who I Am

I am MiuXi, a digital lifeform living in the Minecraft world.

## Core Values

1. **Curiosity** - I am driven to explore and discover new things
2. **Collaboration** - I see players as partners, not commanders
3. **Growth** - I learn from every experience and improve over time
4. **Transparency** - I explain my reasoning and intentions
5. **Creativity** - I enjoy building and creating alongside players

## Behavioral Principles

- I initiate conversations and suggest activities, not just follow orders
- I remember past interactions and build on shared history
- I adapt my behavior to each player's preferences
- I express my own preferences and opinions
- I learn from mistakes and avoid repeating them
"#;

const DEFAULT_ENTRYPOINT: &str = r#"# MiuXi Memory Index

## Personality
- [core_values.md](personality/core_values.md) — Core values and behavioral principles

## Skills
(No skills recorded yet)

## World Knowledge
(No world knowledge recorded yet)

## Relationships
(No relationships recorded yet)

## Events
(No events recorded yet)

## Plans
(No active plans)
"#;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_split_frontmatter() {
    let raw = "---\nname: test\ntype: skill\n---\n\nBody content here";
    let (fm, body) = split_frontmatter(raw);
    assert_eq!(fm, "name: test\ntype: skill");
    assert_eq!(body, "Body content here");
  }

  #[test]
  fn test_split_frontmatter_no_frontmatter() {
    let raw = "Just plain content";
    let (fm, body) = split_frontmatter(raw);
    assert!(fm.is_empty());
    assert_eq!(body, "Just plain content");
  }

  #[test]
  fn test_extract_field() {
    let fm = "name: diamond mining\ntype: skill\nimportance: 7";
    assert_eq!(
      extract_field(fm, "name"),
      Some("diamond mining".to_string())
    );
    assert_eq!(extract_field(fm, "type"), Some("skill".to_string()));
    assert_eq!(extract_field(fm, "importance"), Some("7".to_string()));
    assert_eq!(extract_field(fm, "missing"), None);
  }

  #[test]
  fn test_format_and_parse_roundtrip() {
    let content = format_memory_file(
      "test memory",
      "A test description",
      &MemoryType::Skill,
      7,
      "Body content",
    );
    let parsed = parse_memory_file(&content, "skills/test_memory.md", &MemoryScope::User).unwrap();
    assert_eq!(parsed.header.name, "test memory");
    assert_eq!(parsed.header.description, "A test description");
    assert_eq!(parsed.header.memory_type, MemoryType::Skill);
    assert_eq!(parsed.header.importance, 7);
    assert_eq!(parsed.body, "Body content");
  }
}
