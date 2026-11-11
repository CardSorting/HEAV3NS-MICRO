<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — 主權智能運行時 (Sovereign Intelligence Runtime)

**超壓縮、架構主權的 Rust 智能運行時、雙重包裹空間動作引擎與原生 macOS AppKit IDE (`HEAV3NS-CODE`)** — 遵循 **HEAV3NS-JOY** 5 域 Joyzoning 架構（`100%` 穩定性指數，嚴格零違規，全工作區檔案均處於 Tier 1 Optimal `< 800` 行之內）。

<p align="center">
  <a href="../../doc.md">主權憲章 (doc.md)</a> ·
  <a href="../../AGENTS.md">智能體協議 (AGENTS.md)</a> ·
  <a href="../../SOUL.md">核心心智 (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">演進路線</a> ·
  <a href="../README.md">文檔中心</a> ·
  <a href="../papers/ascendant-intelligence.md">飛升智能白皮書</a> ·
  <a href="../PROJECT_MAP.md">工程拓撲</a> ·
  <a href="../CONSTITUTION.md">憲法級約束</a> ·
  <a href="../../VALIDATION_REPORT.md">驗證報告</a> ·
  <a href="../../native/PRODUCT.md">原生產品規範</a>
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

## 七大主權支柱 (The Seven Sovereign Pillars)

> *"不是單個智能體。不是蜂群。不是操作系統。這是一個相互協作的計算文明智能體生態 — 系統不僅執行指令，更能理解環境、學習結構、銘記經驗並帶著確鑿意圖行動。"* — [`doc.md`](../../doc.md)

| 支柱 | 命名 | 核心實現 |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **精煉智能運行時** | [`AgentEngine`](../../src/core/engine.rs) 多輪 ReAct 狀態機、KV 快取確定性 [`SystemPromptSpec`](../../src/domain/prompt_spec.rs)、[`ContextInspectionSnapshot`](../../src/core/context_inspector.rs) 上下文與預算檢測、三階段自癒式 [`ToolCallArgParser`](../../src/domain/tool_parser.rs) 以及 `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs)。 |
| **2. `SPIDER`** | **結構感知與環境理解** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) 結構關係分析與可供性分類、[`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) 環境觀察以及 `spider.check` 嚴格 AST 審計。 |
| **3. `REWIND`** | **時空認知、連續性與確定性恢復** | 16-MiB [`ArenaAllocator`](../../src/domain/arena.rs) 與 $O(1)$ 快照重退、純 Rust SHA-256 [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs)（具備 `WouldBlock` 保護的 `/undo`、`/restore` 與非 Git `/diff`）、兩階段 [`SessionVfs`](../../src/infrastructure/vfs_store.rs) 基準比對校驗、[`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) 回滾機制，以及 32 深度 [`VirtualDisplayState`](../../src/domain/computer_use.rs) 復原/重做堆疊。 |
| **4. `BROCCOLI`** | **持久化知識、記憶與歷史演進** | 帶拉普拉斯平滑校準置信度的 [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs)、混合式 TF-IDF [`MemoryStore`](../../src/domain/memory.rs)，以及基於 Kahn 拓撲排序校驗的 [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) 有向無環圖。 |
| **5. `SYNTHETIC SWARM`** | **分布式集體智能** | 5 大專家並行協商（視覺感知者、結構分析者、工作流程領航者、風險評估者、恢復專家 + 衝突解決仲裁者）以及 7 專家 [`Legion`](../../src/domain/swarm.rs) 拓撲陣型（包含獨佔檔案保留與確定性任務交接）。 |
| **6. `COMPUTER USE`** | **具身自主：觀察、意圖解釋、行動與自適應** | 9 階段閉環 [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs)、雙重包裹 [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs)（Wrapper A 空間錨定 + Wrapper B 確定性保障）、[`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) 視口外自動滾動，以及零信任的 `browser_use`。 |
| **7. `THE HARNESS`** | **貫穿一切的底層基質體系** | 31 個原生工具（含 8 槽 LRU 快取的 `tool_search` 與 32 步批量執行 `execute_tools`）、DAG 波前並發調度器、5 級遞進的 [`ExecutionPosture`](../../src/domain/types/posture_types.rs)、macOS Seatbelt 沙箱、完整 [`TurnReceipt`](../../src/domain/turn_receipt.rs) 審計憑證，以及多路復用 JSON-RPC stdio 服務與原生 macOS AppKit 客戶端。 |

---

## 5 域 Joyzoning 架構 (`82` 個 Rust 模組 · `300+` 個工作區檔案)

代碼庫中每個原始檔均歸屬於 5 個經過嚴格驗證的架構域之一：

| 領域 | 物理路徑 | 核心職責與架構約束 |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` 個模組) | 無狀態葉子函數與基礎原語：雜湊與數學、文本標記預算、JSON 自癒修復、確定性密鑰脫敏、Seatbelt 沙箱配置、Shell 防護管道、模糊修補引擎、空間幾何及環境感知分析。**絕對零向上引用、零可變全局狀態。** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` 個模組) | 純業務實體與 64 個正式 `I<Name>` 介面特徵、執行姿態、模型定價名錄、系統提示詞規範、Arena 記憶體分配器、技能樹目錄、記憶儲存、蜂群調度模型。**嚴格 0% I/O 交互，單檔案不超過 3 個結構體。** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` 個模組) | 實體 I/O 適配層：HTTP 與 61 家模型提供商協議適配（OpenAI、Anthropic、Gemini、Codestral 等）、兩階段 VFS 儲存、SHA-256 CAS 工作區快照、行為經驗庫與沙箱 Shell 執行器。 |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` 個模組) | 用例編排層：`AgentEngine` 核心狀態機、波前工具調度、空間互動引擎、上下文審查器、運行佇列與自測飛行套件。 |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs)、[`native/`](../../native/) | 表現與使用者意圖層：CLI 呈現、stdio JSON-RPC 多路工作站、原生 macOS AppKit 桌面應用及程式入口組裝點。 |

---

## 快速上手與驗證 (Quick Start)

```bash
# 1. 運行全量 Rust 單元測試、空間引擎、具身用例及整合測試
cargo test

# 2. 運行原生 macOS AppKit 測試套件
cd native && swift test && cd ..

# 3. 運行自主飛行驗證與自檢診斷
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. 運行嚴格 Joyzoning 架構審計與文檔檢查
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
