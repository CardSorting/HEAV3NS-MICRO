<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Суверенная Среда Интеллекта (Sovereign Intelligence Runtime)

**Гиперсжатая, архитектурно суверенная среда интеллекта на Rust, пространственный исполнительный движок с двойным охватом и нативная среда macOS AppKit IDE (`HEAV3NS-CODE`)** — структурировано по 5-зонной архитектуре Joyzoning **HEAV3NS-JOY** (Индекс стабильности `100%`, 0 нарушений, все файлы рабочего пространства входят в Tier 1 Optimal `< 800` строк).

<p align="center">
  <a href="../../doc.md">Суверенная хартия (doc.md)</a> ·
  <a href="../../AGENTS.md">Протокол агентов (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Базовый дух (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Дорожная карта</a> ·
  <a href="../README.md">Центр документации</a> ·
  <a href="../papers/ascendant-intelligence.md">Манифест восходящего интеллекта</a> ·
  <a href="../PROJECT_MAP.md">Карта проекта</a> ·
  <a href="../CONSTITUTION.md">Конституционные рамки</a> ·
  <a href="../../VALIDATION_REPORT.md">Отчет валидации</a> ·
  <a href="../../native/PRODUCT.md">Спецификация AppKit</a>
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

## Семь Суверенных Столпов (The Seven Sovereign Pillars)

> *"Не просто агент. Не просто рой. Не просто операционная система. Это вычислительная цивилизация взаимодействующего интеллекта — где система не просто исполняет команды: она понимает окружение, изучает его структуру, сохраняет опыт и действует с осознанным намерением."* — [`doc.md`](../../doc.md)

| Столп | Обозначение | Ключевая реализация |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Дистиллированная среда интеллекта** | Многошаговый конечный автомат ReAct [`AgentEngine`](../../src/core/engine.rs), детерминированная для KV-кэша спецификация промптов [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), инспектор контекста и лимитов [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), 3-ступенчатый самовосстанавливающийся парсер [`ToolCallArgParser`](../../src/domain/tool_parser.rs) и летный комплекс `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Структурное восприятие и понимание среды** | Анализ связей и классификация возможностей в [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), наблюдения [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) и строгие AST-аудиты с `spider.check`. |
| **3. `REWIND`** | **Временное познание, непрерывность и детерминированное восстановление** | 16-МБ арена-аллокатор [`ArenaAllocator`](../../src/domain/arena.rs) с мгновенным $O(1)$ откатом снимков, чистый Rust SHA-256 CAS снимков рабочей области [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (с защищенными от блокировок `/undo`, `/restore` и автономным `/diff`), двухфазная проверка базы [`SessionVfs`](../../src/infrastructure/vfs_store.rs), журнал транзакций [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) с откатом и 32-уровневый стек отмены/повтора [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Долговременные знания, память и историческая эволюция** | Хранилище поведенческого опыта с калибровкой доверия методом Лапласа [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), гибридная память TF-IDF [`MemoryStore`](../../src/domain/memory.rs) и верифицированный по Кану DAG каталог навыков [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Распределенный коллективный разум** | Параллельные консультации 5 экспертов (зрительное восприятие, структурный анализ, навигация, оценка рисков, восстановление + арбитр конфликтов) и 7-экспертные топологии [`Legion`](../../src/domain/swarm.rs) с эксклюзивным резервированием файлов. |
| **6. `COMPUTER USE`** | **Воплощенная субъектность: наблюдение, интерпретация, действие, адаптация** | 9-этапный замкнутый цикл [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), медиатор с двойной защитой [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Оболочка A Пространственная привязка + Оболочка B Гарантия выполнения), автопрокрутка за экраном в [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) и `browser_use` нулевого доверия. |
| **7. `THE HARNESS`** | **Вычислительный субстрат общего объединения** | 31 нативный инструмент (8-слотовый LRU поиск и пакетное исполнение до 32 шагов), волновой планировщик задач по графу DAG, 5 режимов исполнения [`ExecutionPosture`](../../src/domain/types/posture_types.rs), песочница macOS Seatbelt, квитанции аудита [`TurnReceipt`](../../src/domain/turn_receipt.rs), мультиплексный сервис stdio JSON-RPC и нативный AppKit клиент macOS. |

---

## 5-Зонная Архитектура Joyzoning (`82` Rust Модуля · `300+` Файлов)

| Зона | Физический путь | Обязанности и инварианты |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` модулей) | Примитивы без состояния: хеширование, лимиты токенов, восстановление JSON, маскирование секретов, Seatbelt, защита оболочки shell, нечеткие патчи, пространственная геометрия. **Ноль импортов вверх, ноль глобального изменяемого состояния.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` модуль) | Чистые сущности и 64 формальных трейта `I<Name>`, режимы исполнения, каталог цен моделей, спецификации промптов, арена, дерево навыков. **Строго 0% I/O, не более 3 структур на файл.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` модулей) | Конкретные адаптеры ввода-вывода: HTTP и сетевые диалекты для 61 провайдера, 2-фазный VFS, CAS снимки SHA-256, хранилище поведенческого опыта и запуск в песочнице. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` модулей) | Оркестрация сценариев: конечный автомат `AgentEngine`, волновой планировщик, пространственный движок, инспектор контекста и комплекс тестовых полетов. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Слой представления и намерений: консольный интерфейс CLI, воркер stdio JSON-RPC, нативное десктопное приложение AppKit для macOS и точки сборки. |

---

## Быстрый старт и валидация (Quick Start)

```bash
# 1. Запуск всех юнит-, пространственных и интеграционных тестов на Rust
cargo test

# 2. Запуск набора тестов нативного клиента macOS AppKit
cd native && swift test && cd ..

# 3. Запуск автономных полетов и системной диагностики
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Строгий аудит архитектуры Joyzoning и проверка ссылок документации
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
