<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — Môi Trường Trí Tuệ Chủ Quyền (Sovereign Intelligence Runtime)

**Môi trường thực thi trí tuệ Rust siêu nén và tự chủ kiến trúc, động cơ hành động không gian bọc kép và IDE macOS AppKit nguyên bản (`HEAV3NS-CODE`)** — tuân thủ cấu trúc 5 phân vùng Joyzoning của **HEAV3NS-JOY** (Chỉ số ổn định `100%`, 0 vi phạm, toàn bộ tệp thuộc Tier 1 Optimal `< 800` dòng).

<p align="center">
  <a href="../../doc.md">Hiến Chương Chủ Quyền (doc.md)</a> ·
  <a href="../../AGENTS.md">Giao Thức Tác Tử (AGENTS.md)</a> ·
  <a href="../../SOUL.md">Tinh Thần Cốt Lõi (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">Lộ Trình</a> ·
  <a href="../README.md">Trung Tâm Tài Liệu</a> ·
  <a href="../papers/ascendant-intelligence.md">Bạch Thư Trí Tuệ Thăng Hoa</a> ·
  <a href="../PROJECT_MAP.md">Sơ Đồ Dự Án</a> ·
  <a href="../CONSTITUTION.md">Ràng Buộc Hiến Pháp</a> ·
  <a href="../../VALIDATION_REPORT.md">Báo Cáo Kiểm Định</a> ·
  <a href="../../native/PRODUCT.md">Đặc Tả Bản Địa</a>
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

## Bảy Trụ Cột Chủ Quyền (The Seven Sovereign Pillars)

> *"Không phải một tác tử đơn lẻ. Không phải một bầy đàn. Không phải một hệ điều hành. Đây là một nền văn minh điện toán của trí tuệ hợp tác — nơi hệ thống không chỉ thực thi các chỉ thị: nó thấu hiểu môi trường, học hỏi cấu trúc, ghi nhớ trải nghiệm và hành động với chủ ý kiên định."* — [`doc.md`](../../doc.md)

| Trụ Cột | Danh Hiệu | Triển Khai Cốt Lõi |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **Môi Trường Trí Tuệ Tinh Luyện** | Máy trạng thái ReAct nhiều lượt [`AgentEngine`](../../src/core/engine.rs), đặc tả câu lệnh tiền tố KV cache quyết định luận [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), thanh tra ngữ cảnh [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), bộ phân tích lệnh tự chữa lành 3 giai đoạn [`ToolCallArgParser`](../../src/domain/tool_parser.rs) và chuỗi bay thử `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Nhận Thức Cấu Trúc và Thấu Hiểu Môi Trường** | Phân tích quan hệ cấu trúc trong [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), quan sát môi trường [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) và kiểm toán AST nghiêm ngặt với `spider.check`. |
| **3. `REWIND`** | **Nhận Thức Thời Gian, Tính Liên Tục và Phục Hồi Quyết Định Luận** | Bộ cấp phát bộ nhớ arena 16 MiB [`ArenaAllocator`](../../src/domain/arena.rs) với khả năng hoàn tác ảnh chụp $O(1)$, CAS ảnh chụp SHA-256 thuần Rust [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (với `/undo`, `/restore` chống tắc nghẽn và `/diff` độc lập với Git), xác minh cơ sở 2 giai đoạn [`SessionVfs`](../../src/infrastructure/vfs_store.rs), nhật ký giao dịch hoàn tác [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) và ngăn xếp hoàn tác 32 cấp [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **Tri Thức Bền Vững, Bộ Nhớ và Tiến Hóa Lịch Sử** | Kho tri thức hành vi với hiệu chuẩn độ tin cậy mượt Laplace [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), bộ nhớ lai TF-IDF [`MemoryStore`](../../src/domain/memory.rs) và danh mục kỹ năng DAG đã xác minh bằng thuật toán Kahn [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **Trí Tuệ Tập Thể Phân Tán** | Tham vấn song song 5 chuyên gia (nhận thức thị giác, phân tích cấu trúc, điều hướng quy trình, đánh giá rủi ro và phục hồi + trọng tài xung đột) cùng đội hình topo 7 chuyên gia [`Legion`](../../src/domain/swarm.rs). |
| **6. `COMPUTER USE`** | **Hành Động Hiện Thân: Quan Sát, Diễn Giải, Hành Động và Thích Ứng** | Bộ điều phối vòng kín 9 giai đoạn [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), bộ hòa giải hành động bọc kép [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (Vỏ A Neo Không Gian + Vỏ B Đảm Bảo Thực Thi), tự động cuộn ngoài màn hình trong [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) và `browser_use` không ủy thác. |
| **7. `THE HARNESS`** | **Chất Nền Điện Toán Hợp Nhất Mọi Thứ** | 31 công cụ bản địa (tìm kiếm LRU 8 khe và thực thi lô 32 bước), bộ lập lịch mặt sóng DAG, 5 tư thế thực thi [`ExecutionPosture`](../../src/domain/types/posture_types.rs), hộp cát Seatbelt macOS, biên lai kiểm toán [`TurnReceipt`](../../src/domain/turn_receipt.rs), máy chủ stdio JSON-RPC đa kênh và ứng dụng macOS AppKit nguyên bản. |

---

## Kiến Trúc 5 Phân Vùng Joyzoning (`82` Mô-đun Rust · `300+` Tệp)

| Phân Vùng | Đường Dẫn Vật Lý | Trách Nhiệm và Bất Biến |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` mô-đun) | Các hàm nguyên thủy lá không trạng thái: băm, ngân sách token, vá chuỗi JSON, ẩn giấu bí mật, Seatbelt, bộ bảo vệ shell, vá mờ và hình học không gian. **Không nhập ngược lên trên, không trạng thái toàn cục có thể biến đổi.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` mô-đun) | Thực thể thuần túy và 64 trait `I<Name>` chính thức, tư thế thực thi, bảng giá mô hình, quy chuẩn câu lệnh, bộ cấp phát arena, cây kỹ năng. **0% I/O nghiêm ngặt, tối đa 3 struct mỗi tệp.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` mô-đun) | Bộ điều hợp I/O cụ thể: HTTP và giao thức mạng cho 61 nhà cung cấp, lưu trữ VFS 2 giai đoạn, ảnh chụp CAS SHA-256, kho tri thức hành vi và thực thi lệnh trong hộp cát. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` mô-đun) | Điều phối nghiệp vụ: máy trạng thái `AgentEngine`, bộ lập lịch mặt sóng, động cơ không gian, thanh tra ngữ cảnh và các chuyến bay thử nghiệm. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | Tầng hiển thị và ý định người dùng: giao diện dòng lệnh CLI, worker stdio JSON-RPC, ứng dụng AppKit macOS và điểm neo khởi tạo. |

---

## Bắt Đầu Nhanh và Xác Minh (Quick Start)

```bash
# 1. Chạy toàn bộ kiểm thử đơn vị Rust, kiểm thử không gian và tích hợp
cargo test

# 2. Chạy bộ kiểm thử AppKit macOS nguyên bản
cd native && swift test && cd ..

# 3. Chạy các chuyến bay tự chủ và kiểm tra sức khỏe hệ thống
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. Kiểm toán kiến trúc nghiêm ngặt Joyzoning và kiểm tra liên kết
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
