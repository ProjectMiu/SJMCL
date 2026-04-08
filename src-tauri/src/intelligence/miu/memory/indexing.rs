use super::models::{MemoryHeader, MemoryScope};
use super::storage::MemoryStorage;
use crate::error::SJMCLResult;

/// 索引文件最大行数（参照 Claude Code MAX_ENTRYPOINT_LINES = 200）
const MAX_ENTRYPOINT_LINES: usize = 200;
/// 索引文件最大字节数（参照 Claude Code MAX_ENTRYPOINT_BYTES = 25_000）
const MAX_ENTRYPOINT_BYTES: usize = 25_000;

/// 更新索引文件，添加或更新一条记忆的条目
pub async fn update_entrypoint(storage: &MemoryStorage, header: &MemoryHeader) -> SJMCLResult<()> {
  let scope = &header.scope;
  let mut content = storage
    .read_entrypoint(scope.clone())
    .await
    .unwrap_or_default();

  let entry_line = format_entry_line(header);

  // 查找是否已有同路径的条目
  let path_marker = format!("[{}]", header.relative_path);
  let alt_marker = format!("({})", header.relative_path);

  let mut found = false;
  let updated_lines: Vec<String> = content
    .lines()
    .map(|line| {
      if line.contains(&path_marker) || line.contains(&alt_marker) {
        found = true;
        entry_line.clone()
      } else {
        line.to_string()
      }
    })
    .collect();

  if found {
    content = updated_lines.join("\n");
  } else {
    // 添加到对应 section
    let section_header = format!("## {}", header.memory_type.display_name());
    let placeholder_patterns = [
      "(No skills recorded yet)",
      "(No world knowledge recorded yet)",
      "(No relationships recorded yet)",
      "(No events recorded yet)",
      "(No active plans)",
      "(No personality recorded yet)",
    ];

    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut inserted = false;

    for (i, line) in lines.iter().enumerate() {
      // 移除 placeholder 并插入
      if placeholder_patterns.iter().any(|p| line.contains(p)) {
        let section_line = lines[..i]
          .iter()
          .rposition(|l| l.starts_with("## "))
          .map(|idx| &lines[idx]);

        if let Some(sl) = section_line {
          if sl.contains(header.memory_type.display_name()) {
            lines[i] = entry_line.clone();
            inserted = true;
            break;
          }
        }
      }
    }

    if !inserted {
      // 找到 section header 后插入
      if let Some(idx) = lines.iter().position(|l| l.trim() == section_header) {
        // 找到 section 末尾（下一个 ## 或文件结束）
        let insert_at = lines[idx + 1..]
          .iter()
          .position(|l| l.starts_with("## "))
          .map(|p| idx + 1 + p)
          .unwrap_or(lines.len());
        lines.insert(insert_at, entry_line);
      } else {
        // section 不存在，在末尾添加
        lines.push(String::new());
        lines.push(section_header);
        lines.push(entry_line);
      }
    }

    content = lines.join("\n");
  }

  // 截断保护
  content = truncate_entrypoint(&content);

  storage.write_entrypoint(scope, &content).await
}

/// 从索引文件中移除一条记忆
pub async fn remove_from_entrypoint(
  storage: &MemoryStorage,
  scope: &MemoryScope,
  relative_path: &str,
) -> SJMCLResult<()> {
  let content = storage
    .read_entrypoint(scope.clone())
    .await
    .unwrap_or_default();

  let path_marker = format!("[{}]", relative_path);
  let alt_marker = format!("({})", relative_path);

  let filtered: Vec<&str> = content
    .lines()
    .filter(|line| !line.contains(&path_marker) && !line.contains(&alt_marker))
    .collect();

  storage.write_entrypoint(scope, &filtered.join("\n")).await
}

/// 截断索引内容（参照 Claude Code truncateEntrypointContent）
fn truncate_entrypoint(content: &str) -> String {
  let lines: Vec<&str> = content.lines().collect();

  // 先按行数截断
  let line_truncated: Vec<&str> = if lines.len() > MAX_ENTRYPOINT_LINES {
    lines[..MAX_ENTRYPOINT_LINES].to_vec()
  } else {
    lines
  };

  let mut result = line_truncated.join("\n");

  // 再按字节截断
  if result.len() > MAX_ENTRYPOINT_BYTES {
    // 在最后一个换行符处截断，避免切断中间
    result = result[..MAX_ENTRYPOINT_BYTES].to_string();
    if let Some(last_nl) = result.rfind('\n') {
      result = result[..last_nl].to_string();
    }
  }

  result
}

/// 格式化一条索引条目
fn format_entry_line(header: &MemoryHeader) -> String {
  let desc = if header.description.is_empty() {
    header.name.clone()
  } else {
    header.description.clone()
  };
  // 截断描述到 120 字符
  let short_desc = if desc.len() > 120 {
    format!("{}...", &desc[..117])
  } else {
    desc
  };
  format!(
    "- [{}]({}) — {}",
    header.name, header.relative_path, short_desc
  )
}
