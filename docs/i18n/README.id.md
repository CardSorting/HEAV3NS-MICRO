<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Runtime Inteligensi Berdaulat (Sovereign Intelligence Runtime)

**Runtime inteligensi Rust yang sangat terkompresi dan berdaulat secara arsitektur, mesin aksi spasial berlapis ganda, dan IDE macOS AppKit bawaan (`HEAV3NS-CODE`)** — terstruktur di bawah arsitektur Joyzoning 5-zona **HEAV3NS-JOY** (Indeks Stabilitas `100%`, 0 pelanggaran, semua berkas berada dalam Tier 1 Optimal `< 800` baris).

<p align="center">
  <a href="../../doc.md">Piagam Kedaulatan (doc.md)</a> ·
  <a href="../../AGENTS.md">Protokol Agen (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Jiwa Inti (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Peta Jalan</a> ·
  <a href="../README.md">Pusat Dokumentasi</a> ·
  <a href="../papers/ascendant-intelligence.md">Kertas Putih Kecerdasan Meningkat</a> ·
  <a href="../PROJECT_MAP.md">Peta Proyek</a> ·
  <a href="../CONSTITUTION.md">Batasan Konstitusi</a> ·
  <a href="../../VALIDATION_REPORT.md">Laporan Validasi</a> ·
  <a href="../../native/PRODUCT.md">Spesifikasi Asli</a>
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

## Tujuh Pilar Kedaulatan (The Seven Sovereign Pillars)

> *"Bukan sekadar agen. Bukan kawanan biasa. Bukan sistem operasi. Ini adalah peradaban komputasi kecerdasan yang saling bekerja sama — di mana sistem tidak hanya menjalankan instruksi: ia memahami lingkungannya, mempelajari strukturnya, mengingat pengalamannya, dan bertindak dengan niat yang jelas."* — [`doc.md`](../../doc.md)

| Pilar | Sebutan | Implementasi Utama |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Runtime Inteligensi yang Disuling** | Mesin status ReAct multi-putaran [`AgentEngine`](../../src/core/engine.rs), spesifikasi prompt deterministik cache KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), inspektur konteks [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), parser pemanggilan alat 3 tahap yang memulihkan diri [`ToolCallArgParser`](../../src/domain/tool_parser.rs), dan uji terbang `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Persepsi Struktural dan Pemahaman Lingkungan** | Analisis hubungan struktural di [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), observasi lingkungan [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs), dan audit ketat AST dengan `spider.check`. |
| **3. `REWIND`** | **Kognisi Temporal, Kontinuitas, dan Pemulihan Deterministik** | Pengalokasi memori arena 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) dengan pemutaran balik snapshot $O(1)$, CAS snapshot SHA-256 murni Rust [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (dengan proteksi terhadap blokir pada `/undo`, `/restore`, dan `/diff` non-Git), verifikasi 2 fase [`SessionVfs`](../../src/infrastructure/vfs_store.rs), jurnal transaksi [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) dengan rollback, dan tumpukan undo/redo 32 kedalaman [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Pengetahuan Persisten, Memori, dan Evolusi Historis** | Penyimpanan pengetahuan perilaku dengan kalibrasi Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), memori hibrida TF-IDF [`MemoryStore`](../../src/domain/memory.rs), dan katalog keterampilan DAG yang divalidasi secara topologis Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Kecerdasan Kolektif Terdistribusi** | Konsultasi paralel 5 spesialis (persepsi visual, analisis struktur, navigasi alur kerja, penilaian risiko, pemulihan + penengah konflik) dan formasi topologi 7 spesialis [`Legion`](../../src/domain/swarm.rs). |
| **6. `COMPUTER USE`** | **Agensi Nyata: Observasi, Interpretasi, Tindakan, dan Adaptasi** | Pengatur loop tertutup 9 tahap [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), mediator aksi spasial berlapis ganda [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Lapisan A Landasan Spasial + Lapisan B Jaminan Eksekusi), gulir otomatis di luar layar [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs), dan `browser_use` tanpa kepercayaan. |
| **7. `THE HARNESS`** | **Substrat Komputasi yang Mengikat Segalanya** | 31 perkakas bawaan (pencarian LRU 8 slot dan eksekusi batch 32 langkah), penjadwal gelombang konkurensi DAG, 5 postur eksekusi [`ExecutionPosture`](../../src/domain/types/posture_types.rs), kotak pasir macOS Seatbelt, tanda terima audit [`TurnReceipt`](../../src/domain/turn_receipt.rs), layanan stdio JSON-RPC termultipleks, dan klien AppKit macOS asli. |

---

## Arsitektur 5-Zona Joyzoning (`82` Modul Rust · `300+` Berkas)

| Zona | Jalur Fisik | Tanggung Jawab dan Invarian |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` modul) | Primitif daun tanpa status: hash, anggaran token, perbaikan JSON, penyamaran rahasia, Seatbelt, penjaga shell, patch fuzzy, dan geometri spasial. **Nol impor ke atas, nol mutasi status global.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` modul) | Entitas murni dan 64 trait `I<Name>` resmi, postur eksekusi, katalog harga model, spesifikasi prompt, pengalokasi arena, pohon keterampilan. **Ketat 0% I/O, maksimum 3 struct per berkas.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` modul) | Adaptor I/O konkret: HTTP dan dialek protokol kawat untuk 61 penyedia, penyimpanan VFS 2 fase, snapshot CAS SHA-256, penyimpanan perilaku, dan eksekusi kotak pasir. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` modul) | Orkestrasi kasus penggunaan: mesin status `AgentEngine`, penjadwal perkakas, mesin interaksi spasial, inspektur konteks, dan rangkaian uji terbang. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Presentasi dan niat pengguna: CLI, pekerja stdio JSON-RPC, aplikasi AppKit macOS asli, dan akar komposisi. |

---

## Mulai Cepat dan Verifikasi (Quick Start)

```bash
# 1. Jalankan pengujian unit Rust, spasial, penggunaan komputer, dan integrasi
cargo test

# 2. Jalankan rangkaian pengujian asli macOS AppKit
cd native && swift test && cd ..

# 3. Jalankan pengujian penerbangan otonom dan diagnostik kesehatan
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Audit arsitektur ketat Joyzoning dan verifikasi tautan
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
