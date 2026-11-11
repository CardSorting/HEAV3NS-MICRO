<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Ambiente de Inteligência Soberana (Sovereign Intelligence Runtime)

**Ambiente de inteligência em Rust hipercomprimido e soberano em arquitetura, motor de ação espacial de duplo invólucro e IDE nativo em AppKit para macOS (`HEAV3NS-CODE`)** — estruturado sob a arquitetura de 5 zonas Joyzoning da **HEAV3NS-JOY** (`100%` Índice de Estabilidade, 0 violações, todos os arquivos do workspace no Tier 1 Optimal `< 800` linhas).

<p align="center">
  <a href="../../doc.md">Carta Soberana (doc.md)</a> ·
  <a href="../../AGENTS.md">Protocolo de Agentes (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Mente Central (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Roteiro</a> ·
  <a href="../README.md">Central de Documentação</a> ·
  <a href="../papers/ascendant-intelligence.md">Artigo de Inteligência Ascendente</a> ·
  <a href="../PROJECT_MAP.md">Mapa do Projeto</a> ·
  <a href="../CONSTITUTION.md">Restrições Constitucionais</a> ·
  <a href="../../VALIDATION_REPORT.md">Relatório de Validação</a> ·
  <a href="../../native/PRODUCT.md">Especificação Nativa</a>
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

## Os Sete Pilares Soberanos (The Seven Sovereign Pillars)

> *"Não é um agente. Não é um enxame. Não é um sistema operacional. É uma civilização computacional de inteligência cooperativa — onde o sistema não apenas executa instruções: ele compreende o ambiente, aprende a estrutura, retém a experiência e age com intenção deliberada."* — [`doc.md`](../../doc.md)

| Pilar | Designação | Implementação Central |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **O Ambiente de Inteligência Destilado** | Máquina de estados ReAct multi-turnos [`AgentEngine`](../../src/core/engine.rs), especificação de prompts determinística para cache KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspeção de contexto [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), analisador autorreparável em 3 fases [`ToolCallArgParser`](../../src/domain/tool_parser.rs) e suíte de voo `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Percepção Estrutural e Compreensão do Ambiente** | Análise de relações estruturais e classificação de affordances em [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), observações em [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) e auditorias AST estritas com `spider.check`. |
| **3. `REWIND`** | **Cognição Temporal, Continuidade e Recuperação Determinística** | Alocador de memória em arena de 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) com rebobinamento $O(1)$, CAS de instantâneos SHA-256 em Rust puro [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (com `/undo`, `/restore` protegidos contra travamentos e `/diff` sem Git), verificação em 2 fases [`SessionVfs`](../../src/infrastructure/vfs_store.rs), livro de transações com reversão [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) e pilha de 32 níveis em [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Conhecimento Persistente, Memória e Evolução Histórica** | Armazenamento de conhecimento comportamental com calibração de confiança por suavização de Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), memória híbrida TF-IDF [`MemoryStore`](../../src/domain/memory.rs) e catálogo de habilidades em DAG verificado com Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Inteligência Coletiva Distribuída** | Consulta paralela de 5 especialistas (percepção visual, análise estrutural, navegação de fluxo, avaliação de riscos e recuperação + árbitro de conflitos) e formações topológicas [`Legion`](../../src/domain/swarm.rs) de 7 especialistas com reservas exclusivas. |
| **6. `COMPUTER USE`** | **Agência Corpórea: Observação, Interpretação, Ação e Adaptação** | Orquestrador em loop fechado de 9 etapas [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediador de ação de duplo invólucro [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Invólucro A Ancoragem Espacial + Invólucro B Garantia de Execução), rolagem automática fora de tela em [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) e `browser_use` de confiança zero. |
| **7. `THE HARNESS`** | **O Substrato Computacional Unificador** | 31 ferramentas nativas (busca LRU de 8 slots e execução em lote de 32 passos), agendador por frentes de onda em DAG, 5 posturas de execução [`ExecutionPosture`](../../src/domain/types/posture_types.rs), sandbox Seatbelt no macOS, comprovante de turno [`TurnReceipt`](../../src/domain/turn_receipt.rs), serviço JSON-RPC multiplexado em stdio e cliente nativo AppKit para macOS. |

---

## Arquitetura de 5 Zonas Joyzoning (`82` Módulos Rust · `300+` Arquivos)

| Zona | Caminho Físico | Responsabilidade e Invariantes |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` módulos) | Primitivas folha sem estado: hash, orçamento de tokens, reparo de JSON, ofuscação de segredos, Seatbelt, guarda de shell, patches aproximados e geometria espacial. **Zero importações ascendentes, zero estado global mutável.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` módulos) | Entidades puras e 64 traits formais `I<Name>`, posturas, catálogo de preços, especificações de prompts, arena, árvore de habilidades. **Estritamente 0% I/O, máximo de 3 structs por arquivo.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` módulos) | Adaptadores concretos de E/S: HTTP e dialetos de rede para 61 provedores, armazenamento VFS em 2 fases, instantâneos CAS, armazenamento comportamental e execução segura em sandbox. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` módulos) | Orquestração de casos de uso: máquina de estados `AgentEngine`, agendador em frentes de onda, motor espacial, inspetor de contexto e testes de voo. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Apresentação e intenção do usuário: CLI, trabalhador JSON-RPC via stdio, aplicação nativa AppKit para macOS e pontos de composição. |

---

## Início Rápido e Verificação (Quick Start)

```bash
# 1. Executar testes unitários de Rust, espaciais, de uso de computador e integração
cargo test

# 2. Executar a suíte de testes nativa de macOS em AppKit
cd native && swift test && cd ..

# 3. Executar voos autônomos e diagnóstico de integridade
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Auditoria de arquitetura estrita Joyzoning e verificação de documentação
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
