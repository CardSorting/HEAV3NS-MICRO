<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — ソブリン・インテリジェンス・ランタイム (Sovereign Intelligence Runtime)

**超圧縮・アーキテクチャ主権型 Rust インテリジェンスランタイム、二重ラップ空間アクションエンジン、およびネイティブ macOS AppKit IDE (`HEAV3NS-CODE`)** — **HEAV3NS-JOY** 5ゾーン Joyzoning アーキテクチャに準拠（`100%` 安定性インデックス、違反ゼロ、すべてのワークスペースファイルが Tier 1 Optimal `< 800` 行以内）。

<p align="center">
  <a href="../../doc.md">主権憲章 (doc.md)</a> ·
  <a href="../../AGENTS.md">エージェント規約 (AGENTS.md)</a> ·
  <a href="../../SOUL.md">中核精神 (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">ロードマップ</a> ·
  <a href="../README.md">ドキュメントハブ</a> ·
  <a href="../papers/ascendant-intelligence.md">上昇知能論文</a> ·
  <a href="../PROJECT_MAP.md">プロジェクト構造</a> ·
  <a href="../CONSTITUTION.md">憲法規範</a> ·
  <a href="../../VALIDATION_REPORT.md">検証レポート</a> ·
  <a href="../../native/PRODUCT.md">ネイティブ仕様書</a>
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

## 7大主権ピラー (The Seven Sovereign Pillars)

> *「単一のエージェントではない。スウォームでもない。OSでもない。これは協調する知能の計算文明である — システムは単に命令を実行するだけでなく、環境を理解し、構造を学習し、経験を記憶し、確固たる意図を持って行動する。」* — [`doc.md`](../../doc.md)

| ピラー | 名称 | コア実装 |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **蒸留知能ランタイム** | [`AgentEngine`](../../src/core/engine.rs) マルチターン ReAct 状態マシン、KVキャッシュ決定論的 [`SystemPromptSpec`](../../src/domain/prompt_spec.rs)、[`ContextInspectionSnapshot`](../../src/core/context_inspector.rs) コンテキスト検査、3段階自己修復 [`ToolCallArgParser`](../../src/domain/tool_parser.rs)、および `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs)。 |
| **2. `SPIDER`** | **構造知覚と環境理解** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) 構造関係分析とアフォーダンス分類、[`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) 環境観測、および `spider.check` 厳密な AST 監査。 |
| **3. `REWIND`** | **時間認知、継続性、確定的回復** | 16-MiB [`ArenaAllocator`](../../src/domain/arena.rs) と $O(1)$ スナップショット巻き戻し、純 Rust SHA-256 [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs)（`WouldBlock` 保護付き `/undo`、`/restore`、非Git `/diff`）、2フェーズ [`SessionVfs`](../../src/infrastructure/vfs_store.rs) 基準比較検証、[`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) ロールバック、および 32 深度の [`VirtualDisplayState`](../../src/domain/computer_use.rs) undo/redo スタック。 |
| **4. `BROCCOLI`** | **持続的知識、記憶、歴史的進化** | ラプラス平滑化信頼度較正を備えた [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs)、ハイブリッド TF-IDF [`MemoryStore`](../../src/domain/memory.rs)、および Kahn トポロジカル検証済み [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) DAG。 |
| **5. `SYNTHETIC SWARM`** | **分散集合知能** | 5専門家並行協議（視覚知覚者、構造アナリスト、ワークフローナビゲーター、リスク評価者、回復スペシャリスト ＋ 競合調停者）および 7専門家 [`Legion`](../../src/domain/swarm.rs) トポロジー編成。 |
| **6. `COMPUTER USE`** | **身体化エージェンシー：観察、解釈、行動、適応** | 9段階閉ループ [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs)、二重ラップ [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs)（Wrapper A 空間接地 ＋ Wrapper B 実行保証）、[`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) 画面外自動スクロール、およびゼロトラスト `browser_use`。 |
| **7. `THE HARNESS`** | **すべてを統合する計算基盤** | 31個のネイティブツール（8スロットLRUキャッシュ `tool_search` ＆ 32ステップ一括実行 `execute_tools`）、DAG 波面スケジューラ、5段階 [`ExecutionPosture`](../../src/domain/types/posture_types.rs)、macOS Seatbelt サンドボックス、完全な [`TurnReceipt`](../../src/domain/turn_receipt.rs) 監査証跡、多重化 stdio JSON-RPC ワーカー、およびネイティブ macOS AppKit クライアント。 |

---

## 5ゾーン Joyzoning アーキテクチャ (`82` Rust モジュール · `300+` ファイル)

| ゾーン | パス | 責務と不変条件 |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` モジュール) | ステートレスな末端プリミティブ：ハッシュ、トークン予算、JSON修復、秘密情報マスキング、Seatbelt、シェル保護、パッチ適用、空間幾何。**上位インポート厳禁・可変グローバル状態ゼロ。** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` モジュール) | 純粋なドメインエンティティおよび 64 の公式 `I<Name>` トレイト、実行ポスチャ、モデル価格カタログ、プロンプト仕様、アリーナアロケータ、スキルツリー。**I/O 厳格 0%、ファイルあたり 3 構造体以内。** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` モジュール) | 具象 I/O アダプタ：HTTP および 61 プロバイダ連携、2フェーズ VFS、SHA-256 CAS スナップショット、行動学習ストア、サンドボックスシェル実行。 |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` モジュール) | ユースケース統合：`AgentEngine` 状態マシン、空間エンジン、コンテキスト検査、実行キュー、フライトテスト。 |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs)、[`native/`](../../native/) | プレゼンテーション層：CLI 表示、stdio JSON-RPC ワーカー、デスクトップ AppKit アプリケーション、結合ルート。 |

---

## クイックスタートと検証 (Quick Start)

```bash
# 1. Rust 単体・空間・コンピュータユース・統合テスト実行
cargo test

# 2. ネイティブ macOS AppKit テスト実行
cd native && swift test && cd ..

# 3. フライトテスト・健全性診断
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Joyzoning アーキテクチャ監査およびドキュメント検証
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
