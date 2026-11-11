<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Суверенне Середовище Інтелекту (Sovereign Intelligence Runtime)

**Гіперстиснене, архітектурно суверенне середовище інтелекту на Rust, просторовий рушій дій із подвійною оболонкою та нативне середовище macOS AppKit IDE (`HEAV3NS-CODE`)** — структуровано за 5-зонною архітектурою Joyzoning **HEAV3NS-JOY** (Індекс стабільності `100%`, 0 порушень, усі файли робочого простору перебувають у межах Tier 1 Optimal `< 800` рядків).

<p align="center">
  <a href="../../doc.md">Суверенна хартія (doc.md)</a> ·
  <a href="../../AGENTS.md">Протокол агентів (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Базовий дух (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Дорожня карта</a> ·
  <a href="../README.md">Центр документації</a> ·
  <a href="../papers/ascendant-intelligence.md">Маніфест висхідного інтелекту</a> ·
  <a href="../PROJECT_MAP.md">Карта проєкту</a> ·
  <a href="../CONSTITUTION.md">Конституційні рамки</a> ·
  <a href="../../VALIDATION_REPORT.md">Звіт валідації</a> ·
  <a href="../../native/PRODUCT.md">Специфікація AppKit</a>
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

## Сім Суверенних Стовпів (The Seven Sovereign Pillars)

> *"Не просто агент. Не просто рій. Не просто операційна система. Це обчислювальна цивілізація взаємодіючого інтелекту — де система не просто виконує інструкції: вона розуміє середовище, вивчає його структуру, зберігає досвід і діє з чітким наміром."* — [`doc.md`](../../doc.md)

| Стовп | Позначення | Ключова реалізація |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Дистильоване середовище інтелекту** | Багатокроковий скінченний автомат ReAct [`AgentEngine`](../../src/core/engine.rs), детермінована для KV-кешу специфікація промптів [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), інспектор контексту та бюджету [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), 3-ступеневий самовідновлюваний парсер [`ToolCallArgParser`](../../src/domain/tool_parser.rs) та льотний комплекс `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Структурне сприйняття та розуміння середовища** | Аналіз структурних зв'язків та класифікація affordances у [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), спостереження [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) та суворі AST-аудити з `spider.check`. |
| **3. `REWIND`** | **Часове пізнання, неперервність і детерміноване відновлення** | 16-МБ арена-алокатор [`ArenaAllocator`](../../src/domain/arena.rs) з $O(1)$ відкатом знімків, чистий Rust SHA-256 CAS робочої області [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (із захищеними від блокування `/undo`, `/restore` та незалежним від Git `/diff`), двоетапна перевірка бази [`SessionVfs`](../../src/infrastructure/vfs_store.rs), журнал транзакцій [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) з відкатом і 32-рівневий стек скасування/повторення дій у [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Довготривалі знання, пам'ять та історична еволюція** | Сховище поведінкового досвіду з калібруванням довіри за Лапласом [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), гібридна пам'ять TF-IDF [`MemoryStore`](../../src/domain/memory.rs) та верифікований за Каном DAG каталог навичок [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Розподілений колективний розум** | Паралельні консультації 5 експертів (зорове сприйняття, структурний аналіз, навігація робочим процесом, оцінка ризиків, відновлення + арбітр конфліктів) та 7-експертні топології [`Legion`](../../src/domain/swarm.rs) з ексклюзивним резервуванням файлів. |
| **6. `COMPUTER USE`** | **Втілена суб'єктність: спостереження, інтерпретація, дія, адаптація** | 9-етапний замкнений цикл [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), медіатор дій з подвійною оболонкою [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Оболонка A Просторова прив'язка + Оболонка B Гарантія виконання), автопрокрутка за екраном у [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) та `browser_use` нульової довіри. |
| **7. `THE HARNESS`** | **Обчислювальний субстрат загального об'єднання** | 31 нативний інструмент (8-слотовий LRU пошук та пакетне виконання до 32 кроків), хвильовий планувальник задач по графу DAG, 5 режимів виконання [`ExecutionPosture`](../../src/domain/types/posture_types.rs), пісочниця macOS Seatbelt, квитанції аудиту [`TurnReceipt`](../../src/domain/turn_receipt.rs), мультиплексний сервіс stdio JSON-RPC та нативний AppKit клієнт macOS. |

---

## 5-Зонна Архітектура Joyzoning (`82` Rust Модулі · `300+` Файлів)

| Зона | Фізичний шлях | Обов'язки та інваріанти |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` модулів) | Примітиви без стану: хешування, ліміти токенів, виправлення JSON, маскування секретів, Seatbelt, захист командного рядка shell, нечіткі патчі, просторова геометрія. **Жодних висхідних імпортів, жодного змінного глобального стану.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` модуль) | Чисті сутності та 64 формальні трейти `I<Name>`, режими виконання, каталог цін моделей, специфікації промптів, арена, дерево навичок. **Суворо 0% I/O, не більше 3 структур на файл.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` модулів) | Конкретні адаптери введення-виведення: HTTP та мережеві діалекти для 61 провайдера, 2-етапний VFS, CAS знімки SHA-256, сховище поведінкового досвіду та виконання в пісочниці. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` модулів) | Оркестрація сценаріїв: кінцевий автомат `AgentEngine`, хвильовий планувальник, просторовий рушій, інспектор контексту та тестові польоти. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Шар представлення та намірів: консольний інтерфейс CLI, воркер stdio JSON-RPC, нативний десктопний додаток AppKit для macOS та точки складання. |

---

## Швидкий старт і валідація (Quick Start)

```bash
# 1. Запуск усіх юніт-, просторових та інтеграційних тестів на Rust
cargo test

# 2. Запуск набору тестів нативного клієнта macOS AppKit
cd native && swift test && cd ..

# 3. Запуск автономних польотів та системної діагностики
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Суворий аудит архітектури Joyzoning та перевірка посилань документації
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
