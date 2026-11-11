<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — 주권 지능 런타임 (Sovereign Intelligence Runtime)

**초압축, 아키텍처 주권형 Rust 지능 런타임, 이중 래핑 공간 행동 엔진 및 네이티브 macOS AppKit IDE (`HEAV3NS-CODE`)** — **HEAV3NS-JOY** 5구역 Joyzoning 아키텍처 준수 (`100%` 안정성 지수, 위반 0건, 모든 작업 공간 파일 Tier 1 Optimal `< 800`줄 이내).

<p align="center">
  <a href="../../doc.md">주권 헌장 (doc.md)</a> ·
  <a href="../../AGENTS.md">에이전트 규약 (AGENTS.md)</a> ·
  <a href="../../SOUL.md">핵심 정신 (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">로드맵</a> ·
  <a href="../README.md">문서 센터</a> ·
  <a href="../papers/ascendant-intelligence.md">상승 지능 백서</a> ·
  <a href="../PROJECT_MAP.md">프로젝트 맵</a> ·
  <a href="../CONSTITUTION.md">헌법적 제약</a> ·
  <a href="../../VALIDATION_REPORT.md">검증 보고서</a> ·
  <a href="../../native/PRODUCT.md">네이티브 제품 사양</a>
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

## 7대 주권 기둥 (The Seven Sovereign Pillars)

> *"단순한 에이전트가 아닙니다. 스웜도 아닙니다. 운영체제도 아닙니다. 협력하는 지능의 계산 문명입니다 — 시스템은 단순히 지시를 실행하는 데 그치지 않고, 환경을 이해하고, 구조를 학습하며, 경험을 기억하고, 확고한 의도로 행동합니다."* — [`doc.md`](../../doc.md)

| 기둥 | 명칭 | 핵심 구현 |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **정제된 지능 런타임** | [`AgentEngine`](../../src/core/engine.rs) 다중 턴 ReAct 상태 머신, KV 캐시 결정론적 [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs) 컨텍스트 검사기, 3단계 자체 치유 [`ToolCallArgParser`](../../src/domain/tool_parser.rs), `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **구조적 지각 및 환경 이해** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) 구조 관계 분석 및 행동 가능성 분류, [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs), `spider.check` 엄격한 AST 감사. |
| **3. `REWIND`** | **시간적 인지, 연속성 및 결정론적 복구** | 16-MiB [`ArenaAllocator`](../../src/domain/arena.rs) 및 $O(1)$ 스냅샷 되감기, 순수 Rust SHA-256 [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs)(`WouldBlock` 보호 `/undo`, `/restore`, 비Git `/diff`), 2단계 [`SessionVfs`](../../src/infrastructure/vfs_store.rs) 베이스라인 검증, [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) 롤백, 32단계 가상 디스플레이 되돌리기/다시실행 스택. |
| **4. `BROCCOLI`** | **지속적 지식, 기억 및 역사적 진화** | 라플라스 평활화 신뢰도 보정을 갖춘 [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), 하이브리드 TF-IDF [`MemoryStore`](../../src/domain/memory.rs), Kahn 위상 검증을 거친 [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) DAG. |
| **5. `SYNTHETIC SWARM`** | **분산 집단 지능** | 5대 전문가 병렬 자문(시각 인식, 구조 분석, 워크플로 탐색, 위험 평가, 복구 전문가 + 충돌 중재자) 및 7개 전문가 [`Legion`](../../src/domain/swarm.rs) 위상 대형. |
| **6. `COMPUTER USE`** | **체화된 주도성: 관찰, 해석, 실행, 적응** | 9단계 폐루프 [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), 이중 래핑 [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs)(Wrapper A 공간 접지 + Wrapper B 실행 보증), [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) 자동 스크롤 및 무신뢰 `browser_use`. |
| **7. `THE HARNESS`** | **모든 것을 엮어내는 기질 인프라** | 31개 네이티브 도구(8슬롯 LRU `tool_search` & 32단계 일괄 실행 `execute_tools`), DAG 파면 동시 스케줄러, 5단계 [`ExecutionPosture`](../../src/domain/types/posture_types.rs), macOS Seatbelt 샌드박스, 완전한 [`TurnReceipt`](../../src/domain/turn_receipt.rs) 감사 영수증, 다중화 stdio JSON-RPC 워커 및 네이티브 macOS AppKit 클라이언트. |

---

## 5구역 Joyzoning 아키텍처 (`82`개 Rust 모듈 · `300+`개 파일)

| 구역 | 물리 경로 | 책임 및 불변 조건 |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12`개) | 무상태 리프 프리미티브: 해시, 토큰 예산, JSON 복구, 비밀 정보 마스킹, 샌드박스, 셸 보호, 패치 적용, 공간 기하학. **상향 임포트 금지, 가변 글로벌 상태 0건.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31`개) | 순수 엔티티 및 64개 공식 `I<Name>` 인터페이스, 실행 자세, 모델 가격 카탈로그, 프롬프트 사양, 아레나 할당기, 기술 트리. **I/O 0%, 파일당 3개 이하의 구조체.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11`개) | 구체적 I/O 어댑터: HTTP 및 61개 프로바이더 와이어 프로토콜, 2단계 VFS, SHA-256 CAS 스냅샷, 행동 지식 저장소, 샌드박스 셸 실행기. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20`개) | 유스케이스 조율: `AgentEngine` 상태 머신, 파면 도구 스케줄러, 공간 상호작용 엔진, 컨텍스트 검사기, 실행 큐, 비행 테스트. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | 프레젠테이션 계층: CLI 발표기, stdio JSON-RPC 워커, 데스크톱 AppKit 애플리케이션, 진입점 루트. |

---

## 빠른 시작 및 검증 (Quick Start)

```bash
# 1. Rust 단위, 공간, 컴퓨터 사용 및 통합 테스트 실행
cargo test

# 2. 네이티브 macOS AppKit 테스트 실행
cd native && swift test && cd ..

# 3. 비행 검증 및 진단 실행
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Joyzoning 아키텍처 감사 및 문서 점검
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
