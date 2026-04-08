pub mod indexing;
pub mod models;
pub mod prompt_builder;
pub mod recall;
pub mod storage;

use std::path::PathBuf;
use std::sync::Arc;

use models::{
  MemoryHeader, MemoryScope, MemoryType, MemoryUpdateRequest, RecallContext, RecalledMemory,
};
use storage::MemoryStorage;

use crate::error::SJMCLResult;

/// MiuXi 记忆系统
///
/// 分层架构（参照 Claude Code memdir）：
/// - User scope: ~/.sjmcl/miu-memory/ — 跨世界、跨实例的长期记忆
/// - Instance scope: <instance_dir>/miu-memory/ — 当前世界的实例级记忆
pub struct MemorySystem {
  storage: MemoryStorage,
  instance_id: Option<String>,
}

impl MemorySystem {
  /// 创建记忆系统实例
  pub fn new(app_data_dir: &PathBuf) -> Self {
    Self {
      storage: MemoryStorage::new(app_data_dir),
      instance_id: None,
    }
  }

  /// 绑定当前游戏实例
  pub fn set_instance(&mut self, instance_id: &str, instance_dir: &PathBuf) {
    self.instance_id = Some(instance_id.to_string());
    self.storage.set_instance_dir(instance_dir);
  }

  /// 初始化目录结构和默认记忆
  pub async fn initialize(&self) -> SJMCLResult<()> {
    self.storage.ensure_dirs().await?;
    self.storage.initialize_defaults().await?;
    Ok(())
  }

  /// 写入记忆
  pub async fn write(
    &self,
    scope: MemoryScope,
    memory_type: MemoryType,
    name: &str,
    content: &str,
    importance: u8,
  ) -> SJMCLResult<()> {
    let header = MemoryHeader::new(name, memory_type, scope, importance);
    self.storage.write_memory(&header, content).await?;
    indexing::update_entrypoint(&self.storage, &header).await?;
    Ok(())
  }

  /// 基于上下文召回相关记忆
  pub async fn recall(
    &self,
    context: &RecallContext,
    limit: usize,
  ) -> SJMCLResult<Vec<RecalledMemory>> {
    recall::recall_relevant(&self.storage, context, limit).await
  }

  /// 构建注入 LLM prompt 的记忆上下文
  pub async fn build_prompt(&self, context: &RecallContext) -> SJMCLResult<String> {
    let recalled = self.recall(context, 8).await?;
    let entrypoint = self
      .storage
      .read_entrypoint(MemoryScope::User)
      .await
      .unwrap_or_default();
    Ok(prompt_builder::build_memory_prompt(&recalled, &entrypoint))
  }

  /// 处理 LLM 输出的记忆更新请求
  pub async fn apply_updates(&self, updates: Vec<MemoryUpdateRequest>) -> SJMCLResult<()> {
    for update in updates {
      let scope = update.scope.unwrap_or(MemoryScope::User);
      self
        .write(
          scope,
          update.memory_type,
          &update.name,
          &update.content,
          update.importance,
        )
        .await?;
    }
    Ok(())
  }
}
