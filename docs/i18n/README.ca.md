<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Entorn d'Intel·ligència Sobirana (Sovereign Intelligence Runtime)

**Entorn d'intel·ligència en Rust hipercomprimit i arquitectònicament sobirà, motor d'acció espacial de doble embolcall i IDE natiu per a macOS en AppKit (`HEAV3NS-CODE`)** — estructurat sota l'arquitectura de 5 zones Joyzoning de **HEAV3NS-JOY** (`100%` d'Índex d'Estabilitat, 0 violacions, tots els fitxers de l'espai de treball dins de Tier 1 Optimal `< 800` línies).

<p align="center">
  <a href="../../doc.md">Carta Sobirana (doc.md)</a> ·
  <a href="../../AGENTS.md">Protocol d'Agents (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Esperit Fundacional (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Full de Ruta</a> ·
  <a href="../README.md">Centre de Documentació</a> ·
  <a href="../papers/ascendant-intelligence.md">Llibre Blanc d'Intel·ligència Ascendent</a> ·
  <a href="../PROJECT_MAP.md">Mapa del Projecte</a> ·
  <a href="../CONSTITUTION.md">Límits Constitucionals</a> ·
  <a href="../../VALIDATION_REPORT.md">Informe de Validació</a> ·
  <a href="../../native/PRODUCT.md">Especificació Nativa</a>
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

## Els Set Pilars Sobirans (The Seven Sovereign Pillars)

> *"No és un simple agent. No és un eixam. No és un sistema operatiu. És una civilització computacional d'intel·ligència cooperativa — on el sistema no es limita a executar instruccions: comprèn el seu entorn, aprèn la seva estructura, recorda la seva experiència i actua amb una intenció deliberada."* — [`doc.md`](../../doc.md)

| Pilar | Designació | Implementació Principal |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **L'Entorn d'Intel·ligència Destil·lat** | Màquina d'estats ReAct multi-torn [`AgentEngine`](../../src/core/engine.rs), especificació de prompts determinista per a la memòria cau KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspecció de context [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), analitzador autorreparable de 3 fases [`ToolCallArgParser`](../../src/domain/tool_parser.rs) i suite de vol `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Percepció Estructural i Comprensió de l'Entorn** | Anàlisi de relacions estructurals i classificació d'oportunitats d'acció a [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), observacions a [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) i auditories AST estrictes amb `spider.check`. |
| **3. `REWIND`** | **Cognició Temporal, Continuïtat i Recuperació Determinista** | Assignador de memòria d'arena de 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) amb rebobinat $O(1)$, CAS d'instantànies SHA-256 en Rust pur [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (amb `/undo`, `/restore` protegits contra bloquejos i `/diff` independent de Git), verificació en dues fases [`SessionVfs`](../../src/infrastructure/vfs_store.rs), registre de transaccions amb reversió [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) i pila de desfer/refer de 32 nivells a [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Coneixement Persistent, Memòria i Evolució Històrica** | Magatzem de coneixement conductual amb calibratge de confiança per suavitzat de Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), memòria híbrida TF-IDF [`MemoryStore`](../../src/domain/memory.rs) i catàleg d'habilitats en DAG validat amb Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Intel·ligència Col·lectiva Distribuïda** | Consulta paral·lela de 5 especialistes (percepció visual, anàlisi estructural, navegació de fluxos, avaluació de riscos i recuperació + àrbitre de conflictes) i formacions topològiques [`Legion`](../../src/domain/swarm.rs) de 7 especialistes amb reserves exclusives. |
| **6. `COMPUTER USE`** | **Agència Corpòria: Observació, Interpretació, Acció i Adaptació** | Orquestrador en bucle tancat de 9 etapes [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediador d'accions espacials de doble embolcall [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Embolcall A Ancoratge Espacial + Embolcall B Garantia d'Execució), desplaçament automàtic fora de pantalla a [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) i `browser_use` de confiança zero. |
| **7. `THE HARNESS`** | **El Substrat Computacional Unificador** | 31 eines natives (cerca LRU de 8 ranures i execució per lots de 32 passos), planificador de front d'ona DAG, 5 postures d'execució [`ExecutionPosture`](../../src/domain/types/posture_types.rs), sandbox Seatbelt a macOS, rebut d'auditoria [`TurnReceipt`](../../src/domain/turn_receipt.rs), servei stdio JSON-RPC multiplexat i client natiu AppKit per a macOS. |

---

## Arquitectura de 5 Zones Joyzoning (`82` Mòduls Rust · `300+` Fitxers)

| Zona | Ruta Física | Responsabilitats i Invariants |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` mòduls) | Primitives fulla sense estat: funcions resum, pressupost de tokens, reparació de JSON, ofuscació de secrets, Seatbelt, protecció de l'intèrpret shell, pedaços difusos i geometria espacial. **Zero importacions ascendents, zero estat global mutable.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` mòduls) | Entitats pures i 64 trets formals `I<Name>`, postures, catàleg de preus, especificacions de prompts, assignador d'arena, arbre d'habilitats. **Estrictament 0% d'E/S, màxim 3 estructures per fitxer.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` mòduls) | Adaptadors concrets d'E/S: protocols de xarxa per a 61 proveïdors, emmagatzematge VFS en dues fases, instantànies CAS SHA-256, magatzem conductual i execució aïllada en sandbox. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` mòduls) | Orquestració de casos d'ús: màquina d'estats `AgentEngine`, planificador per front d'ona, motor espacial, inspector de context i proves de vol. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Presentació i intenció de l'usuari: interfície CLI, treballador stdio JSON-RPC, aplicació nativa AppKit per a macOS i arrels de composició. |

---

## Guia Ràpida i Verificació (Quick Start)

```bash
# 1. Executa les proves unitàries de Rust, espacials, d'ús de computadores i d'integració
cargo test

# 2. Executa la suite de proves natives macOS AppKit
cd native && swift test && cd ..

# 3. Executa la suite de vol autònom i diagnòstic del sistema
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Auditoria estricta d'arquitectura Joyzoning i verificació d'enllaços
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
