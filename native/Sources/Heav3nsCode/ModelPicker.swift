// [LAYER: UI]
import AppKit

/// Modeless so the conversation and Stop remain available while browsing routes.
final class ModelPicker: NSWindowController, NSWindowDelegate, NSTextFieldDelegate, NSSearchFieldDelegate {
    private let request: (String, [String: Any], @escaping (Result<[String: Any], Error>) -> Void) -> Void
    private let connected: () -> Bool
    private let supportsDiscovery: () -> Bool
    private let canApply: () -> Bool
    private let onSelection: (WorkspaceModelSelection) -> Void
    private let providerFilter = NSSearchField()
    private let recentRow = NSStackView()
    private let provider = NSPopUpButton()
    private let apiKey = NSSecureTextField()
    private let keyStatus = JourneyStyle.label("", size: 11, secondary: true)
    private let saveKey = NSButton(title: "Save Key", target: nil, action: nil)
    private let removeKey = NSButton(title: "Remove Key", target: nil, action: nil)
    private let credentialsRow = NSStackView()
    private let appleIntelligenceStatus = JourneyStyle.label("", size: 11, secondary: true)
    private let openAppleIntelligence = NSButton(title: "Open Apple Intelligence Settings…", target: nil, action: nil)
    private let appleIntelligenceRow = NSStackView()
    private let openChatGPTAccounts = NSButton(title: "Manage ChatGPT accounts…", target: nil, action: nil)
    private let chatGPTAccountsRow = NSStackView()
    private var accountsObserver: NSObjectProtocol?
    private var keyPending = false
    private var keySaved = false
    private let supportsCredentials: () -> Bool
    private let model = NSTextField()
    private let modelChoice = ModelChoiceList(frame: .zero)
    private let customRow = NSStackView()
    private var suggestions = [String]()
    private var customMode = false
    private var customProviders = Set<String>()
    private var forceDiscovery = false
    private let detail = JourneyStyle.label("", size: 12, secondary: true)
    private let status = JourneyStyle.label("Loading current selection…", size: 12)
    private let apply = NSButton(title: "Switch Model", target: nil, action: nil)
    private let reload = NSButton(title: "Refresh", target: nil, action: nil)
    private var providers = [WorkspaceModelCatalog.Provider]()
    private var visibleProviders = [WorkspaceModelCatalog.Provider]()
    private var recentSelections = [WorkspaceModelSelection]()
    private let availability = JourneyStyle.label("", size: 11, secondary: true)
    private var failure: String?
    private var reconciled = false
    private var blocked: String?
    private var preserveDraft = false
    private var selection: WorkspaceModelSelection?
    private var remembered = [String: String]()
    private var displayedProvider: String?
    private var pending = false
    private var dismissed = false
    private var loadID = UUID()
    private var resetRevision = false
    private weak var previousResponder: NSResponder?
    private var focusModelAfterLoad = false
    private let catalogStatus = JourneyStyle.label("", size: 11, secondary: true)
    private let modelInformation = JourneyStyle.label("", size: 11, secondary: true)
    private var discoveries = [String: WorkspaceModelDiscovery]()
    private var discoveryRequests = [String: UUID]()

    private struct CredentialSpec {
        let rpcPrefix: String
        let placeholder: String
        let envVar: String
        let keyPortal: String
    }

    private static func credentialSpec(for providerID: String) -> CredentialSpec? {
        switch providerID {
        case "openrouter": return CredentialSpec(rpcPrefix: "openrouter", placeholder: "OpenRouter API key", envVar: "OPENROUTER_API_KEY", keyPortal: "openrouter.ai/keys")
        case "orcarouter": return CredentialSpec(rpcPrefix: "orcarouter", placeholder: "OrcaRouter API key", envVar: "ORCAROUTER_API_KEY", keyPortal: "orcarouter.ai")
        case "codewhale": return CredentialSpec(rpcPrefix: "codewhale", placeholder: "Codewhale API key", envVar: "CODEWHALE_API_KEY", keyPortal: "codewhale.net")
        case "cline-pass": return CredentialSpec(rpcPrefix: "clinePass", placeholder: "Cline Pass API key", envVar: "CLINE_API_KEY", keyPortal: "app.cline.bot")
        case "nous": return CredentialSpec(rpcPrefix: "nous", placeholder: "Nous Research API key", envVar: "NOUS_API_KEY", keyPortal: "portal.nousresearch.com/keys")
        case "opencode": return CredentialSpec(rpcPrefix: "opencode", placeholder: "OpenCode API key or Access Token", envVar: "OPENCODE_API_KEY", keyPortal: "opencode.ai/zen")
        case "opencode-go": return CredentialSpec(rpcPrefix: "opencodeGo", placeholder: "OpenCode Go API key", envVar: "OPENCODE_GO_API_KEY", keyPortal: "opencode.ai/auth")
        case "openai": return CredentialSpec(rpcPrefix: "openai", placeholder: "OpenAI API key", envVar: "OPENAI_API_KEY", keyPortal: "platform.openai.com/api-keys")
        case "anthropic": return CredentialSpec(rpcPrefix: "anthropic", placeholder: "Anthropic API key", envVar: "ANTHROPIC_API_KEY", keyPortal: "console.anthropic.com/settings/keys")
        case "gemini": return CredentialSpec(rpcPrefix: "gemini", placeholder: "Google Gemini API key", envVar: "GEMINI_API_KEY", keyPortal: "aistudio.google.com/apikey")
        case "deepseek", "deepseek-anthropic": return CredentialSpec(rpcPrefix: "deepseek", placeholder: "DeepSeek API key", envVar: "DEEPSEEK_API_KEY", keyPortal: "platform.deepseek.com/api_keys")
        case "xai": return CredentialSpec(rpcPrefix: "xai", placeholder: "xAI API key", envVar: "XAI_API_KEY", keyPortal: "console.x.ai")
        case "groq": return CredentialSpec(rpcPrefix: "groq", placeholder: "Groq API key", envVar: "GROQ_API_KEY", keyPortal: "console.groq.com/keys")
        case "mistral": return CredentialSpec(rpcPrefix: "mistral", placeholder: "Mistral API key", envVar: "MISTRAL_API_KEY", keyPortal: "console.mistral.ai/api-keys")
        case "qwen", "qwen-anthropic": return CredentialSpec(rpcPrefix: "qwen", placeholder: "DashScope / Qwen API key", envVar: "DASHSCOPE_API_KEY", keyPortal: "dashscope.console.aliyun.com/apiKey")
        case "modelstudio-token-plan", "modelstudio-token-plan-anthropic", "modelstudio-coding-plan", "modelstudio-coding-plan-anthropic":
            return CredentialSpec(rpcPrefix: "modelstudioTokenPlan", placeholder: "Alibaba Model Studio API key", envVar: "MODELSTUDIO_API_KEY", keyPortal: "modelstudio.console.alibabacloud.com")
        case "moonshot", "kimi-code": return CredentialSpec(rpcPrefix: "moonshot", placeholder: "Moonshot / Kimi API key", envVar: "MOONSHOT_API_KEY", keyPortal: "platform.moonshot.ai/console/api-keys")
        case "zai", "zai-anthropic": return CredentialSpec(rpcPrefix: "zai", placeholder: "Z.ai / GLM API key", envVar: "ZAI_API_KEY", keyPortal: "z.ai/manage-apikey/apikey-list")
        case "minimax", "minimax-anthropic": return CredentialSpec(rpcPrefix: "minimax", placeholder: "MiniMax API key", envVar: "MINIMAX_API_KEY", keyPortal: "platform.minimax.io")
        case "volcengine": return CredentialSpec(rpcPrefix: "volcengine", placeholder: "Volcengine Ark API key", envVar: "ARK_API_KEY", keyPortal: "console.volcengine.com/ark")
        case "wanjie-ark": return CredentialSpec(rpcPrefix: "wanjieArk", placeholder: "Wanjie Ark API key", envVar: "WANJIE_ARK_API_KEY", keyPortal: "wanjiedata.com")
        case "nvidia-nim": return CredentialSpec(rpcPrefix: "nvidiaNim", placeholder: "NVIDIA NIM API key", envVar: "NVIDIA_API_KEY", keyPortal: "build.nvidia.com")
        case "together": return CredentialSpec(rpcPrefix: "together", placeholder: "Together AI API key", envVar: "TOGETHER_API_KEY", keyPortal: "api.together.ai/settings/api-keys")
        case "fireworks": return CredentialSpec(rpcPrefix: "fireworks", placeholder: "Fireworks AI API key", envVar: "FIREWORKS_API_KEY", keyPortal: "fireworks.ai/account/api-keys")
        case "deepinfra": return CredentialSpec(rpcPrefix: "deepinfra", placeholder: "DeepInfra API key", envVar: "DEEPINFRA_API_KEY", keyPortal: "deepinfra.com/dash/api_keys")
        case "atlascloud": return CredentialSpec(rpcPrefix: "atlascloud", placeholder: "AtlasCloud API key", envVar: "ATLASCLOUD_API_KEY", keyPortal: "atlascloud.ai/console/api-keys")
        case "siliconflow": return CredentialSpec(rpcPrefix: "siliconflow", placeholder: "SiliconFlow API key", envVar: "SILICONFLOW_API_KEY", keyPortal: "cloud.siliconflow.com/account/ak")
        case "siliconflow-CN": return CredentialSpec(rpcPrefix: "siliconflowCN", placeholder: "SiliconFlow (CN) API key", envVar: "SILICONFLOW_API_KEY", keyPortal: "cloud.siliconflow.cn/account/ak")
        case "cerebras": return CredentialSpec(rpcPrefix: "cerebras", placeholder: "Cerebras API key", envVar: "CEREBRAS_API_KEY", keyPortal: "cloud.cerebras.ai")
        case "baseten": return CredentialSpec(rpcPrefix: "baseten", placeholder: "Baseten API key", envVar: "BASETEN_API_KEY", keyPortal: "app.baseten.co/settings/api_keys")
        case "novita": return CredentialSpec(rpcPrefix: "novita", placeholder: "Novita AI API key", envVar: "NOVITA_API_KEY", keyPortal: "novita.ai/settings/key-management")
        case "arcee": return CredentialSpec(rpcPrefix: "arcee", placeholder: "Arcee AI API key", envVar: "ARCEE_API_KEY", keyPortal: "chat.arcee.ai/account/api-keys")
        case "xiaomi-mimo": return CredentialSpec(rpcPrefix: "xiaomiMimo", placeholder: "Xiaomi MiMo API key", envVar: "MIMO_API_KEY", keyPortal: "platform.xiaomimimo.com")
        case "stepfun": return CredentialSpec(rpcPrefix: "stepfun", placeholder: "StepFun API key", envVar: "STEPFUN_API_KEY", keyPortal: "platform.stepfun.com")
        case "huggingface": return CredentialSpec(rpcPrefix: "huggingface", placeholder: "Hugging Face Token", envVar: "HF_TOKEN", keyPortal: "huggingface.co/settings/tokens")
        case "modelscope": return CredentialSpec(rpcPrefix: "modelscope", placeholder: "ModelScope API key", envVar: "MODELSCOPE_API_KEY", keyPortal: "modelscope.cn/my/myaccesstoken")
        case "qianfan": return CredentialSpec(rpcPrefix: "qianfan", placeholder: "Baidu Qianfan API key", envVar: "QIANFAN_API_KEY", keyPortal: "console.bce.baidu.com/iam")
        case "openmodel": return CredentialSpec(rpcPrefix: "openmodel", placeholder: "OpenModel API key", envVar: "OPENMODEL_API_KEY", keyPortal: "console.openmodel.ai/api-keys")
        case "sakana": return CredentialSpec(rpcPrefix: "sakana", placeholder: "Sakana AI (Fugu) API key", envVar: "FUGU_API_KEY", keyPortal: "console.sakana.ai")
        case "longcat": return CredentialSpec(rpcPrefix: "longcat", placeholder: "Meituan LongCat API key", envVar: "LONGCAT_API_KEY", keyPortal: "longcat.chat/platform/api_keys")
        case "meta": return CredentialSpec(rpcPrefix: "meta", placeholder: "Meta Model API key", envVar: "META_MODEL_API_KEY", keyPortal: "developers.facebook.com")
        case "telecomjs": return CredentialSpec(rpcPrefix: "telecomjs", placeholder: "TelecomJS TokenHub API key", envVar: "TELECOMJS_API_KEY", keyPortal: "aigw.telecomjs.com")
        case "edenai": return CredentialSpec(rpcPrefix: "edenai", placeholder: "Eden AI API key", envVar: "EDENAI_API_KEY", keyPortal: "app.edenai.run")
        case "zenmux": return CredentialSpec(rpcPrefix: "zenmux", placeholder: "ZenMux API key", envVar: "ZENMUX_API_KEY", keyPortal: "zenmux.ai/settings/keys")
        case "csdn": return CredentialSpec(rpcPrefix: "csdn", placeholder: "CSDN Starmap API key", envVar: "CSDN_API_KEY", keyPortal: "ai.csdn.net")
        case "perplexity": return CredentialSpec(rpcPrefix: "perplexity", placeholder: "Perplexity API key", envVar: "PERPLEXITY_API_KEY", keyPortal: "perplexity.ai/settings/api")
        case "cohere": return CredentialSpec(rpcPrefix: "cohere", placeholder: "Cohere API key", envVar: "COHERE_API_KEY", keyPortal: "dashboard.cohere.com/api-keys")
        case "concentrate": return CredentialSpec(rpcPrefix: "concentrate", placeholder: "Concentrate AI API key", envVar: "CONCENTRATE_API_KEY", keyPortal: "concentrate.ai")
        case "ollama-cloud": return CredentialSpec(rpcPrefix: "ollamaCloud", placeholder: "Ollama Cloud API key", envVar: "OLLAMA_CLOUD_API_KEY", keyPortal: "ollama.com/settings/keys")
        case "custom": return CredentialSpec(rpcPrefix: "custom", placeholder: "Custom Endpoint API key", envVar: "CUSTOM_API_KEY", keyPortal: "your custom endpoint")
        default: return nil
        }
    }

    convenience init(client: RuntimeClient, canApply: @escaping () -> Bool, onSelection: @escaping (WorkspaceModelSelection) -> Void) {
        self.init(request: { method, params, completion in client.request(method, params: params, completion: completion) }, connected: { client.isReady }, supportsDiscovery: { client.capabilities.contains("model-discovery-v1") }, supportsCredentials: { client.capabilities.contains("openrouter-credentials-v1") || client.capabilities.contains("multi-provider-parity-v1") }, canApply: canApply, onSelection: onSelection)
    }
    init(request: @escaping (String, [String: Any], @escaping (Result<[String: Any], Error>) -> Void) -> Void, connected: @escaping () -> Bool, supportsDiscovery: @escaping () -> Bool = { false }, supportsCredentials: @escaping () -> Bool = { false }, canApply: @escaping () -> Bool, onSelection: @escaping (WorkspaceModelSelection) -> Void) {
        self.supportsCredentials = supportsCredentials
        self.request = request; self.connected = connected; self.supportsDiscovery = supportsDiscovery; self.canApply = canApply; self.onSelection = onSelection
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 560, height: 640), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        panel.title = "Model & Provider"; panel.isReleasedWhenClosed = false; panel.hidesOnDeactivate = false
        super.init(window: panel); panel.delegate = self
        apiKey.placeholderString = "OpenRouter API key"; apiKey.setAccessibilityLabel("OpenRouter API key")
        apiKey.delegate = self; apiKey.target = self; apiKey.action = #selector(storeKey)
        apiKey.setContentHuggingPriority(.defaultLow, for: .horizontal)
        saveKey.target = self; saveKey.action = #selector(storeKey); saveKey.bezelStyle = .rounded
        removeKey.target = self; removeKey.action = #selector(clearKey); removeKey.bezelStyle = .rounded
        let keyEntry = JourneyStyle.stack([apiKey, saveKey, removeKey], vertical: false, spacing: 8)
        credentialsRow.orientation = .vertical; credentialsRow.alignment = .leading; credentialsRow.spacing = 6
        credentialsRow.addArrangedSubview(keyEntry); credentialsRow.addArrangedSubview(keyStatus)
        keyEntry.widthAnchor.constraint(equalTo: credentialsRow.widthAnchor).isActive = true
        keyStatus.widthAnchor.constraint(equalTo: credentialsRow.widthAnchor).isActive = true
        keyStatus.maximumNumberOfLines = 2
        credentialsRow.isHidden = true
        openAppleIntelligence.target = self; openAppleIntelligence.action = #selector(showAppleIntelligenceSettings); openAppleIntelligence.bezelStyle = .rounded
        appleIntelligenceStatus.maximumNumberOfLines = 2
        appleIntelligenceRow.orientation = .horizontal; appleIntelligenceRow.alignment = .centerY; appleIntelligenceRow.spacing = 8
        appleIntelligenceRow.addArrangedSubview(appleIntelligenceStatus); appleIntelligenceRow.addArrangedSubview(NSView()); appleIntelligenceRow.addArrangedSubview(openAppleIntelligence)
        appleIntelligenceStatus.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        openAppleIntelligence.setContentCompressionResistancePriority(.required, for: .horizontal)
        appleIntelligenceRow.isHidden = true
        openChatGPTAccounts.target = self; openChatGPTAccounts.action = #selector(showChatGPTAccounts); openChatGPTAccounts.bezelStyle = .rounded
        openChatGPTAccounts.toolTip = "See plan and usage-limit status, switch accounts, or add another"
        chatGPTAccountsRow.orientation = .horizontal; chatGPTAccountsRow.alignment = .centerY; chatGPTAccountsRow.spacing = 8
        chatGPTAccountsRow.addArrangedSubview(NSView()); chatGPTAccountsRow.addArrangedSubview(openChatGPTAccounts)
        chatGPTAccountsRow.isHidden = true
        // Switching an account elsewhere changes the status line shown for this provider; keep it current.
        accountsObserver = NotificationCenter.default.addObserver(forName: ChatGPTAccountsWindow.didChange, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.window?.isVisible == true else { return }
            self.load()
        }
        providerFilter.placeholderString = "Filter 65 providers (e.g. local, anthropic, codewhale, qwen)…"
        providerFilter.setAccessibilityLabel("Filter providers")
        providerFilter.delegate = self; providerFilter.sendsSearchStringImmediately = true
        recentRow.orientation = .horizontal; recentRow.alignment = .centerY; recentRow.spacing = 6; recentRow.isHidden = true
        provider.target = self; provider.action = #selector(changeProvider); provider.setAccessibilityLabel("Provider")
        model.delegate = self; model.setAccessibilityLabel("Custom model ID")
        model.placeholderString = "Exact model ID"; model.target = self; model.action = #selector(save)
        modelChoice.onChoose = { [weak self] in self?.chooseModel($0) }
        modelChoice.onCustom = { [weak self] in self?.chooseCustom() }
        modelChoice.onCancel = { [weak self] in self?.dismiss() }
        modelChoice.setContentHuggingPriority(.init(1), for: .vertical)
        apply.target = self; apply.action = #selector(save); apply.bezelStyle = .rounded
        reload.target = self; reload.action = #selector(refreshProviderModels); reload.bezelStyle = .rounded
        reload.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: nil); reload.imagePosition = .imageLeading
        reload.toolTip = "Refresh the saved selection and this provider’s model list. Your custom entry is preserved."
        customRow.orientation = .horizontal; customRow.alignment = .centerY; customRow.spacing = 8
        customRow.addArrangedSubview(model); customRow.addArrangedSubview(apply)
        model.setContentHuggingPriority(.defaultLow, for: .horizontal)
        model.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        apply.setContentCompressionResistancePriority(.required, for: .horizontal)
        catalogStatus.maximumNumberOfLines = 2; catalogStatus.isSelectable = true
        modelInformation.maximumNumberOfLines = 2
        let footer = JourneyStyle.stack([catalogStatus, NSView(), reload], vertical: false, spacing: 8)
        catalogStatus.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        reload.setContentCompressionResistancePriority(.required, for: .horizontal)
        let root = JourneyStyle.stack([
            JourneyStyle.label("Choose Model", size: 20, weight: .semibold),
            JourneyStyle.label("Choose a model to switch immediately in this workspace.", size: 12, secondary: true),
            status, recentRow, JourneyStyle.rule(), JourneyStyle.label("Provider", size: 12, weight: .semibold), providerFilter, provider, detail,
            credentialsRow, appleIntelligenceRow, chatGPTAccountsRow, modelChoice, customRow, modelInformation,
            availability, footer
        ], spacing: 10)
        panel.contentView?.addSubview(root)
        if let content = panel.contentView {
            JourneyStyle.pin(root, to: content, inset: 20)
            for view in root.arrangedSubviews { view.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true }
        }
        status.isSelectable = true; status.maximumNumberOfLines = 3
        status.toolTip = status.stringValue
        detail.maximumNumberOfLines = 3
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    func present(for parent: NSWindow) {
        guard let window else { return }
        dismissed = false
        previousResponder = parent.firstResponder
        focusModelAfterLoad = true
        forceDiscovery = true
        parent.addChildWindow(window, ordered: .above)
        syncPresentation(recenter: true)
        window.makeKeyAndOrderFront(nil); window.makeFirstResponder(modelChoice.search); load()
    }
    func syncPresentation(recenter: Bool = false) {
        guard let window, let parent = window.parent else { return }
        window.appearance = parent.appearance
        if recenter {
            let visible = parent.screen?.visibleFrame ?? parent.frame
            let x = min(max(parent.frame.midX - window.frame.width / 2, visible.minX), visible.maxX - window.frame.width)
            let y = min(max(parent.frame.midY - window.frame.height / 2, visible.minY), visible.maxY - window.frame.height)
            window.setFrameOrigin(NSPoint(x: x, y: y))
        }
    }
    private var draftModel: String { model.stringValue.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var dirty: Bool { guard let selection else { return false }; return displayedProvider != selection.provider || draftModel != selection.model }
    private var validation: String? {
        if displayedProvider == "openrouter" && !draftModel.contains("/") { return "Enter an OpenRouter model ID, such as anthropic/claude-sonnet-4.5." }
        guard WorkspaceModelSelection.validModel(draftModel) else { return "Enter a model ID of up to 200 characters, without spaces." }
        if (displayedProvider == "openai-codex" || displayedProvider == "nous" || displayedProvider == "apple-playground") && draftModel.lowercased().hasPrefix("claude-") { return "Choose Claude Code for a native Claude model, or enter a compatible model ID." }
        return nil
    }
    @objc private func showChatGPTAccounts() { ChatGPTAccountsWindow.shared.present() }
    @objc private func showAppleIntelligenceSettings() {
        OnDevicePlaygroundService.shared.openAppleIntelligenceSettings()
    }
    func refreshAvailability() {
        syncPresentation()
        let editable = !dismissed && !pending && !keyPending && selection != nil
        let hasCredentials = Self.credentialSpec(for: displayedProvider ?? "") != nil
        credentialsRow.isHidden = !hasCredentials || !supportsCredentials()
        let isApplePlayground = displayedProvider == "apple-playground"
        appleIntelligenceRow.isHidden = !isApplePlayground
        chatGPTAccountsRow.isHidden = displayedProvider != "openai-codex"
        if isApplePlayground {
            let localStatus = OnDevicePlaygroundService.shared.localTextModelStatus()
            appleIntelligenceStatus.stringValue = OnDevicePlaygroundService.shared.statusSummary()
            openAppleIntelligence.isHidden = localStatus == "ready"
        }
        apiKey.isEnabled = connected() && editable && canApply()
        saveKey.isEnabled = apiKey.isEnabled && !apiKey.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        removeKey.isEnabled = apiKey.isEnabled && keySaved
        providerFilter.isEnabled = editable
        provider.isEnabled = editable && !visibleProviders.isEmpty; model.isEnabled = editable
        modelChoice.setAvailability(browse: editable, canSwitch: editable && connected() && reconciled && blocked == nil && canApply(), customVisible: customMode)
        customRow.isHidden = !customMode
        reload.isEnabled = connected() && !pending && !keyPending && discoveryRequests[displayedProvider ?? ""] == nil
        apply.isEnabled = connected() && editable && reconciled && blocked == nil && canApply() && dirty && validation == nil
        apply.toolTip = "Switch to this exact model ID for future instructions"
        availability.stringValue = !connected() ? "Disconnected. Your edit is preserved. Reconnect the workspace to check the saved choice." : !canApply() ? "Finish or stop active and queued work before switching." : ""
        availability.isHidden = availability.stringValue.isEmpty
        refreshDiscoveryPresentation()
        if pending { return }
        status.stringValue = blocked ?? failure ?? (customMode ? validation : nil) ?? selection.map { "Current: \($0.providerLabel) · \($0.model)" } ?? "Refresh to read the current selection."
        status.toolTip = status.stringValue
    }
    @objc func load() {
        guard !pending, connected() else { return }
        preserveDraft = dirty
        if let displayedProvider { remembered[displayedProvider] = model.stringValue }
        let token = UUID(); loadID = token; pending = true; reconciled = false; failure = nil
        status.stringValue = "Loading current selection…"; refreshAvailability()
        request("models.list", [:]) { [weak self] result in
            guard let self, self.loadID == token else { return }
            self.pending = false
            switch result {
            case .success(let data): self.install(data, preserve: self.preserveDraft)
            case .failure(let error): self.failure = error.localizedDescription + " Your model edit is preserved. Choose Refresh to try again."
            }
            self.refreshAvailability()
            if self.focusModelAfterLoad, self.reconciled, self.window?.isKeyWindow == true {
                self.window?.makeFirstResponder(self.customMode ? self.model : self.modelChoice.search)
            }
            self.focusModelAfterLoad = false
        }
    }
    private func install(_ data: [String: Any], preserve: Bool = false) {
        guard let catalog = WorkspaceModelCatalog(data), resetRevision || selection == nil || catalog.selection.revision >= selection!.revision else {
            reconciled = false; failure = "Model settings were incomplete or outdated. Your edit is preserved. Choose Refresh to try again."; return
        }
        let draftProvider = preserve ? displayedProvider : nil
        blocked = catalog.blocked
        selection = catalog.selection; providers = catalog.providers; recentSelections = catalog.recent; reconciled = true; resetRevision = false; failure = nil
        for item in catalog.recent where remembered[item.provider] == nil { remembered[item.provider] = item.model }
        if !preserve { remembered[catalog.selection.provider] = catalog.selection.model }
        rebuildRecentRow()
        let target = draftProvider ?? catalog.selection.provider
        rebuildProviderPopup(preferring: target)
        displayedProvider = nil; changeProvider(); onSelection(catalog.selection)
    }
    private func rebuildRecentRow() {
        recentRow.arrangedSubviews.forEach { recentRow.removeArrangedSubview($0); $0.removeFromSuperview() }
        let quick = recentSelections.filter { $0.provider != selection?.provider || $0.model != selection?.model }.prefix(3)
        guard !quick.isEmpty else { recentRow.isHidden = true; return }
        recentRow.addArrangedSubview(JourneyStyle.label("Recent:", size: 11, secondary: true))
        for (idx, item) in quick.enumerated() {
            let btn = NSButton(title: "\(item.providerLabel) · \(item.model)", target: self, action: #selector(selectRecentChip(_:)))
            btn.bezelStyle = .inline; btn.tag = idx; btn.font = .systemFont(ofSize: 11)
            btn.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            recentRow.addArrangedSubview(btn)
        }
        recentRow.addArrangedSubview(NSView())
        recentRow.isHidden = false
    }
    @objc private func selectRecentChip(_ sender: NSButton) {
        let quick = Array(recentSelections.filter { $0.provider != selection?.provider || $0.model != selection?.model }.prefix(3))
        guard quick.indices.contains(sender.tag), connected(), reconciled, blocked == nil, canApply() else { return }
        let chosen = quick[sender.tag]
        providerFilter.stringValue = ""
        rebuildProviderPopup(preferring: chosen.provider)
        changeProvider()
        model.stringValue = chosen.model
        if validation == nil && dirty { save() }
    }
    private func rebuildProviderPopup(preferring targetID: String?) {
        let terms = providerFilter.stringValue.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current).split(whereSeparator: \.isWhitespace)
        if terms.isEmpty {
            visibleProviders = providers
        } else {
            visibleProviders = providers.filter { item in
                let hay = "\(item.id) \(item.label) \(item.detail) \(WorkspaceModelSelection.providerCategory(for: item.id))"
                    .folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
                return terms.allSatisfy { hay.contains($0) }
            }
        }
        provider.removeAllItems()
        visibleProviders.forEach { provider.addItem(withTitle: $0.label) }
        guard !visibleProviders.isEmpty else { return }
        let idx = targetID.flatMap { id in visibleProviders.firstIndex { $0.id == id } }
            ?? selection.flatMap { sel in visibleProviders.firstIndex { $0.id == sel.provider } }
            ?? 0
        provider.selectItem(at: idx)
    }
    @objc private func changeProvider() {
        if let displayedProvider { remembered[displayedProvider] = model.stringValue }
        guard visibleProviders.indices.contains(provider.indexOfSelectedItem) else { return }
        let item = visibleProviders[provider.indexOfSelectedItem]
        if displayedProvider != item.id { apiKey.stringValue = "" }
        displayedProvider = item.id
        if let spec = Self.credentialSpec(for: item.id) {
            apiKey.placeholderString = spec.placeholder
            apiKey.setAccessibilityLabel(item.id == "opencode" ? "OpenCode API key" : spec.placeholder)
            loadKeyStatus()
        }
        model.stringValue = remembered[item.id] ?? ""
        populateModels()
        customMode = customProviders.contains(item.id) || (!model.stringValue.isEmpty && !suggestions.contains(model.stringValue))
        detail.stringValue = item.detail; detail.toolTip = item.detail
        refreshAvailability(); discoverModels(refresh: forceDiscovery); forceDiscovery = false
    }
    private static func fallbackModelMetadata(for id: String) -> (label: String, detail: String)? {
        switch id {
        case "apple-foundation-model": return ("Apple Intelligence (On-Device)", "4k context · On-device FoundationModels LanguageModelSession")
        case "apple-playground-coding": return ("Apple Playground Coding", "4k context · On-device FoundationModels coding & tool specialist")
        case "apple-playground-fast": return ("Apple Playground Fast", "4k context · Low-latency concise on-device model")
        case "apple-image-playground": return ("Apple Image Playground", "32k context · On-device ImagePlayground & FoundationModels")
        case "Hermes-4-405B": return ("Hermes 4 (405B)", "128k context · Advanced reasoning")
        case "Hermes-4-70B": return ("Hermes 4 (70B)", "128k context · Balanced reasoning")
        case "~x-ai/grok-4.5", "~xai/grok-4.5": return ("Grok 4.5", "500k context · xAI reasoning")
        case "~anthropic/claude-opus-latest": return ("Claude Opus 4.8", "1M context · Frontier intelligence")
        case "~google/gemini-flash-latest": return ("Gemini 3.5 Flash", "1M context · Multimodal speed")
        case "~openai/gpt-latest": return ("GPT-5.5", "1M context · Flagship intelligence")
        case "xiaomi/mimo-v2-pro": return ("Xiaomi MiMo-v2 Pro", "1M context · Agentic engineering")
        case "xiaomi/mimo-v2-omni": return ("Xiaomi MiMo-v2 Omni", "262k context · Multimodal agentic")
        case "moonshotai/kimi-k2.6": return ("Kimi K2.6", "262k context · 1T parameter MoE")
        case "arcee-ai/trinity-large-thinking": return ("Trinity Large Thinking", "262k context · Thinking MoE")
        case "deepseek/deepseek-v4-flash": return ("DeepSeek-V4 Flash", "1M context · Fast MoE")
        case "cline-pass/qwen3.7-max": return ("Qwen3.7 Max", "1M context · Flagship coding")
        case "cline-pass/qwen3.7-plus": return ("Qwen3.7 Plus", "1M context · Fast coding & vision")
        case "cline-pass/minimax-m3": return ("MiniMax-M3", "524k context · Long horizon")
        case "cline-pass/mimo-v2.5-pro": return ("MiMo-V2.5-Pro", "1M context · High precision")
        case "cline-pass/glm-5.2": return ("GLM-5.2", "1M context · Reasoning")
        case "cline-pass/kimi-k2.7-code": return ("Kimi K2.7 Code", "262k context · Code intelligence")
        case "claude-sonnet-5[1m]": return ("Claude Sonnet 5", "1M context · Default")
        case "claude-opus-5[1m]": return ("Claude Opus 5", "1M context · Maximum intelligence")
        case "claude-opus-4-8[1m]": return ("Claude Opus 4.8", "1M context · Complex coding")
        case "claude-haiku-4-5-20251001": return ("Claude Haiku 4.5", "200k context · Fast response")
        case "claude-fable-5-1[1m]": return ("Claude Fable 5.1", "1M context · Agentic planning")
        case "gpt-5.6-terra": return ("GPT-5.6 Terra", "Flagship agentic model")
        case "kimi-k2.6": return ("Kimi K2.6", "262k context · Moonshot reasoning")
        case "kimi-k2.7-code": return ("Kimi K2.7 Code", "262k context · Moonshot coding")
        case "kimi-k3": return ("Kimi K3", "262k context · Moonshot frontier")
        case "glm-5": return ("GLM-5", "128k context · General reasoning")
        case "glm-5.1": return ("GLM-5.1", "128k context · Advanced reasoning")
        case "glm-5.2": return ("GLM-5.2", "128k context · High capability")
        case "glm-5.3": return ("GLM-5.3", "128k context · Frontier Chinese/English")
        case "mimo-v2.5-pro": return ("MiMo-V2.5-Pro", "1M context · High precision")
        case "minimax-m3": return ("MiniMax-M3", "524k context · Long horizon")
        case "qwen3.7-max": return ("Qwen3.7 Max", "1M context · Flagship coding")
        case "qwen3.7-plus": return ("Qwen3.7 Plus", "1M context · Fast coding & vision")
        case "deepseek-v4": return ("DeepSeek V4", "1M context · Frontier reasoning")
        case "deepseek-v4.1": return ("DeepSeek V4.1", "1M context · Enhanced reasoning")
        case "deepseek-v4.1-flash": return ("DeepSeek V4.1 Flash", "1M context · High speed")
        case "grok-4": return ("Grok 4", "500k context · xAI flagship")
        case "gpt-5.6-luna": return ("GPT-5.6 Luna", "128k context · High-speed agentic")
        case "beam-free": return ("Beam (Free)", "128k context · Free tier")
        case "deepseek-v4.1-flash-free": return ("DeepSeek V4.1 Flash (Free)", "1M context · Free tier")
        case "glm-5.3-flash-free": return ("GLM-5.3 Flash (Free)", "128k context · Free tier")
        case "jev-1.13-free": return ("Jev 1.13 (Free)", "128k context · Free tier")
        case "ll-alpha-free": return ("LL Alpha (Free)", "128k context · Free tier")
        case "longcat-2.5-preview-free": return ("LongCat 2.5 Preview (Free)", "128k context · Free tier")
        case "mimo-v2.6-flash-free": return ("MiMo-V2.6 Flash (Free)", "1M context · Free tier")
        case "muse-spark-1.3-contributor-free": return ("Muse Spark 1.3 (Free)", "128k context · Free tier")
        case "nemotron-3-ultra-free": return ("Nemotron 3 Ultra (Free)", "128k context · Free tier")
        case "nemotron-3.5-lightning-free": return ("Nemotron 3.5 Lightning (Free)", "128k context · Free tier")
        case "nemotron-3.5-super-vl-free": return ("Nemotron 3.5 Super VL (Free)", "128k context · Free tier vision")
        case "space-bunny-free": return ("Space Bunny (Free)", "128k context · Free tier")
        case "claude-sonnet-4-6": return ("Claude Sonnet 4.6", "200k context · Anthropic Messages + adaptive thinking")
        case "claude-opus-4-6": return ("Claude Opus 4.6", "200k context · Anthropic flagship intelligence")
        case "gemini-2.5-pro": return ("Gemini 2.5 Pro", "1M context · Google frontier reasoning")
        case "gemini-2.5-flash": return ("Gemini 2.5 Flash", "1M context · High-speed multimodal")
        case "deepseek-chat": return ("DeepSeek V3.2 Chat", "128k context · High-efficiency coding")
        case "deepseek-reasoner": return ("DeepSeek R1 Reasoner", "128k context · Chain-of-thought reasoning")
        case "grok-4-fast": return ("Grok 4 Fast", "256k context · Low-latency xAI coding")
        case "llama-3.3-70b-versatile": return ("Llama 3.3 70B", "128k context · Groq LPU ultra-low latency")
        case "mistral-large-latest": return ("Mistral Large", "128k context · Mistral flagship")
        case "codestral-latest": return ("Codestral", "256k context · Mistral code specialist")
        case "qwen3-coder-plus": return ("Qwen3 Coder Plus", "1M context · Alibaba agentic coding")
        case "llama3.2": return ("Llama 3.2 (Local)", "128k context · Free local Ollama inference")
        case "qwen2.5-coder:7b": return ("Qwen 2.5 Coder 7B (Local)", "32k context · Free local coding specialist")
        default: return nil
        }
    }
    private func populateModels() {
        guard let id = displayedProvider, let item = providers.first(where: { $0.id == id }) else { return }
        let draft = model.stringValue
        suggestions = []
        let current = selection?.provider == id ? [selection!.model] : []
        let liveModels = discoveries[id]?.models.map(\.id) ?? []
        let discovered = (!liveModels.isEmpty ? liveModels : item.models).sorted { $0.localizedStandardCompare($1) == .orderedAscending }
        for candidate in current + recentSelections.filter({ $0.provider == id }).map(\.model) + discovered where !suggestions.contains(candidate) { suggestions.append(candidate) }
        var meta = [String: (label: String, detail: String)]()
        if let models = discoveries[id]?.models {
            for m in models {
                meta[m.id] = (label: m.label, detail: m.detail)
            }
        }
        for s in suggestions {
            if meta[s] == nil, let fallback = Self.fallbackModelMetadata(for: s) {
                meta[s] = fallback
            }
        }
        modelChoice.update(models: suggestions, current: selection?.provider == id ? selection?.model : nil, context: id, metadata: meta)
        model.stringValue = draft
    }
    private func chooseCustom() {
        guard !dismissed, !pending else { return }
        if let displayedProvider { customProviders.insert(displayedProvider) }
        customMode = true; refreshAvailability(); window?.makeFirstResponder(model)
    }
    private func chooseModel(_ id: String) {
        guard !dismissed, !pending, connected(), reconciled, blocked == nil, canApply(), suggestions.contains(id) else { return }
        if let displayedProvider { customProviders.remove(displayedProvider) }
        customMode = false; model.stringValue = id
        if validation != nil { customMode = true; refreshAvailability(); return }
        if !dirty { dismiss(); return }
        save()
    }
    private func refreshDiscoveryPresentation() {
        guard let id = displayedProvider else { catalogStatus.stringValue = ""; modelInformation.stringValue = ""; return }
        let discovery = discoveries[id]
        if discoveryRequests[id] != nil { catalogStatus.stringValue = "Loading models from this connection…" }
        else if let discovery { catalogStatus.stringValue = discovery.summary }
        else { catalogStatus.stringValue = supportsDiscovery() ? "Refresh to load models from this connection." : "Provider discovery requires a newer runtime. Enter an exact model ID." }
        catalogStatus.toolTip = discovery?.message ?? catalogStatus.stringValue
        if discovery?.models.contains(where: { $0.id == draftModel }) == true {
            modelInformation.stringValue = "Press Return or choose Switch Model to use this ID."
        } else { modelInformation.stringValue = draftModel.isEmpty ? "Choose a model above, or use Other model ID." : discovery?.state == "ready" ? "This ID is not in the latest list. Choose a listed model or keep this exact custom ID." : "Current, recent, or custom ID · Access is checked when you send." }
        modelInformation.isHidden = !customMode
        modelInformation.toolTip = modelInformation.stringValue
        if let discovery, ["stale", "empty", "unavailable"].contains(discovery.state) {
            modelInformation.stringValue = discovery.message; modelInformation.isHidden = false; modelInformation.toolTip = discovery.message
        }
    }
    @objc private func refreshProviderModels() { guard !pending else { return }; forceDiscovery = true; load() }
    private func discoverModels(refresh: Bool = false) {
        guard connected(), supportsDiscovery(), let id = displayedProvider, discoveryRequests[id] == nil else { return }
        let token = UUID(); discoveryRequests[id] = token; refreshAvailability()
        request("models.discover", ["provider": id, "refresh": refresh]) { [weak self] result in
            guard let self, self.discoveryRequests[id] == token else { return }
            self.discoveryRequests[id] = nil
            if case .success(let data) = result, let discovery = WorkspaceModelDiscovery(data), discovery.provider == id {
                self.discoveries[id] = discovery
            } else {
                let previous = self.discoveries[id]
                var data: [String: Any] = ["provider": id, "models": previous?.models.map { ["id": $0.id, "label": $0.label, "detail": $0.detail] } ?? [], "state": previous?.models.isEmpty == false ? "stale" : "unavailable", "source": previous?.source ?? "Provider discovery", "message": "Could not load models. Check the connection and choose Refresh. Your edit is preserved."]
                if let checkedAt = previous?.checkedAt { data["checkedAt"] = checkedAt.timeIntervalSince1970 * 1000 }
                self.discoveries[id] = WorkspaceModelDiscovery(data)
            }
            guard self.displayedProvider == id else { return }
            self.populateModels(); self.refreshAvailability()
        }
    }
    private func loadKeyStatus() {
        guard supportsCredentials(), connected(), !keyPending, let id = displayedProvider, let spec = Self.credentialSpec(for: id) else { return }
        keyStatus.stringValue = "Checking saved key…"
        let token = loadID
        request("\(spec.rpcPrefix).status", [:]) { [weak self] result in
            guard let self, self.loadID == token, !self.dismissed else { return }
            self.installKeyStatus(result)
        }
    }
    private func installKeyStatus(_ result: Result<[String: Any], Error>) {
        switch result {
        case .success(let data):
            keySaved = data["saved"] as? Bool == true
            if displayedProvider == "opencode" {
                keyStatus.stringValue = keySaved ? "API key saved. Access is checked when you send." : data["configured"] as? Bool == true ? "Using OPENCODE_API_KEY or OpenCode session." : "Add your OpenCode API key or access token to send instructions."
            } else if let spec = Self.credentialSpec(for: displayedProvider ?? "") {
                keyStatus.stringValue = keySaved ? "API key saved. Access is checked when you send." : data["configured"] as? Bool == true ? "Using \(spec.envVar) from the environment." : "Add your key from \(spec.keyPortal) to send instructions."
            }
        case .failure(let error): keyStatus.stringValue = error.localizedDescription
        }
        refreshAvailability()
    }
    @objc private func storeKey() {
        let value = apiKey.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else { return }; updateKey(value)
    }
    @objc private func clearKey() { updateKey("") }
    private func updateKey(_ value: String) {
        guard supportsCredentials(), connected(), canApply(), !pending, !keyPending, !dismissed, let id = displayedProvider, let spec = Self.credentialSpec(for: id) else { return }
        keyPending = true; keyStatus.stringValue = "Saving key…"; refreshAvailability()
        let token = loadID
        request("\(spec.rpcPrefix).saveKey", ["apiKey": value]) { [weak self] result in
            guard let self, self.loadID == token, !self.dismissed else { return }
            self.keyPending = false
            if case .success = result { self.apiKey.stringValue = "" }
            self.installKeyStatus(result)
            if case .success = result { self.discoverModels(refresh: true) }
        }
    }
    func keyProbe(_ value: String) { apiKey.stringValue = value; storeKey() }
    func controlTextDidChange(_ obj: Notification) {
        if let field = obj.object as? NSTextField, field === providerFilter {
            let prev = displayedProvider
            rebuildProviderPopup(preferring: prev)
            if visibleProviders.indices.contains(provider.indexOfSelectedItem),
               visibleProviders[provider.indexOfSelectedItem].id != prev {
                changeProvider()
            } else {
                refreshAvailability()
            }
            return
        }
        if let field = obj.object as? NSTextField, field === apiKey { refreshAvailability(); return }
        if let displayedProvider { customProviders.insert(displayedProvider) }
        customMode = true; refreshAvailability()
    }
    @objc private func save() {
        guard !dismissed, !pending, connected(), reconciled, blocked == nil, canApply(), dirty, validation == nil, let selection, let id = displayedProvider else { return }
        pending = true; failure = nil; status.stringValue = "Saving selection…"; refreshAvailability()
        let token = loadID
        request("models.select", ["provider": id, "model": draftModel, "revision": selection.revision]) { [weak self] result in
            guard let self, self.loadID == token else { return }
            self.pending = false
            switch result {
            case .success(let data):
                self.install(data)
                if self.reconciled { self.customProviders.remove(id); self.customMode = false; self.dismiss() }
            case .failure(let error):
                self.reconciled = false
                self.failure = "\(error.localizedDescription) Your model edit is preserved. Choose Refresh to check the current selection."
            }
            self.refreshAvailability()
        }
    }
    func receive(_ data: [String: Any]) {
        guard !pending, let incoming = WorkspaceModelSelection(data), let selection, incoming.revision > selection.revision else { return }
        let preserve = dirty
        if let displayedProvider { remembered[displayedProvider] = model.stringValue }
        install(data, preserve: preserve); refreshAvailability()
    }
    /// A reconnect replaces the runtime and its revision space, never the user's draft.
    /// Invalidate in-flight callbacks before allowing the new client to reconcile.
    func connectionChanged() {
        if connected() { load(); return }
        loadID = UUID(); pending = false; keyPending = false; apiKey.stringValue = ""; reconciled = false; resetRevision = true
        discoveryRequests.removeAll(); discoveries.removeAll()
        refreshAvailability()
    }
    func dismiss(restoreFocus: Bool = true) {
        dismissed = true; loadID = UUID(); pending = false; keyPending = false; apiKey.stringValue = ""
        discoveryRequests.removeAll()
        refreshAvailability()
        let parent = window?.parent
        if let window { parent?.removeChildWindow(window); window.orderOut(nil) }
        if restoreFocus, let parent, parent.isVisible, NSApp.isActive {
            parent.makeKeyAndOrderFront(nil)
            if let previousResponder { parent.makeFirstResponder(previousResponder) }
        }
    }
    override func cancelOperation(_ sender: Any?) { dismiss() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { dismiss(); return false }
    func editProbe(_ value: String) {
        model.stringValue = value; controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: model))
    }
    func providerProbe(_ id: String) {
        if !providerFilter.stringValue.isEmpty, !visibleProviders.contains(where: { $0.id == id }) {
            providerFilter.stringValue = ""
            rebuildProviderPopup(preferring: id)
        }
        guard let index = visibleProviders.firstIndex(where: { $0.id == id }) else { return }
        provider.selectItem(at: index); changeProvider()
    }
    func providerFilterProbe(_ value: String) {
        providerFilter.stringValue = value
        controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: providerFilter))
    }
    func refreshProbe() { refreshProviderModels() }
    func selectProbe(_ id: String) { modelChoice.selectProbe(id) }
    func searchProbe(_ value: String) { modelChoice.searchProbe(value) }
    var diagnostics: [String: Any] {
        ["credentialsVisible": !credentialsRow.isHidden, "keyStatus": keyStatus.stringValue,
         "appleIntelligenceVisible": !appleIntelligenceRow.isHidden, "appleIntelligenceStatus": appleIntelligenceStatus.stringValue,
         "visible": window?.isVisible == true, "draft": model.stringValue, "dirty": dirty,
         "connected": connected(), "pending": pending, "reconciled": reconciled,
         "applyEnabled": apply.isEnabled, "status": status.stringValue, "availability": availability.stringValue,
         "catalogStatus": catalogStatus.stringValue, "modelInformation": modelInformation.stringValue, "modelSuggestions": suggestions, "customVisible": !customRow.isHidden, "list": modelChoice.diagnostics]
    }
    func dispose() {
        if let obs = accountsObserver {
            NotificationCenter.default.removeObserver(obs)
            accountsObserver = nil
        }
        window?.orderOut(nil)
    }
}
