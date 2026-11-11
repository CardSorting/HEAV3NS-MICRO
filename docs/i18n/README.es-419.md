<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Entorno de Inteligencia Soberana (Sovereign Intelligence Runtime)

**Entorno de inteligencia en Rust arquitectónicamente soberano e hipercomprimido, motor de acción espacial con doble envoltura e IDE nativo para macOS en AppKit (`HEAV3NS-CODE`)** — estructurado bajo la arquitectura de 5 zonas Joyzoning de **HEAV3NS-JOY** (`100%` Índice de Estabilidad, 0 violaciones, todos los archivos del repositorio en Tier 1 Óptimo `< 800` líneas).

<p align="center">
  <a href="../../doc.md">Carta Soberana (doc.md)</a> ·
  <a href="../../AGENTS.md">Protocolo de Agentes (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Mente Central (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Hoja de Ruta</a> ·
  <a href="../README.md">Centro de Documentación</a> ·
  <a href="../papers/ascendant-intelligence.md">Artículo de Inteligencia Ascendente</a> ·
  <a href="../PROJECT_MAP.md">Mapa del Proyecto</a> ·
  <a href="../CONSTITUTION.md">Constitución</a> ·
  <a href="../../VALIDATION_REPORT.md">Informe de Validación</a> ·
  <a href="../../native/PRODUCT.md">Especificación Nativa</a>
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

## Los Siete Pilares Soberanos (The Seven Sovereign Pillars)

> *"No es un agente. No es un enjambre. No es un sistema operativo. Es una civilización computacional de inteligencia cooperativa — donde el sistema no se limita a ejecutar instrucciones: comprende su entorno, aprende su estructura, recuerda su experiencia y actúa con intención fundada."* — [`doc.md`](../../doc.md)

| Pilar | Designación | Implementación Principal |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **El Entorno de Inteligencia Destilado** | Máquina de estados ReAct multi-turno [`AgentEngine`](../../src/core/engine.rs), especificación de prompts determinista para caché KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspección de contexto [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), analizador auto-reparable de 3 fases [`ToolCallArgParser`](../../src/domain/tool_parser.rs) y suite de vuelo `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Percepción Estructural y Comprensión del Entorno** | Análisis de relaciones y clasificación de posibilidades en [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), observaciones en [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) y auditoría AST estricta con `spider.check`. |
| **3. `REWIND`** | **Cognición Temporal, Continuidad y Recuperación Determinista** | Asignador de memoria en arena de 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) con retroceso $O(1)$, CAS de instantáneas SHA-256 en Rust puro [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (con `/undo`, `/restore` protegidos contra bloqueos y `/diff` independiente de Git), verificación en 2 fases con [`SessionVfs`](../../src/infrastructure/vfs_store.rs), libro de transacciones con reversión [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) y pila de deshacer/rehacer de 32 niveles en [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Conocimiento Persistente, Memoria y Evolución Histórica** | Almacén de conocimiento conductual con calibración de confianza por suavizado de Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), memoria híbrida TF-IDF [`MemoryStore`](../../src/domain/memory.rs) y catálogo de habilidades en DAG verificado con Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Inteligencia Colectiva Distribuida** | Consulta paralela de 5 especialistas (percepción visual, análisis estructural, navegación de flujo, evaluación de riesgos y recuperación + resolución de conflictos) y formaciones topológicas [`Legion`](../../src/domain/swarm.rs) de 7 especialistas con reservas exclusivas. |
| **6. `COMPUTER USE`** | **Agencia Corpórea: Observación, Interpretación, Acción y Adaptación** | Orquestador en bucle cerrado de 9 etapas [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediador con doble envoltura [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Envoltura A Anclaje Espacial + Envoltura B Garantía de Ejecución), auto-desplazamiento fuera de pantalla en [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) y `browser_use` de confianza cero. |
| **7. `THE HARNESS`** | **El Sustrato Computacional Integrador** | 31 herramientas nativas (búsqueda LRU de 8 ranuras y ejecución por lotes de 32 pasos), planificador concurrente DAG por frentes de onda, 5 posturas de ejecución [`ExecutionPosture`](../../src/domain/types/posture_types.rs), sandbox Seatbelt en macOS, comprobante de turno [`TurnReceipt`](../../src/domain/turn_receipt.rs), servicio multiplexado JSON-RPC en stdio y cliente nativo AppKit para macOS. |

---

## Arquitectura de 5 Zonas Joyzoning (`82` Módulos en Rust · `300+` Archivos)

| Zona | Ruta Física | Responsabilidad e Invariantes |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` módulos) | Primitivas de hoja sin estado: hashes, presupuesto de tokens, auto-reparación de JSON, ofuscación de secretos, Seatbelt, protección de shell, parches difusos y geometría espacial. **Cero importaciones ascendentes, cero estado global mutable.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` módulos) | Entidades puras y 64 rasgos formales `I<Name>`, posturas, catálogo de precios, especificaciones de prompts, arena, árbol de habilidades y modelos de enjambre. **Estrictamente 0% I/O, máximo 3 estructuras por archivo.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` módulos) | Adaptadores concretos de E/S: HTTP y dialectos de cable para 61 proveedores, almacenamiento VFS de 2 fases, instantáneas CAS, almacén conductual y ejecución en sandbox. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` módulos) | Orquestación de casos de uso: máquina de estados `AgentEngine`, programador por frentes de onda, motor espacial, inspector de contexto y pruebas de vuelo. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Presentación e intención del usuario: interfaz CLI, trabajador JSON-RPC por stdio, aplicación nativa AppKit para macOS y raíces de composición. |

---

## Inicio Rápido y Verificación (Quick Start)

```bash
# 1. Ejecutar pruebas unitarias de Rust, espaciales, de uso de computadora e integración
cargo test

# 2. Ejecutar la suite de pruebas nativas de macOS en AppKit
cd native && swift test && cd ..

# 3. Ejecutar suite de vuelo autónomo y diagnóstico de salud
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Auditoría de arquitectura estricta Joyzoning y verificación de enlaces
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
