<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — بيئة تشغيل الذكاء السيادي (Sovereign Intelligence Runtime)

**بيئة تشغيل ذكاء فائقة الضغط وذات سيادة معمارية بلغة Rust، ومحرك أفعال مكانية مزدوج الغلاف، وبيئة تطوير متكاملة أصيلة لنظام macOS باستخدام AppKit (`HEAV3NS-CODE`)** — مُهيكلة وفق معمارية مناطق Joyzoning الخمس من **HEAV3NS-JOY** (مؤشر استقرار بنسبة `100%`، وصفر انتهاكات، وجميع ملفات مساحة العمل ضمن المستوى الأمثل الأول Tier 1 Optimal أقل من 800 سطر).

<p align="center">
  <a href="../../doc.md">ميثاق السيادة (doc.md)</a> ·
  <a href="../../AGENTS.md">بروتوكول الوكلاء (AGENTS.md)</a> ·
  <a href="../../SOUL.md">الروح التأسيسية (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">خريطة الطريق</a> ·
  <a href="../README.md">مركز التوثيق</a> ·
  <a href="../papers/ascendant-intelligence.md">الورقة البيضاء للذكاء الصاعد</a> ·
  <a href="../PROJECT_MAP.md">خريطة المشروع</a> ·
  <a href="../CONSTITUTION.md">القيود الدستورية</a> ·
  <a href="../../VALIDATION_REPORT.md">تقرير التحقق</a> ·
  <a href="../../native/PRODUCT.md">المواصفات الأصيلة</a>
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

## الركائز السيادية السبع (The Seven Sovereign Pillars)

> *"ليس مجرد وكيل برمجي. ليس مجرد سرب. ليس نظام تشغيل. إنه حضارة حوسبية قائمة على تعاون الذكاء — حيث لا يكتفي النظام بتنفيذ التعليمات، بل يفهم بيئته، ويتعلم بنيتها، ويتذكر خبراته السابقة، ويتصرف بنية حاسمة."* — [`doc.md`](../../doc.md)

| الركيزة | المسمى | التطبيق الأساسي |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **بيئة تشغيل الذكاء المقطرة** | آلة الحالة ReAct متعددة الجولات [`AgentEngine`](../../src/core/engine.rs)، وتحديد موجه حتمي لذاكرة التخزين المؤقت KV [`SystemPromptSpec`](../../src/domain/prompt_spec.rs)، وفاحص السياق والميزانية [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs)، ومحلل استدعاء الأدوات ذاتي المعالجة ذو الـ 3 مراحل [`ToolCallArgParser`](../../src/domain/tool_parser.rs)، وحزمة اختبارات الطيران `FT-01..FT-05` [`flight_runner`](../../src/core/flight_runner.rs). |
| **2. `SPIDER`** | **الإدراك الهيكلي وفهم البيئة المحيطة** | تحليل العلاقات البنيوية وتصنيف الإمكانيات في [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs)، والملاحظات البيئية في [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs)، وتدقيق AST الصارم عبر `spider.check`. |
| **3. `REWIND`** | **الإدراك الزمني والاستمرارية والاستعادة الحتمية** | مخصص ذاكرة من نوع Arena بحجم 16 ميغابايت [`ArenaAllocator`](../../src/domain/arena.rs) مع تراجع فوري $O(1)$ لللقطات، ومخزن لقطات SHA-256 بلغة Rust الخالصة [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (مع حماية ضد الحظر لـ `/undo` و `/restore` وعمليات `/diff` المستقلة عن Git)، وتحقق ثنائي المرحلة لـ [`SessionVfs`](../../src/infrastructure/vfs_store.rs)، وسجل معاملات التراجع [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs)، ومكدس التراجع والإعادة بعمق 32 خطوة [`VirtualDisplayState`](../../src/domain/computer_use.rs). |
| **4. `BROCCOLI`** | **المعرفة الدائمة والذاكرة والتطور التاريخي** | مخزن المعرفة السلوكية بمعايرة ثقة لابلاس [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs)، والذاكرة الهجينة TF-IDF [`MemoryStore`](../../src/domain/memory.rs)، ودليل شجرة المهارات DAG المحقق طوبولوجياً عبر كاهن [`SkillTreeCatalog`](../../src/domain/skill_tree.rs). |
| **5. `SYNTHETIC SWARM`** | **الذكاء الجمعي الموزع** | استشارة متوازية لخمسة خبراء تخصصيين (الإدراك البصري، التحليل الهيكلي، ملاحة تدفق العمل، تقييم المخاطر، خبير الاسترداد + محكم النزاعات) وتشكيلات سرب [`Legion`](../../src/domain/swarm.rs) طوبولوجية لـ 7 خبراء مع حجز ملفات حصري. |
| **6. `COMPUTER USE`** | **الفاعلية المتجسدة: الملاحظة، التفسير، الفعل والتكيف** | منسق الحلقة المغلقة ذو الـ 9 مراحل [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs)، ووسيط الأفعال المكانية المزدوج الغلاف [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (الغلاف أ للرسو المكاني + الغلاف ب لضمان التنفيذ)، والتمرير التلقائي خارج الشاشة في [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs)، وأداة `browser_use` القائمة على مبدأ انعدام الثقة. |
| **7. `THE HARNESS`** | **الركيزة الحوسبية الموحدة لكل شيء** | 31 أداة أصيلة (بحث LRU بثماني فتحات وتنفيذ دفعات حتى 32 خطوة)، وجدول موجي تزامن لرسوم DAG، و5 وضعيات تنفيذية [`ExecutionPosture`](../../src/domain/types/posture_types.rs)، وعزل macOS Seatbelt الآمن، وإيصالات تدقيق دورة العمل [`TurnReceipt`](../../src/domain/turn_receipt.rs)، وخدمة stdio JSON-RPC متعددة القنوات، وعميل AppKit أصيل لنظام macOS. |

---

## معمارية Joyzoning ذات المناطق الـ 5 (`82` وحدة Rust · `300+` ملف)

| المنطقة | المسار الفعلي | المسؤوليات والثوابت المعمارية |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` وحدة) | الدوال الأولية عديمة الحالة: التجزئة، ميزانية الرموز، معالجة JSON، إخفاء الأسرار، Seatbelt، حماية سطر الأوامر، التصحيحات التقريبية والهندسة المكانية. **صفر استيراد للأعلى، وصفر حالة عامة قابلة للتغيير.** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` وحدة) | الكيانات الخالصة و64 سمة رسمية من نوع `I<Name>`، وضعيات التنفيذ، دليل أسعار النماذج، مواصفات الموجهات، مخصص Arena، شجرة المهارات. **0% إدخال/إخراج، وحد أقصى 3 هياكل لكل ملف.** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` وحدة) | محولات الإدخال/الإخراج الملموسة: بروتوكولات الشبكة لـ 61 مزوداً، تخزين VFS ثنائي المرحلة، لقطات CAS SHA-256، مخزن السلوك، والتنفيذ المعزول. |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` وحدة) | تنسيق حالات الاستخدام: آلة الحالة `AgentEngine`، جدول موجة الأدوات، المحرك المكاني، فاحص السياق، واختبارات الطيران. |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs)، [`native/`](../../native/) | طبقة العرض ونوايا المستخدم: واجهة الأوامر CLI، خادم stdio JSON-RPC، تطبيق AppKit الأصيل لنظام macOS، وجذور التركيب. |

---

## البدء السريع والتحقق (Quick Start)

```bash
# 1. تشغيل كامل اختبارات الوحدة واختبارات الفضاء والحوسبة والتكامل بلغة Rust
cargo test

# 2. تشغيل حزمة اختبارات AppKit الأصيلة لنظام macOS
cd native && swift test && cd ..

# 3. تشغيل اختبارات الطيران الذاتي وتشخيص سلامة النظام
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. تدقيق معمارية Joyzoning الصارم والتحقق من روابط التوثيق
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
