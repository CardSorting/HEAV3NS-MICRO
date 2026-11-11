<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Sovranità di Intelligenza Computazionale (Sovereign Intelligence Runtime)

**Runtime di intelligenza in Rust ipercompresso e architetturalmente sovrano, motore di azione spaziale a doppio involucro e IDE nativo macOS AppKit (`HEAV3NS-CODE`)** — strutturato secondo l'architettura a 5 zone Joyzoning di **HEAV3NS-JOY** (Indice di Stabilità `100%`, zero violazioni, tutti i file entro Tier 1 Optimal `< 800` righe).

<p align="center">
  <a href="../../doc.md">Carta Sovrana (doc.md)</a> ·
  <a href="../../AGENTS.md">Protocollo Agenti (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Anima Fondante (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Tabella di Marcia</a> ·
  <a href="../README.md">Centro Documentazione</a> ·
  <a href="../papers/ascendant-intelligence.md">Documento Intelligenza Ascendente</a> ·
  <a href="../PROJECT_MAP.md">Mappa Progetto</a> ·
  <a href="../CONSTITUTION.md">Vincoli Costituzionali</a> ·
  <a href="../../VALIDATION_REPORT.md">Rapporto Validazione</a> ·
  <a href="../../native/PRODUCT.md">Specifica Nativa</a>
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

## I Sette Pilastri Sovrani (The Seven Sovereign Pillars)

> *"Non un semplice agente. Non uno sciame. Non un sistema operativo. Una civiltà computazionale di intelligenza cooperativa — in cui il sistema non si limita a eseguire istruzioni: comprende l'ambiente, ne apprende la struttura, ricorda l'esperienza e agisce con ferma intenzione."* — [`doc.md`](../../doc.md)

| Pilastro | Designazione | Implementazione Principale |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Il Runtime di Intelligenza Distillato** | Macchina a stati ReAct multi-turno [`AgentEngine`](../../src/core/engine.rs), specifica prompt deterministica per la cache KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), ispezione del contesto [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), parser di strumenti autoriparante a 3 stadi [`ToolCallArgParser`](../../src/domain/tool_parser.rs) e suite di volo `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Percezione Strutturale e Comprensione Ambientale** | Analisi delle relazioni strutturali e classificazione delle affordance in [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), osservazioni ambientali [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) e rigorosi controlli AST con `spider.check`. |
| **3. `REWIND`** | **Cognizione Temporale, Continuità e Ripristino Deterministico** | Allocatore di memoria arena da 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) con riavvolgimento $O(1)$, CAS di snapshot SHA-256 in Rust puro [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (con `/undo`, `/restore` protetti da blocchi e `/diff` indipendente da Git), verifica a due fasi [`SessionVfs`](../../src/infrastructure/vfs_store.rs), registro transazioni [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) con rollback e stack di annullamento/ripetizione a 32 livelli [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Conoscenza Persistente, Memoria ed Evoluzione Storica** | Archivio della conoscenza comportamentale con calibrazione della confidenza di Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), memoria ibrida TF-IDF [`MemoryStore`](../../src/domain/memory.rs) e catalogo delle competenze DAG validato con Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Intelligenza Collettiva Distribuita** | Consultazione parallela di 5 specialisti (percezione visiva, analisi strutturale, navigazione workflow, valutazione rischi, ripristino + arbitro dei conflitti) e formazioni topologiche a 7 specialisti [`Legion`](../../src/domain/swarm.rs). |
| **6. `COMPUTER USE`** | **Agentività Incarnata: Osservazione, Interpretazione, Azione e Adattamento** | Orchestratore a circuito chiuso in 9 fasi [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediatore di azioni spaziali a doppio involucro [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Involucro A Radicamento Spaziale + Involucro B Garanzia di Esecuzione), scorrimento automatico fuori schermo in [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) e `browser_use` zero-trust. |
| **7. `THE HARNESS`** | **Il Substrato Computazionale Unificante** | 31 strumenti nativi (ricerca LRU a 8 slot ed esecuzione batch di 32 passaggi), pianificatore d'onda concorrente DAG, 5 posture di esecuzione [`ExecutionPosture`](../../src/domain/types/posture_types.rs), sandbox Seatbelt di macOS, ricevuta di turno [`TurnReceipt`](../../src/domain/turn_receipt.rs), servizio stdio JSON-RPC multiplato e client nativo macOS AppKit. |

---

## Architettura a 5 Zone Joyzoning (`82` Moduli Rust · `300+` File)

| Zona | Percorso Fisico | Responsabilità e Invarianti |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` moduli) | Primitive foglia senza stato: hashing, budget dei token, riparazione JSON, oscuramento segreti, Seatbelt, guardia shell, patch fuzzy e geometria spaziale. **Zero import verso l'alto, zero stato globale mutabile.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` moduli) | Entità pure e 64 trait formali `I<Name>`, posture, catalogo prezzi, specifiche prompt, allocatore arena, albero delle competenze. **Rigoroso 0% I/O, massimo 3 struct per file.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` moduli) | Adattatori I/O concreti: protocolli di rete per 61 provider, storage VFS a due fasi, snapshot CAS SHA-256, archivio comportamentale ed esecuzione sandboxed. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` moduli) | Orchestrazione casi d'uso: macchina a stati `AgentEngine`, pianificatore d'onda, motore spaziale, ispettore del contesto e suite di volo. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Presentazione e intenti utente: CLI, worker stdio JSON-RPC, applicazione desktop macOS AppKit e radici di composizione. |

---

## Guida Rapida e Verifica (Quick Start)

```bash
# 1. Esegui i test unitari Rust, spaziali, uso computer e integrazione
cargo test

# 2. Esegui la suite di test nativa macOS AppKit
cd native && swift test && cd ..

# 3. Esegui i test di volo autonomo e la diagnostica di sistema
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Audit rigoroso Joyzoning e controllo collegamenti documentali
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
