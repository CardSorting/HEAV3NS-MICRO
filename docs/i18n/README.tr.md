<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Egemen Zeka Çalışma Zamanı (Sovereign Intelligence Runtime)

**Hiper-sıkıştırılmış, mimari açıdan egemen Rust zeka çalışma zamanı, çift katmanlı uzamsal eylem motoru ve yerel macOS AppKit IDE (`HEAV3NS-CODE`)** — **HEAV3NS-JOY** 5 bölgeli Joyzoning mimarisi altında yapılandırılmıştır (`%100` Kararlılık İndeksi, 0 ihlal, tüm çalışma alanı dosyaları Tier 1 Optimal `< 800` satır altında).

<p align="center">
  <a href="../../doc.md">Egemenlik Bildirgesi (doc.md)</a> ·
  <a href="../../AGENTS.md">Ajan Protokolü (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Temel Ruh (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Yol Haritası</a> ·
  <a href="../README.md">Belgelendirme Merkezi</a> ·
  <a href="../papers/ascendant-intelligence.md">Yükselen Zeka Bildirisi</a> ·
  <a href="../PROJECT_MAP.md">Proje Haritası</a> ·
  <a href="../CONSTITUTION.md">Anayasal Sınırlar</a> ·
  <a href="../../VALIDATION_REPORT.md">Doğrulama Raporu</a> ·
  <a href="../../native/PRODUCT.md">Yerel Ürün Spesifikasyonu</a>
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

## Yedi Egemen Sütun (The Seven Sovereign Pillars)

> *"Sıradan bir ajan değil. Bir sürü değil. Bir işletim sistemi değil. Bu, iş birliği yapan zekanın bilişimsel bir medeniyetidir — sistem sadece talimatları yerine getirmekle kalmaz: çevresini anlar, yapısını öğrenir, deneyimini hatırlar ve kesin bir niyetle hareket eder."* — [`doc.md`](../../doc.md)

| Sütun | Tanım | Temel Uygulama |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Damıtılmış Zeka Çalışma Zamanı** | Çok turlu ReAct durum makinesi [`AgentEngine`](../../src/core/engine.rs), KV önbelleği için deterministik istem belirtimi [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), bağlam denetleyicisi [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), 3 aşamalı kendi kendini onaran araç ayrıştırıcısı [`ToolCallArgParser`](../../src/domain/tool_parser.rs) ve `FT-01..FT-05` uçuş paketi [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Yapısal Algı ve Çevre Anlayışı** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) dosyasında yapısal ilişki analizi ve olanak sınıflandırması, çevre gözlemleri [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) ve `spider.check` ile katı AST denetimleri. |
| **3. `REWIND`** | **Zamansal Biliş, Süreklilik ve Deterministik Kurtarma** | $O(1)$ anlık görüntü geri sarma özelliğine sahip 16 MiB arena bellek ayırıcısı [`ArenaAllocator`](../../src/domain/arena.rs), saf Rust SHA-256 çalışma alanı CAS anlık görüntüleri [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (engellemeye karşı korumalı `/undo`, `/restore` ve Git'ten bağımsız `/diff`), 2 aşamalı [`SessionVfs`](../../src/infrastructure/vfs_store.rs) taban doğrulaması, geri alma özellikli işlem günlüğü [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) ve 32 derinlikli [`VirtualDisplayState`](../../src/domain/computer_use.rs) geri alma/yineleme yığını. |
| **4. `BROCCOLI`** | **Kalıcı Bilgi, Bellek ve Tarihsel Evrim** | Laplace yumuşatmalı güven kalibrasyonuna sahip davranışsal bilgi deposu [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), hibrit TF-IDF bellek deposu [`MemoryStore`](../../src/domain/memory.rs) ve Kahn topolojik doğrulamalı beceri ağacı kataloğu [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) DAG. |
| **5. `SYNTHETIC SWARM`** | **Dağıtılmış Kolektif Zeka** | 5 uzmanlı paralel danışma (görsel algılayıcı, yapısal analist, iş akışı rehberi, risk değerlendirici, kurtarma uzmanı + çatışma hakemi) ve özel dosya rezervasyonlarına sahip 7 uzmanlı [`Legion`](../../src/domain/swarm.rs) topoloji dizilimleri. |
| **6. `COMPUTER USE`** | **Bedenleşmiş Eylemlilik: Gözlem, Yorumlama, Eylem ve Uyarlanma** | 9 aşamalı kapalı döngü orkestratör [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), çift sarmallı uzamsal eylem arabulucusu [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Sarma A Uzamsal Çapalama + Sarma B İcra Güvencesi), ekran dışı otomatik kaydırma [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) ve sıfır güvenli `browser_use`. |
| **7. `THE HARNESS`** | **Her Şeyi Birbirine Bağlayan Hesaplamalı Alt Taban** | 31 yerel araç (8 yuvalı LRU arama ve 32 adımlı toplu çalıştırma), DAG dalga cephesi eşzamanlılık zamanlayıcısı, 5 yürütme duruşu [`ExecutionPosture`](../../src/domain/types/posture_types.rs), macOS Seatbelt koruma alanı, denetim makbuzu [`TurnReceipt`](../../src/domain/turn_receipt.rs), çoklu stdio JSON-RPC hizmeti ve yerel macOS AppKit istemcisi. |

---

## 5 Bölgeli Joyzoning Mimarisi (`82` Rust Modülü · `300+` Dosya)

| Bölge | Fiziksel Yol | Sorumluluk ve Değişmezler |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` modül) | Durumsuz yaprak ilkeller: karma, belirteç bütçesi, JSON onarımı, gizli bilgi maskeleme, Seatbelt, kabuk koruması, belirsiz yamalar ve uzamsal geometri. **Yukarı yönlü içe aktarma kesinlikle yasaktır, değişken genel durum sıfırdır.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` modül) | Saf varlıklar ve 64 resmi `I<Name>` özelliği, duruşlar, model fiyatlandırma kataloğu, istem özellikleri, arena bellek ayırıcısı, beceri ağacı. **Kesinlikle %0 G/Ç, dosya başına en fazla 3 yapı.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` modül) | Somut G/Ç bağdaştırıcıları: 61 sağlayıcı için HTTP ve ağ protokolleri, 2 aşamalı VFS depolaması, SHA-256 CAS anlık görüntüleri, davranış deposu ve korumalı alanda kabuk çalıştırma. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` modül) | Kullanım senaryosu orkestrasyonu: `AgentEngine` durum makinesi, dalga cephesi zamanlayıcısı, uzamsal motor, bağlam denetleyicisi ve uçuş testleri. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Sunum ve kullanıcı niyeti: CLI arayüzü, stdio JSON-RPC çalışanı, yerel masaüstü AppKit uygulaması ve bileşim kökleri. |

---

## Hızlı Başlangıç ve Doğrulama (Quick Start)

```bash
# 1. Tüm Rust birim, uzamsal, bilgisayar kullanımı ve entegrasyon testlerini çalıştırın
cargo test

# 2. Yerel macOS AppKit test paketini çalıştırın
cd native && swift test && cd ..

# 3. Otonom uçuş testlerini ve sistem sağlığı teşhisini çalıştırın
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Katı Joyzoning mimari denetimini ve belge bağlantılarını kontrol edin
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
