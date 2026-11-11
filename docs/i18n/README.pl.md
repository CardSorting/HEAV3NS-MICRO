<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Suwerenne Środowisko Inteligencji (Sovereign Intelligence Runtime)

**Hiper-skompresowane, suwerenne architektonicznie środowisko inteligencji w języku Rust, dwuwarstwowy silnik działań przestrzennych oraz natywne środowisko IDE macOS AppKit (`HEAV3NS-CODE`)** — ustrukturyzowane w 5-strefowej architekturze Joyzoning **HEAV3NS-JOY** (Wskaźnik Stabilności `100%`, 0 naruszeń, wszystkie pliki w Tier 1 Optimal `< 800` linii).

<p align="center">
  <a href="../../doc.md">Karta Suwerenna (doc.md)</a> ·
  <a href="../../AGENTS.md">Protokół Agentów (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Główny Duch (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Mapa Drogowa</a> ·
  <a href="../README.md">Centrum Dokumentacji</a> ·
  <a href="../papers/ascendant-intelligence.md">Biała Księga Wyniesionej Inteligencji</a> ·
  <a href="../PROJECT_MAP.md">Mapa Projektu</a> ·
  <a href="../CONSTITUTION.md">Ramy Konstytucyjne</a> ·
  <a href="../../VALIDATION_REPORT.md">Raport Walidacji</a> ·
  <a href="../../native/PRODUCT.md">Specyfikacja Natywna</a>
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

## Siedem Suwerennych Filarów (The Seven Sovereign Pillars)

> *"Nie zwykły agent. Nie rój. Nie system operacyjny. To obliczeniowa cywilizacja współpracującej inteligencji — gdzie system nie tylko wykonuje polecenia: rozumie swoje otoczenie, uczy się jego struktury, pamięta doświadczenia i działa z jasną intencją."* — [`doc.md`](../../doc.md)

| Filar | Oznaczenie | Kluczowa Implementacja |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Destylowane Środowisko Inteligencji** | Wieloturowy automat skończony ReAct [`AgentEngine`](../../src/core/engine.rs), deterministyczna dla pamięci podręcznej KV specyfikacja promptów [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspektor kontekstu [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), 3-fazowy samoleczący parser narzędzi [`ToolCallArgParser`](../../src/domain/tool_parser.rs) oraz zestaw lotów `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Percepcja Strukturalna i Zrozumienie Otoczenia** | Analiza relacji strukturalnych w [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), obserwacje otoczenia [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) i rygorystyczne audyty AST z `spider.check`. |
| **3. `REWIND`** | **Poznanie Czasowe, Ciągłość i Deterministyczne Przywracanie** | 16-MiB alokator pamięci arena [`ArenaAllocator`](../../src/domain/arena.rs) z natychmiastowym cofaniem migawek $O(1)$, czysty Rust SHA-256 CAS migawek przestrzeni roboczej [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (z zabezpieczeniem przed blokowaniem dla `/undo`, `/restore` oraz `/diff` niezależnym od Gita), 2-fazowa weryfikacja bazy [`SessionVfs`](../../src/infrastructure/vfs_store.rs), dziennik transakcji [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) z wycofywaniem i 32-poziomowy stos cofania/ponawiania w [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Trwała Wiedza, Pamięć i Ewolucja Historyczna** | Magazyn wiedzy behawioralnej z kalibracją ufności metodą Laplace'a [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), hybrydowa pamięć TF-IDF [`MemoryStore`](../../src/domain/memory.rs) oraz zweryfikowany topologicznie przez Kahna katalog umiejętności DAG [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Rozproszona Inteligencja Zbiorowa** | Równoległa konsultacja 5 specjalistów (percepcja wizualna, analiza strukturalna, nawigacja procesów, ocena ryzyka, odzyskiwanie + arbiter konfliktów) oraz formacje topologiczne 7 specjalistów [`Legion`](../../src/domain/swarm.rs). |
| **6. `COMPUTER USE`** | **Ucieleśniona Sprawczość: Obserwacja, Interpretacja, Działanie i Adaptacja** | 9-fazowy koordynator w pętli zamkniętej [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediator działań w podwójnej osłonie [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Osłona A Zakotwiczenie Przestrzenne + Osłona B Zapewnienie Wykonania), automatyczne przewijanie poza ekranem w [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) oraz `browser_use` o zerowym zaufaniu. |
| **7. `THE HARNESS`** | **Spajający Substrat Obliczeniowy** | 31 narzędzi natywnych (wyszukiwarka LRU z 8 gniazdami i wykonywanie wsadowe do 32 kroków), harmonogram fali DAG, 5 postaw wykonawczych [`ExecutionPosture`](../../src/domain/types/posture_types.rs), piaskownica macOS Seatbelt, pokwitowanie audytu [`TurnReceipt`](../../src/domain/turn_receipt.rs), zmultipleksowana usługa stdio JSON-RPC i natywny klient macOS AppKit. |

---

## Architektura 5 Stref Joyzoning (`82` Moduły Rust · `300+` Plików)

| Strefa | Ścieżka Fizyczna | Odpowiedzialność i Niezmienniki |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` modułów) | Bezstanowe prymitywy liściaste: haszowanie, budżet tokenów, naprawa JSON, maskowanie sekretów, Seatbelt, ochrona powłoki shell, łatanie rozmyte i geometria przestrzenna. **Zero importów w górę, zero modyfikowalnego stanu globalnego.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` modułów) | Czyste encje i 64 formalne cechy `I<Name>`, postawy, cennik modeli, specyfikacje promptów, alokator arena, drzewo umiejętności. **Ścisłe 0% I/O, maksymalnie 3 struktury na plik.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` modułów) | Konkretne adaptery wejścia/wyjścia: HTTP i protokoły sieciowe dla 61 dostawców, 2-fazowy VFS, migawki CAS SHA-256, magazyn behawioralny i bezpieczne wykonywanie w piaskownicy. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` modułów) | Orkiestracja logiki biznesowej: maszyna stanów `AgentEngine`, harmonogram fali, silnik przestrzenny, inspektor kontekstu i testy w locie. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Prezentacja i intencje użytkownika: interfejs CLI, pracownik stdio JSON-RPC, aplikacja pulpitowa AppKit dla macOS i korzenie kompozycji. |

---

## Szybki Start i Weryfikacja (Quick Start)

```bash
# 1. Uruchom testy jednostkowe Rust, przestrzenne, obsługi komputera i integracyjne
cargo test

# 2. Uruchom zestaw testów natywnego klienta macOS AppKit
cd native && swift test && cd ..

# 3. Uruchom loty autonomiczne i diagnostykę systemową
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Ścisły audyt architektoniczny Joyzoning oraz weryfikacja linków
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
