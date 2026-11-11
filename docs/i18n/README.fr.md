<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Environnement d'Intelligence Souveraine (Sovereign Intelligence Runtime)

**Environnement d'intelligence Rust hyper-compressé et souverain sur le plan architectural, moteur d'action spatiale à double enveloppe et IDE natif macOS AppKit (`HEAV3NS-CODE`)** — structuré selon l'architecture Joyzoning en 5 zones de **HEAV3NS-JOY** (indice de stabilité de `100%`, zéro violation, tous les fichiers sous le seuil Tier 1 Optimal `< 800` lignes).

<p align="center">
  <a href="../../doc.md">Charte Souveraine (doc.md)</a> ·
  <a href="../../AGENTS.md">Protocole des Agents (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Esprit Fondateur (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Feuille de Route</a> ·
  <a href="../README.md">Centre de Documentation</a> ·
  <a href="../papers/ascendant-intelligence.md">Livre Blanc Intelligence Ascendante</a> ·
  <a href="../PROJECT_MAP.md">Carte du Projet</a> ·
  <a href="../CONSTITUTION.md">Constitution</a> ·
  <a href="../../VALIDATION_REPORT.md">Rapport de Validation</a> ·
  <a href="../../native/PRODUCT.md">Spécifications Natives</a>
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

## Les Sept Piliers Souverains (The Seven Sovereign Pillars)

> *"Ni un simple agent. Ni un essaim. Ni un système d'exploitation. Une véritable civilisation computationnelle d'intelligence coopérative — où le système ne se contente pas d'exécuter des instructions : il comprend son environnement, apprend sa structure, retient son expérience et agit avec une intention délibérée."* — [`doc.md`](../../doc.md)

| Pilier | Désignation | Implémentation Principale |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **L'Environnement d'Intelligence Distillé** | Machine à états ReAct multi-tours [`AgentEngine`](../../src/core/engine.rs), spécification de prompts déterministe pour le cache KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspection de contexte [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), analyseur autoréparateur en 3 phases [`ToolCallArgParser`](../../src/domain/tool_parser.rs) et suite de vol `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Perception Structurelle et Compréhension de l'Environnement** | Analyse des relations structurelles et classification des affordances dans [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), observations de l'environnement [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) et audits stricts de l'AST avec `spider.check`. |
| **3. `REWIND`** | **Cognition Temporelle, Continuité et Récupération Déterministe** | Allocateur mémoire arène de 16 Mo [`ArenaAllocator`](../../src/domain/arena.rs) avec rembobinage $O(1)$, CAS d'instantanés SHA-256 en Rust pur [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (avec `/undo`, `/restore` sécurisés contre les blocages et `/diff` hors Git), vérification en 2 phases avec [`SessionVfs`](../../src/infrastructure/vfs_store.rs), journal de transactions avec annulation [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) et pile de 32 niveaux dans [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Connaissance Persistante, Mémoire et Évolution Historique** | Magasin de connaissances comportementales avec calibration de confiance par lissage de Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), mémoire hybride TF-IDF [`MemoryStore`](../../src/domain/memory.rs) et catalogue de compétences en DAG vérifié avec Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Intelligence Collective Distribuée** | Consultation parallèle de 5 spécialistes (perception visuelle, analyse structurelle, navigation de flux, évaluation des risques et récupération + résolution des conflits) et formations topologiques [`Legion`](../../src/domain/swarm.rs) à 7 spécialistes avec réservations exclusives. |
| **6. `COMPUTER USE`** | **Agentivité Incarnée : Observation, Interprétation, Action et Adaptation** | Orchestrateur en boucle fermée à 9 étapes [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), médiateur à double enveloppe [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Enveloppe A Ancrage Spatial + Enveloppe B Assurance d'Exécution), défilement automatique hors écran dans [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) et `browser_use` à confiance zéro. |
| **7. `THE HARNESS`** | **Le Substrat Computationnel Unificateur** | 31 outils natifs (recherche LRU à 8 fentes et exécution par lots de 32 étapes), planificateur DAG par fronts d'ondes, 5 postures d'exécution [`ExecutionPosture`](../../src/domain/types/posture_types.rs), bac à sable macOS Seatbelt, reçu d'audit [`TurnReceipt`](../../src/domain/turn_receipt.rs), service JSON-RPC multiplexé via stdio et client natif AppKit pour macOS. |

---

## Architecture Joyzoning en 5 Zones (`82` Modules Rust · `300+` Fichiers)

| Zone | Chemin Physique | Responsabilités et Invariants |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` modules) | Primitives feuilles sans état : hachage, budget de tokens, réparation JSON, masquage de secrets, Seatbelt, garde shell, correctifs flous et géométrie spatiale. **Zéro import montant, zéro état global mutable.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` modules) | Entités pures et 64 traits formels `I<Name>`, postures, catalogue de tarification, spécifications de prompts, allocateur arène, arbre de compétences. **Strictement 0% d'E/S, maximum 3 structures par fichier.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` modules) | Adaptateurs concrets d'E/S : HTTP et protocoles filaires pour 61 fournisseurs, stockage VFS à 2 phases, instantanés CAS SHA-256, magasin comportemental et exécution en bac à sable. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` modules) | Orchestration des cas d'usage : machine à états `AgentEngine`, ordonnanceur d'outils par fronts d'ondes, moteur spatial, inspecteur de contexte et tests de vol. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Présentation et intention utilisateur : interface CLI, service de travail JSON-RPC stdio, application native AppKit pour macOS et racines de composition. |

---

## Démarrage Rapide et Vérification (Quick Start)

```bash
# 1. Exécuter les tests unitaires Rust, spatiaux, d'utilisation machine et d'intégration
cargo test

# 2. Exécuter la suite de tests natifs macOS AppKit
cd native && swift test && cd ..

# 3. Exécuter la suite de vol autonome et le diagnostic d'intégrité
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Exécuter l'audit strict d'architecture Joyzoning et la vérification des liens
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
