# MiuXi Memory System + Capability Protocol 实现计划

## 一、目标

1. **Memory 系统**：为 MiuXi bot 构建分层记忆系统，参考 Claude Code 的 memdir 架构
2. **Capability 协议**：将硬编码的 4 个动作（Move/Mine/Attack/Wait）重构为可扩展的能力协议，并新增更多 azalea API 支持的动作

## 二、代码结构

现有 intelligence 模块结构：
```
src-tauri/src/intelligence/
├── mod.rs
├── models.rs          # LLM API types + ChatHistory
├── commands.rs        # Tauri commands
├── providers.rs       # LLM provider implementations
└── azalea_bot/
    ├── mod.rs
    ├── bot.rs          # 当前全部 bot 逻辑
    ├── constants.rs
    └── models.rs       # BotState, AgentDecision, ActionType
```

重构后的结构：
```
src-tauri/src/intelligence/
├── mod.rs                          # 新增 miu 模块声明
├── models.rs                       # 不变
├── commands.rs                     # 新增 memory 相关 commands
├── providers.rs                    # 不变
├── azalea_bot/                     # 保留现有结构，逐步迁移
│   ├── mod.rs
│   ├── bot.rs                      # 改造：使用 capability pipeline
│   ├── constants.rs
│   └── models.rs                   # 精简：移除 ActionType（迁移到 capability）
│
└── miu/                            # 新增：MiuXi 核心模块
    ├── mod.rs
    │
    ├── memory/                     # 记忆系统
    │   ├── mod.rs                  # 公共接口 MemorySystem
    │   ├── models.rs               # MemoryScope, MemoryType, RecalledMemory 等
    │   ├── storage.rs              # 文件读写、目录管理
    │   ├── indexing.rs             # 索引文件（MIUXI.md）维护
    │   ├── recall.rs               # 记忆召回（相关性筛选）
    │   └── prompt_builder.rs       # 构建注入 LLM prompt 的记忆上下文
    │
    └── capabilities/               # 能力系统
        ├── mod.rs                  # Capability trait + Registry + Pipeline
        ├── models.rs               # CapabilitySpec, CapabilityResult, CostEstimate 等
        ├── registry.rs             # CapabilityRegistry（注册/查找）
        ├── pipeline.rs             # 执行管线（校验→执行→记忆更新）
        ├── prompt_builder.rs       # 将能力列表转换为 LLM tool 描述
        │
        ├── movement.rs             # goto, follow, explore
        ├── mining.rs               # mine, mine_with_auto_tool
        ├── combat.rs               # attack, flee
        ├── interaction.rs          # block_interact, entity_interact, use_item
        ├── inventory.rs            # open_container, select_slot, get_held_item
        ├── chat.rs                 # chat, send_command
        └── observation.rs          # 等待/观察（替代 Wait）
```

## 三、实现步骤

### Phase 1: Memory 系统基础设施

#### 1.1 创建 `miu/memory/models.rs` — 数据结构

```rust
// 记忆存储层级
pub enum MemoryScope {
    /// 用户级：跨世界、跨实例，存于 ~/.sjmcl/miu-memory/
    User,
    /// 实例级：当前世界，存于实例目录下
    Instance(String), // instance_id
}

// 记忆分类
pub enum MemoryType {
    Personality,     // 核心人格与偏好
    Skill,           // 技能与经验
    WorldKnowledge,  // 世界知识
    Relationship,    // 玩家关系
    Event,           // 重要事件
    Plan,            // 进行中的计划
}

// 记忆文件头（用于索引和召回）
pub struct MemoryHeader {
    pub name: String,
    pub description: String,
    pub memory_type: MemoryType,
    pub scope: MemoryScope,
    pub file_path: PathBuf,
    pub updated_at: u64,
    pub importance: u8, // 1-10
}

// 召回的记忆
pub struct RecalledMemory {
    pub header: MemoryHeader,
    pub content: String,
}

// 记忆更新请求（LLM 输出的一部分）
pub struct MemoryUpdateRequest {
    pub memory_type: MemoryType,
    pub target_file: Option<String>,
    pub content: String,
    pub importance: u8,
}

// 召回上下文（用于判断相关性）
pub struct RecallContext {
    pub current_biome: Option<String>,
    pub nearby_players: Vec<String>,
    pub nearby_blocks: Vec<String>,
    pub nearby_entities: Vec<String>,
    pub current_activity: Option<String>,
}
```

#### 1.2 创建 `miu/memory/storage.rs` — 文件系统操作

核心功能：
- `ensure_memory_dirs()` — 确保目录结构存在
- `get_user_memory_dir()` — `~/.sjmcl/miu-memory/` 或便携模式下的等价路径
- `get_instance_memory_dir(instance_id)` — 实例级记忆路径
- `write_memory_file(scope, memory_type, name, content)` — 写入记忆文件（带 frontmatter）
- `read_memory_file(path)` — 读取并解析 frontmatter
- `scan_memory_files(dir)` — 扫描目录下所有 .md 文件的 header
- `delete_memory_file(path)` — 删除记忆

存储格式参考 Claude Code：
```markdown
---
name: diamond_mining_at_y54
description: Y=-54 层是钻石矿最佳挖掘高度
type: skill
importance: 8
created: 2026-04-08T12:00:00Z
updated: 2026-04-08T12:00:00Z
---

## 钻石矿挖掘经验

在 Y=-54 层进行分支挖矿效率最高...
```

使用项目现有的 `APP_DATA_DIR`（非便携）/ `EXE_DIR`（便携模式）逻辑，与 `Storage` trait 对齐。

#### 1.3 创建 `miu/memory/indexing.rs` — 索引维护

核心功能：
- `load_entrypoint(scope)` — 读取 MIUXI.md 或 INSTANCE.md
- `update_entrypoint(scope, entry)` — 添加/更新索引条目
- `remove_from_entrypoint(scope, name)` — 移除索引条目
- `truncate_entrypoint(content)` — 截断保护（参照 Claude Code 的 200 行 / 25KB 限制）

索引文件格式：
```markdown
# MiuXi Memory

## Personality
- [core_values.md](personality/core_values.md) — 核心价值观和行为原则
- [learned_preferences.md](personality/learned_preferences.md) — 从互动中学到的偏好

## Skills
- [diamond_mining.md](skills/mining/diamond_mining.md) — Y=-54 层钻石挖掘策略

## Relationships
- [Kilox](relationships/kilox/profile.md) — 主要合作伙伴，偏好建造
```

#### 1.4 创建 `miu/memory/recall.rs` — 记忆召回

核心功能：
- `recall_relevant(context, limit)` — 基于当前上下文召回最相关的记忆
- `recall_by_type(memory_type, limit)` — 按类型召回
- `recall_player(player_name)` — 召回特定玩家的关系记忆

相关性评分策略（不使用 LLM 做 sidequery，而是基于规则）：
1. 如果附近有玩家 → 高优召回该玩家的 relationship 记忆
2. 如果在特定生物群系 → 召回该生物群系的 world_knowledge
3. 如果附近有危险实体 → 召回 combat skill 记忆
4. 如果有活跃计划 → 召回 plan 记忆
5. 按 importance × recency 排序

#### 1.5 创建 `miu/memory/prompt_builder.rs` — Prompt 注入

核心功能：
- `build_memory_prompt(recalled_memories, world_state)` — 构建注入系统 prompt 的记忆文本
- `build_memory_update_instructions()` — 告诉 LLM 如何输出 memory_updates

输出格式：
```
## Your Memory

### Who You Are
{personality/core_values.md 内容摘要}

### Relevant Skills
{根据当前情境召回的技能记忆}

### People Nearby
{如果附近有已知玩家，注入关系记忆}

### World Knowledge
{当前区域相关的世界知识}

### Active Plans
{进行中的计划}
```

#### 1.6 创建 `miu/memory/mod.rs` — MemorySystem 主接口

```rust
pub struct MemorySystem {
    user_dir: PathBuf,
    instance_dir: Option<PathBuf>,
}

impl MemorySystem {
    pub fn new(app_handle: &AppHandle) -> Self;
    pub fn set_instance(&mut self, instance_id: &str);
    
    // 写入
    pub async fn write(&self, scope, memory_type, name, content, importance) -> Result<()>;
    
    // 召回
    pub async fn recall(&self, context: &RecallContext, limit: usize) -> Result<Vec<RecalledMemory>>;
    
    // 构建 prompt
    pub async fn build_prompt(&self, world_state: &WorldState) -> Result<String>;
    
    // 处理 LLM 输出的记忆更新
    pub async fn apply_updates(&self, updates: Vec<MemoryUpdateRequest>) -> Result<()>;
    
    // 初始化默认记忆
    pub async fn initialize_defaults(&self) -> Result<()>;
}
```

### Phase 2: Capability 协议

#### 2.1 创建 `miu/capabilities/models.rs` — 类型定义

```rust
pub struct CapabilityId(pub String); // "movement:goto"

pub struct CapabilitySpec {
    pub id: CapabilityId,
    pub name: String,
    pub description: String,       // LLM 可见
    pub parameters: Value,         // JSON Schema
    pub examples: Vec<Example>,
}

pub struct CapabilityResult {
    pub success: bool,
    pub message: String,
    pub data: Option<Value>,
    pub memory_updates: Vec<MemoryUpdateRequest>,
}

pub struct CostEstimate {
    pub duration_seconds: u32,
    pub risk_level: RiskLevel,     // Low / Medium / High
}

pub enum RiskLevel { Low, Medium, High }

pub enum PreconditionResult {
    Ok,
    Warning(String),
    Fail(String),
}
```

#### 2.2 创建 `miu/capabilities/mod.rs` — Capability trait

```rust
#[async_trait]
pub trait Capability: Send + Sync {
    fn spec(&self) -> CapabilitySpec;
    fn check_preconditions(&self, ctx: &GameContext, input: &Value) -> PreconditionResult;
    fn estimate_cost(&self, ctx: &GameContext, input: &Value) -> CostEstimate;
    async fn execute(&self, ctx: &mut GameContext, input: Value) -> Result<CapabilityResult, CapabilityError>;
    fn is_interruptible(&self) -> bool { false }
}

pub struct GameContext {
    pub bot: Client,
    pub memory: Arc<MemorySystem>,
    pub app_handle: AppHandle,
}
```

#### 2.3 创建 `miu/capabilities/registry.rs`

```rust
pub struct CapabilityRegistry {
    capabilities: HashMap<CapabilityId, Box<dyn Capability>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, capability: Box<dyn Capability>);
    pub fn get(&self, id: &CapabilityId) -> Option<&dyn Capability>;
    pub fn list(&self) -> Vec<&dyn Capability>;
    pub fn build_tool_descriptions(&self) -> Vec<Value>; // 用于 LLM function calling
}

pub fn register_all_capabilities() -> CapabilityRegistry;
```

#### 2.4 创建 `miu/capabilities/pipeline.rs` — 执行管线

```rust
pub struct CapabilityPipeline {
    registry: Arc<CapabilityRegistry>,
    memory: Arc<MemorySystem>,
}

impl CapabilityPipeline {
    pub async fn execute(
        &self,
        capability_id: &str,
        input: Value,
        ctx: &mut GameContext,
    ) -> Result<CapabilityResult, PipelineError> {
        // 1. 查找能力
        // 2. 前置条件检查
        // 3. 执行
        // 4. 处理记忆更新
        // 5. 返回结果
    }
}
```

#### 2.5 实现具体能力（覆盖所有可用 azalea API）

**movement.rs** — 移动类
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `movement:goto` | `bot.start_goto(BlockPosGoal)` | 寻路到坐标 |
| `movement:goto_xz` | `bot.start_goto(XZGoal)` | 寻路到 XZ（忽略 Y）|
| `movement:follow` | `bot.start_goto(RadiusGoal)` + 循环 | 跟随实体 |
| `movement:sprint` | `bot.sprint(direction)` | 冲刺移动 |
| `movement:look_at` | `bot.look_at(pos)` | 转向看某位置 |
| `movement:stop` | `bot.force_stop_pathfinding()` | 停止移动 |

**mining.rs** — 挖掘类
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `mining:mine_block` | `bot.mine_with_auto_tool(pos)` | 自动选工具挖掘 |
| `mining:mine_at` | `bot.start_mining(pos)` | 开始挖掘（非阻塞） |

**combat.rs** — 战斗类
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `combat:attack` | `bot.look_at() + bot.attack()` | 攻击实体 |
| `combat:flee` | `bot.start_goto(InverseGoal)` 或反向跑 | 逃离 |

**interaction.rs** — 交互类
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `interaction:use_block` | `bot.block_interact(pos)` | 右键方块 |
| `interaction:use_entity` | `bot.entity_interact(entity)` | 右键实体 |
| `interaction:use_item` | `bot.start_use_item()` | 使用手持物品 |
| `interaction:place_block` | `bot.block_interact(pos)` | 放置方块 |

**inventory.rs** — 物品管理
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `inventory:select_slot` | `bot.set_selected_hotbar_slot(n)` | 切换快捷栏 |
| `inventory:open_container` | `bot.open_container_at(pos)` | 打开容器 |
| `inventory:check_inventory` | `bot.menu()` | 查看背包 |
| `inventory:get_held_item` | `bot.get_held_item()` | 查看手持物品 |

**chat.rs** — 聊天
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `chat:send_message` | `bot.chat(msg)` | 发送聊天消息 |
| `chat:send_command` | `bot.write_command_packet(cmd)` | 执行服务器命令 |

**observation.rs** — 观察
| 能力 ID | azalea API | 描述 |
|---------|-----------|------|
| `observe:wait` | `bot.wait_ticks(n)` | 等待 N tick |
| `observe:scan_area` | `perceive_world_state()` | 扫描周围环境 |
| `observe:check_health` | `bot.health() + bot.hunger()` | 查看生命值和饥饿度 |

#### 2.6 创建 `miu/capabilities/prompt_builder.rs`

将 CapabilityRegistry 中的所有能力转换为 LLM 可理解的 function/tool 描述，注入 prompt。

### Phase 3: 集成改造

#### 3.1 改造 `azalea_bot/bot.rs`

- `perceive_world_state()` → 增强：加入生命值、饥饿度、背包信息、生物群系
- `query_llm_decision()` → 重写：
  - 注入记忆上下文（调用 MemorySystem.build_prompt）
  - 注入能力列表（调用 CapabilityRegistry.build_tool_descriptions）
  - 改 prompt 格式为 function calling 或结构化输出
- `execute_action()` → 替换为 CapabilityPipeline.execute()
- `handle_events()` → 增强：处理更多 Event 类型（Death, Disconnect, Chat 互动）

#### 3.2 改造 `azalea_bot/models.rs`

- 移除 `ActionType` 枚举（被 Capability 替代）
- 移除 `AgentDecision`（被新的 LLM 输出格式替代）
- 保留 `BotState`，扩展字段：
  ```rust
  pub struct BotState {
      pub client: Arc<tokio::sync::Mutex<Option<Client>>>,
      pub app_handle: Option<AppHandle>,
      pub exit_notified: Arc<AtomicBool>,
      pub last_action_time: Arc<std::sync::Mutex<Instant>>,
      pub cooldown: Duration,
      // 新增
      pub memory: Option<Arc<MemorySystem>>,
      pub capabilities: Option<Arc<CapabilityRegistry>>,
  }
  ```

#### 3.3 注册到 `lib.rs`

- 在 `setup()` 中初始化 MemorySystem，创建默认记忆
- 将 MemorySystem 注入 BotState 或单独管理为 Tauri State

#### 3.4 新增 Tauri commands

```rust
// commands.rs 新增
pub fn retrieve_bot_memories(app, scope) -> Vec<MemoryHeader>;
pub fn retrieve_bot_memory_content(app, path) -> String;
pub fn update_bot_memory(app, path, content) -> ();
pub fn delete_bot_memory(app, path) -> ();
pub fn retrieve_bot_capabilities(app) -> Vec<CapabilitySpec>;
```

### Phase 4: LLM 决策格式升级

将 LLM 输出从当前的：
```json
{"thought": "...", "action": "move", "target_coords": {"x":1,"y":2,"z":3}}
```

升级为：
```json
{
  "reflection": "基于记忆，我认为...",
  "capability": "movement:goto",
  "parameters": {"x": 100, "z": 200},
  "memory_updates": [
    {
      "type": "skill",
      "target": "mining/diamond_strategy.md",
      "content": "在 Y=-54 发现高效钻石矿脉",
      "importance": 7
    }
  ],
  "speak": "我发现了一个有趣的洞穴！"
}
```

## 四、实现顺序

按依赖关系排列：

1. `miu/mod.rs` + `miu/memory/mod.rs` + `miu/memory/models.rs` — 模块骨架
2. `miu/memory/storage.rs` — 文件存储实现
3. `miu/memory/indexing.rs` — 索引维护
4. `miu/memory/recall.rs` — 记忆召回
5. `miu/memory/prompt_builder.rs` — Prompt 构建
6. `miu/capabilities/models.rs` + `miu/capabilities/mod.rs` — Capability trait
7. `miu/capabilities/registry.rs` — 注册表
8. `miu/capabilities/pipeline.rs` — 执行管线
9. 具体能力实现（movement → mining → combat → interaction → inventory → chat → observation）
10. `miu/capabilities/prompt_builder.rs` — 能力 prompt 描述
11. 改造 `azalea_bot/bot.rs` — 集成 memory + capability
12. 改造 `azalea_bot/models.rs` — 更新类型
13. 更新 `commands.rs` — 新增 Tauri commands
14. 更新 `lib.rs` — 注册新 state
15. 创建默认记忆文件（personality/core_values.md）
