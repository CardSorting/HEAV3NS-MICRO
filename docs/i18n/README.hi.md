<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — संप्रभु बुद्धिमत्ता रनटाइम (Sovereign Intelligence Runtime)

**अति-संपीड़ित, वास्तुकला की दृष्टि से संप्रभु Rust बुद्धिमत्ता रनटाइम, डबल-रैप्ड स्थानिक क्रिया इंजन और मूल macOS AppKit IDE (`HEAV3NS-CODE`)** — **HEAV3NS-JOY** 5-ज़ोन Joyzoning वास्तुकला के अंतर्गत संरचित (`100%` स्थिरता सूचकांक, 0 उल्लंघन, कार्यक्षेत्र की सभी फाइलें Tier 1 Optimal `< 800` पंक्तियों के भीतर)।

<p align="center">
  <a href="../../doc.md">संप्रभु चार्टर (doc.md)</a> ·
  <a href="../../AGENTS.md">एजेंट प्रोटोकॉल (AGENTS.md)</a> ·
  <a href="../../SOUL.md">मूल चेतना (SOUL.md)</a> ·
  <a href="../../ROADMAP.md">रोडमैप</a> ·
  <a href="../README.md">दस्तावेज़ केंद्र</a> ·
  <a href="../papers/ascendant-intelligence.md">आरोही बुद्धिमत्ता श्वेतपत्र</a> ·
  <a href="../PROJECT_MAP.md">परियोजना मानचित्र</a> ·
  <a href="../CONSTITUTION.md">संवैधानिक सीमाएं</a> ·
  <a href="../../VALIDATION_REPORT.md">सत्यापन रिपोर्ट</a> ·
  <a href="../../native/PRODUCT.md">मूल विनिर्देश</a>
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

## सात संप्रभु स्तंभ (The Seven Sovereign Pillars)

> *"कोई साधारण एजेंट नहीं। कोई झुंड नहीं। कोई ऑपरेटिंग सिस्टम नहीं। यह सहयोग करने वाली बुद्धिमत्ता की एक संगणकीय सभ्यता है — जहां प्रणाली केवल निर्देशों को निष्पादित नहीं करती: वह अपने परिवेश को समझती है, उसकी संरचना सीखती है, अनुभव को याद रखती है और दृढ़ संकल्प के साथ कार्य करती है।"* — [`doc.md`](../../doc.md)

| स्तंभ | पदनाम | मुख्य कार्यान्वयन |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **आसुत बुद्धिमत्ता रनटाइम** | मल्टी-टर्न ReAct स्टेट मशीन [`AgentEngine`](../../src/core/engine.rs), KV-कैश नियतात्मक प्रॉम्प्ट विनिर्देश [`SystemPromptSpec`](../../src/domain/prompt_spec.rs), संदर्भ निरीक्षक [`ContextInspectionSnapshot`](../../src/core/context_inspector.rs), 3-चरणीय स्व-उपचार उपकरण पार्सर [`ToolCallArgParser`](../../src/domain/tool_parser.rs) और `FT-01..FT-05` उड़ान परीक्षण [`flight_runner`](../../src/core/flight_runner.rs)। |
| **2. `SPIDER`** | **संरचनात्मक बोध और परिवेश की समझ** | [`environmental_diff.rs`](../../src/plumbing/environmental_diff.rs) में संरचनात्मक संबंध विश्लेषण, पर्यावरणीय अवलोकन [`EnvironmentalObservation`](../../src/domain/types/environmental_types.rs) और `spider.check` के साथ कठोर AST ऑडिट। |
| **3. `REWIND`** | **कालिक संज्ञान, निरंतरता और नियतात्मक पुनर्प्राप्ति** | 16-MiB अखाड़ा मेमोरी आवंटक [`ArenaAllocator`](../../src/domain/arena.rs) $O(1)$ स्नैपशॉट रिवाइंड के साथ, शुद्ध Rust SHA-256 स्नैपशॉट CAS [`WorkspaceSnapshotCas`](../../src/infrastructure/workspace_cas.rs) (अवरोध-मुक्त `/undo`, `/restore` और Git-मुक्त `/diff`), 2-चरणीय [`SessionVfs`](../../src/infrastructure/vfs_store.rs) आधार रेखा सत्यापन, रोलबैक युक्त लेन-देन जर्नल [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) और 32-गहरी पूर्ववत/पुनः करें स्टैक [`VirtualDisplayState`](../../src/domain/computer_use.rs)। |
| **4. `BROCCOLI`** | **स्थायी ज्ञान, स्मृति और ऐतिहासिक विकास** | लाप्लास-स्मूथ विश्वास अंशांकन के साथ व्यावहारिक ज्ञान भंडार [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), हाइब्रिड TF-IDF स्मृति भंडार [`MemoryStore`](../../src/domain/memory.rs) और काह्न-टोपोलॉजिकल रूप से सत्यापित कौशल वृक्ष सूची [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) DAG। |
| **5. `SYNTHETIC SWARM`** | **वितरित सामूहिक बुद्धिमत्ता** | 5 विशेषज्ञों का समानांतर परामर्श (दृश्य धारणा, संरचनात्मक विश्लेषक, वर्कफ़्लो नेविगेटर, जोखिम मूल्यांकनकर्ता, पुनर्प्राप्ति विशेषज्ञ + संघर्ष मध्यस्थ) और विशिष्ट फ़ाइल आरक्षण के साथ 7-विशेषज्ञ [`Legion`](../../src/domain/swarm.rs) गठन। |
| **6. `COMPUTER USE`** | **मूर्त एजेंसी: अवलोकन, व्याख्या, क्रिया और अनुकूलन** | 9-चरणीय क्लोज्ड-लूप ऑर्केस्ट्रेटर [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs), डबल-रैप्ड स्थानिक क्रिया मध्यस्थ [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) (आवरण A स्थानिक एंकरिंग + आवरण B निष्पादन आश्वासन), ऑफ-स्क्रीन ऑटो-स्क्रॉल [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) और शून्य-विश्वास `browser_use`। |
| **7. `THE HARNESS`** | **सब कुछ बांधने वाला संगणकीय आधार** | 31 मूल उपकरण (8-स्लॉट LRU खोज और 32-चरणीय बैच निष्पादन), DAG वेवफ्रंट शेड्यूलर, 5 निष्पादन मुद्राएं [`ExecutionPosture`](../../src/domain/types/posture_types.rs), macOS Seatbelt सैंडबॉक्स, ऑडिट रसीद [`TurnReceipt`](../../src/domain/turn_receipt.rs), मल्टीप्लेक्स stdio JSON-RPC सेवा और मूल macOS AppKit क्लाइंट। |

---

## 5-ज़ोन Joyzoning वास्तुकला (`82` Rust मॉड्यूल · `300+` फ़ाइलें)

| ज़ोन | भौतिक पथ | उत्तरदायित्व और अपरिवर्तनीय नियम |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](../../src/plumbing/mod.rs) (`12` मॉड्यूल) | स्टेटलेस प्रिमिटिव: हैशिंग, टोकन बजट, JSON मरम्मत, गुप्त डेटा मास्किंग, Seatbelt, शेल सुरक्षा, फ़ज़ी पैच और स्थानिक ज्यामिति। **शून्य अपवर्ड आयात, शून्य परिवर्तनीय वैश्विक स्थिति।** |
| **`DOMAIN`** | [`src/domain/`](../../src/domain/mod.rs) (`31` मॉड्यूल) | शुद्ध व्यावसायिक इकाइयां और 64 औपचारिक `I<Name>` ट्रे लक्षण, निष्पादन मुद्राएं, मॉडल मूल्य सूची, प्रॉम्प्ट विनिर्देश, अखाड़ा आवंटक, कौशल वृक्ष। **सख्ती से 0% I/O, प्रति फ़ाइल अधिकतम 3 संरचनाएं।** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](../../src/infrastructure/mod.rs) (`11` मॉड्यूल) | ठोस I/O एडेप्टर: 61 प्रदाताओं के लिए HTTP और नेटवर्क प्रोटोकॉल, 2-चरणीय VFS भंडारण, SHA-256 CAS स्नैपशॉट, व्यवहार ज्ञान भंडार और सैंडबॉक्स शेल निष्पादन। |
| **`CORE`** | [`src/core/`](../../src/core/mod.rs) (`20` मॉड्यूल) | उपयोग-मामला ऑर्केस्ट्रेशन: `AgentEngine` स्टेट मशीन, वेवफ्रंट शेड्यूलर, स्थानिक इंजन, संदर्भ निरीक्षक और उड़ान परीक्षण। |
| **`UI`** | [`src/ui/`](../../src/ui/mod.rs), [`native/`](../../native/) | प्रस्तुति और उपयोगकर्ता अभिप्राय: CLI इंटरफ़ेस, stdio JSON-RPC वर्कर, मूल macOS AppKit डेस्कटॉप अनुप्रयोग और संरचना जड़ें। |

---

## त्वरित शुरुआत और सत्यापन (Quick Start)

```bash
# 1. सभी Rust यूनिट, स्थानिक, कंप्यूटर उपयोग और एकीकरण परीक्षण चलाएं
cargo test

# 2. मूल macOS AppKit परीक्षण सूट चलाएं
cd native && swift test && cd ..

# 3. स्वायत्त उड़ान परीक्षण और सिस्टम निदान चलाएं
cargo run -- --flight
cargo run -- doctor
cargo run -- receipts

# 4. सख्त Joyzoning वास्तुशिल्प ऑडिट और दस्तावेज़ लिंक जांचें
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
