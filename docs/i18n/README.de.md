<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Souveräne Intelligenz-Laufzeitumgebung (Sovereign Intelligence Runtime)

**Hyperkomprimierte, architektonisch souveräne Rust-Intelligenz-Laufzeitumgebung, zweifach gekapselte Spatial-Action-Engine und native macOS AppKit IDE (`HEAV3NS-CODE`)** — strukturiert unter der **HEAV3NS-JOY** 5-Zonen-Joyzoning-Architektur (`100%` Stabilitätsindex, null Verstöße, alle Dateien im Workspace innerhalb Tier 1 Optimal `< 800` Zeilen).

<p align="center">
  <a href="../../doc.md">Souveräne Charta (doc.md)</a> ·
  <a href="../../AGENTS.md">Agenten-Protokoll (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Kernelement (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Roadmap</a> ·
  <a href="../README.md">Dokumentations-Hub</a> ·
  <a href="../papers/ascendant-intelligence.md">Ascendant Intelligence Papier</a> ·
  <a href="../PROJECT_MAP.md">Projektübersicht</a> ·
  <a href="../CONSTITUTION.md">Verfassungsgrenzen</a> ·
  <a href="../../VALIDATION_REPORT.md">Validierungsbericht</a> ·
  <a href="../../native/PRODUCT.md">Native Spezifikation</a>
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

## Die sieben souveränen Säulen (The Seven Sovereign Pillars)

> *"Kein bloßer Agent. Kein Schwarm. Kein Betriebssystem. Eine rechnerische Zivilisation kooperierender Intelligenz — in der das System Anweisungen nicht einfach ausführt: Es versteht seine Umgebung, erlernt deren Struktur, behält Erfahrungen im Gedächtnis und handelt mit fundierter Absicht."* — [`doc.md`](../../doc.md)

| Säule | Bezeichnung | Kernimplementierung |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Die destillierte Intelligenz-Laufzeitumgebung** | Multi-Turn ReAct Zustandsautomat [`AgentEngine`](../../src/core/engine.rs), KV-Cache deterministische [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), Kontextinspektion [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), dreistufiger selbstreparierender [`ToolCallArgParser`](../../src/domain/tool_parser.rs) und `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Strukturwahrnehmung und Umgebungsverständnis** | Strukturelle Beziehungsanalyse und Affordanzklassifizierung in [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), Umgebungsbeobachtungen [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) und strenge AST-Audits mit `spider.check`. |
| **3. `REWIND`** | **Temporale Kognition, Kontinuität und deterministische Wiederherstellung** | 16-MiB [`ArenaAllocator`](../../src/domain/arena.rs) mit $O(1)$ Snapshot-Rückspulung, reines Rust SHA-256 [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (mit blockierungsgeschütztem `/undo`, `/restore` und nicht-Git `/diff`), zweiphasige [`SessionVfs`](../../src/infrastructure/vfs_store.rs) Baseline-Prüfung, Transaktionsjournal [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) mit Rollback und 32-stufiger [`VirtualDisplayState`](../../src/domain/computer_use.rs) Undo/Redo-Stack. |
| **4. `BROCCOLI`** | **Persistentes Wissen, Gedächtnis und historische Evolution** | Verhaltenswissensspeicher mit Laplace-geglätteter Konfidenzkalibrierung [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), hybrider TF-IDF [`MemoryStore`](../../src/domain/memory.rs) und Kahn-topologisch validierter [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) DAG. |
| **5. `SYNTHETIC SWARM`** | **Verteilte kollektive Intelligenz** | Parallele Konsultation von 5 Spezialisten (visuelle Wahrnehmung, Strukturanalyse, Workflow-Navigation, Risikobewertung und Wiederherstellung + Konfliktlösung) und 7-Spezialisten [`Legion`](../../src/domain/swarm.rs) Topologieformationen mit exklusiven Dateireservierungen. |
| **6. `COMPUTER USE`** | **Verkörperte Handlungsfähigkeit: Beobachtung, Interpretation, Aktion und Anpassung** | Neunstufiger geschlossener Regelkreis [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), doppelt gekapselter [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Hülle A Raumverankerung + Hülle B Ausführungsgarantie), automatisches Scrollen außerhalb des Bildschirms in [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) und Zero-Trust `browser_use`. |
| **7. `THE HARNESS`** | **Das verbindende rechnerische Substrat** | 31 native Werkzeuge (8-Slot LRU-Suche `tool_search` & 32-Schritt Batch-Ausführung `execute_tools`), DAG-Wellenfront-Scheduler, 5 Ausführungshaltungen [`ExecutionPosture`](../../src/domain/types/posture_types.rs), macOS Seatbelt Sandbox, vollständige Prüfbelege [`TurnReceipt`](../../src/domain/turn_receipt.rs), gemultiplexter stdio JSON-RPC Dienst und nativer macOS AppKit Client. |

---

## 5-Zonen Joyzoning Architektur (`82` Rust-Module · `300+` Dateien)

| Zone | Pfad | Verantwortung und Invarianten |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` Module) | Zustandslose Blattprimitive: Hashing, Token-Budget, JSON-Reparatur, Geheimnisschutz, Seatbelt, Shell-Schutz, Fuzzy-Patches und Raumgeometrie. **Keine Aufwärtsimporte, kein veränderlicher globaler Zustand.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` Module) | Reine Domänenentitäten und 64 formale `I<Name>` Traits, Haltungen, Preismodelle, Prompt-Spezifikationen, Arena-Allokator, Skill-Tree. **Strikte 0% I/O, maximal 3 Structs pro Datei.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` Module) | Konkrete E/A-Adapter: HTTP und Wire-Dialekte für 61 Provider, zweiphasiges VFS, SHA-256 CAS-Snapshots, Verhaltenswissen und Sandbox-Shell. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` Module) | Anwendungsfall-Orchestrierung: `AgentEngine` Zustandsautomat, Wellenfront-Scheduler, Raum-Engine, Kontextinspektor und Testflüge. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Präsentation und Nutzerabsicht: CLI-Presenter, stdio JSON-RPC Worker, native Desktop-Applikation in AppKit und Kompositionswurzeln. |

---

## Schnellstart und Verifizierung (Quick Start)

```bash
# 1. Rust-Unit-, Raum-, Computer-Use- und Integrationstests ausführen
cargo test

# 2. Native macOS AppKit Test-Suite ausführen
cd native && swift test && cd ..

# 3. Flugtest-Suite und Diagnose ausführen
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Strikte Joyzoning-Architekturprüfung und Dokumentenprüfung durchführen
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
