<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — 主权智能运行时 (Sovereign Intelligence Runtime)

**超压缩、架构主权的 Rust 智能运行时、双重包裹空间动作引擎与原生 macOS AppKit IDE (`HEAV3NS-CODE`)** — 遵循 **HEAV3NS-JOY** 5 域 Joyzoning 架构（`100%` 稳定性指数，严格零违规，全工作区文件均处于 Tier 1 Optimal `< 800` 行之内）。

<p align="center">
  <a href="../../doc.md">主权宪章 (doc.md)</a> ·
  <a href="../../AGENTS.md">智能体协议 (AGENTS.md)</a> ·
  <a href="../../SOUL.md">核心心智 (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">演进路线</a> ·
  <a href="../README.md">文档中心</a> ·
  <a href="../papers/ascendant-intelligence.md">飞升智能白皮书</a> ·
  <a href="../PROJECT_MAP.md">工程拓扑</a> ·
  <a href="../CONSTITUTION.md">宪法级约束</a> ·
  <a href="../../VALIDATION_REPORT.md">验证报告</a> ·
  <a href="../../native/PRODUCT.md">原生产品规范</a>
</p>

<p align="center">
  <a href="../../README.md">English</a> ·
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.ja-JP.md">日本語</a> ·
  <a href="README.vi.md">Tiếng Việt</a> ·
  <a href="README.id.md">Bahasa Indonesia</a> ·
  <a href="README.ko-KR.md">한국어</a> ·
  <a href="README.es-419.md">Español</a> ·
  <a href="README.pt-BR.md">Português</a> ·
  <a href="README.ru.md">Русский</a> ·
  <a href="README.uk.md">Українська</a> ·
  <a href="README.fr.md">Français</a> ·
  <a href="README.de.md">Deutsch</a> ·
  <a href="README.zh-TW.md">繁體中文</a> ·
  <a href="README.hi.md">हिन्दी</a> ·
  <a href="README.tr.md">Türkçe</a> ·
  <a href="README.it.md">Italiano</a> ·
  <a href="README.pl.md">Polski</a> ·
  <a href="README.ar.md">العربية</a> ·
  <a href="README.ca.md">Català</a>
</p>

---

## 七大主权支柱 (The Seven Sovereign Pillars)

> *"不是单个智能体。不是蜂群。不是操作系统。这是一个相互协作的计算文明智能体生态 — 系统不仅执行指令，更能理解环境、学习结构、铭记经验并带着确凿意图行动。"* — [`doc.md`](../../doc.md)

| 支柱 | 命名 | 核心实现 |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **精炼智能运行时** | [`AgentEngine`](../../src/core/engine.rs) 多轮 ReAct 状态机、KV 缓存确定性 [`SystemPromptSpec`](../../src/domain/prompt_spec.rs)、[`ContextInspectionSnapshot`](../../src/core/context_inspector.rs) 上下文与预算检测、三阶段自愈式 [`ToolCallArgParser`](../../src/domain/tool_parser.rs) 以及 `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs)。 |
| **2. `SPIDER`** | **结构感知与环境理解** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) 结构关系分析与可供性分类、[`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) 环境观察以及 `spider.check` 严格 AST 审计。 |
| **3. `REWIND`** | **时空认知、连续性与确定性恢复** | 16-MiB [`ArenaAllocator`](../../src/domain/arena.rs) 与 $O(1)$ 快照重退、纯 Rust SHA-256 [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs)（具备 `WouldBlock` 保护的 `/undo`、`/restore` 与非 Git `/diff`）、两阶段 [`SessionVfs`](../../src/infrastructure/vfs_store.rs) 基线比对校验、[`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) 回滚机制，以及 32 深度 [`VirtualDisplayState`](../../src/domain/computer_use.rs) 撤销/重做栈。 |
| **4. `BROCCOLI`** | **持久化知识、记忆与历史演进** | 带拉普拉斯平滑校准置信度的 [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs)、混合式 TF-IDF [`MemoryStore`](../../src/domain/memory.rs)，以及基于 Kahn 拓扑排序校验的 [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) 有向无环图。 |
| **5. `SYNTHETIC SWARM`** | **分布式集体智能** | 5 大专家并行协商（视觉感知者、结构分析者、工作流领航者、风险评估者、恢复专家 + 冲突解决仲裁者）以及 7 专家 [`Legion`](../../src/domain/swarm.rs) 拓扑阵型（包含独占文件保留与确定性任务交接）。 |
| **6. `COMPUTER USE`** | **具身自主：观察、意图解释、行动与自适应** | 9 阶段闭环 [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs)、双重包裹 [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs)（Wrapper A 空间锚定 + Wrapper B 确定性保障）、[`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) 视口外自动滚动，以及零信任的 `browser_use`。 |
| **7. `THE HARNESS`** | **贯穿一切的底层基质体系** | 31 个原生工具（含 8 槽 LRU 缓存的 `tool_search` 与 32 步批量执行 `execute_tools`）、DAG 波阵面并发调度器、5 级递进的 [`ExecutionPosture`](../../src/domain/types/posture_types.rs)、macOS Seatbelt 沙箱、完整 [`TurnReceipt`](../../src/domain/turn_receipt.rs) 审计凭证，以及多路复用 JSON-RPC stdio 服务与原生 macOS AppKit 客户端。 |

---

## 5 域 Joyzoning 架构 (`82` 个 Rust 模块 · `300+` 个工作区文件)

代码库中每个源文件均归属于 5 个经过严格验证的架构域之一：

| 领域 | 物理路径 | 核心职责与架构约束 |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` 个模块) | 无状态叶子函数与基础原语：哈希与数学、文本标记预算、JSON 自愈修复、确定性密钥脱敏、Seatbelt 沙箱配置、Shell 防护管道、模糊补丁引擎、空间几何及环境感知分析。**绝对零向上引用、零可变全局状态。** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` 个模块) | 纯业务实体与 64 个正式 `I<Name>` 接口特征、执行姿态、模型定价名录、系统提示词规范、Arena 内存分配器、技能树目录、记忆存储、蜂群调度模型。**严格 0% I/O 交互，单文件不超过 3 个结构体。** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` 个模块) | 实体 I/O 适配层：HTTP 与 61 家模型提供商线协议适配（OpenAI、Anthropic、Gemini、Codestral 等）、两阶段 VFS 存储、SHA-256 CAS 工作区快照、行为经验库与沙箱 Shell 执行器。 |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` 个模块) | 用例编排层：`AgentEngine` 核心状态机、波阵面工具调度、空间交互引擎、上下文审查器、运行队列与自测飞行套件。 |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs)、[`native/`](../../native/) | 表现与用户意图层：CLI 呈现、stdio JSON-RPC 多路工作站、原生 macOS AppKit 桌面应用及程序入口组装点。 |

---

## 快速上手与验证 (Quick Start)

```bash
# 1. 运行全量 Rust 单元测试、空间引擎、具身用例及集成测试
cargo test

# 2. 运行原生 macOS AppKit 测试套件
cd native && swift test && cd ..

# 3. 运行自主飞行验证与自检诊断
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. 运行严格 Joyzoning 架构审计与文档检查
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
