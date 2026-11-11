// [LAYER: PLUMBING]
import Foundation
import CoreFoundation

struct WorkspaceModelSelection: Equatable {
    let provider: String
    let model: String
    let revision: Int
    let reasoningEffort: String
    let reasoningOptions: [String]
    let busy: Bool
    let blocked: String?
    static let reasoningLevels: Set<String> = ["none", "minimal", "low", "medium", "high", "xhigh", "max"]
    var providerLabel: String {
        switch provider {
        case "openrouter": return "OpenRouter"
        case "orcarouter": return "OrcaRouter"
        case "codewhale": return "Codewhale"
        case "cline-pass": return "Cline Pass"
        case "nous": return "Nous Research"
        case "opencode": return "OpenCode"
        case "opencode-go": return "OpenCode Go"
        case "apple-playground": return "On this Mac (Playground)"
        case "openai-codex": return "OpenAI Codex"
        case "openai": return "OpenAI API"
        case "anthropic": return "Anthropic"
        case "gemini": return "Google Gemini"
        case "deepseek", "deepseek-anthropic": return "DeepSeek"
        case "xai": return "xAI Grok"
        case "groq": return "Groq"
        case "mistral": return "Mistral AI"
        case "qwen", "qwen-anthropic": return "Qwen (Alibaba)"
        case "modelstudio-token-plan", "modelstudio-token-plan-anthropic": return "Alibaba Model Studio"
        case "modelstudio-coding-plan", "modelstudio-coding-plan-anthropic": return "Model Studio (Coding)"
        case "moonshot", "kimi-code": return "Moonshot Kimi"
        case "zai", "zai-anthropic": return "Z.ai GLM"
        case "minimax", "minimax-anthropic": return "MiniMax"
        case "volcengine": return "Volcengine Ark"
        case "wanjie-ark": return "Wanjie Ark"
        case "nvidia-nim": return "NVIDIA NIM"
        case "together": return "Together AI"
        case "fireworks": return "Fireworks AI"
        case "deepinfra": return "DeepInfra"
        case "atlascloud": return "AtlasCloud"
        case "siliconflow": return "SiliconFlow"
        case "siliconflow-CN": return "SiliconFlow (CN)"
        case "cerebras": return "Cerebras"
        case "baseten": return "Baseten"
        case "novita": return "Novita AI"
        case "arcee": return "Arcee AI"
        case "xiaomi-mimo": return "Xiaomi MiMo"
        case "stepfun": return "StepFun"
        case "huggingface": return "Hugging Face"
        case "modelscope": return "ModelScope"
        case "qianfan": return "Baidu Qianfan"
        case "openmodel": return "OpenModel"
        case "sakana": return "Sakana AI (Fugu)"
        case "longcat": return "Meituan LongCat"
        case "meta": return "Meta Model API"
        case "telecomjs": return "TelecomJS TokenHub"
        case "edenai": return "Eden AI"
        case "zenmux": return "ZenMux"
        case "csdn": return "CSDN Starmap"
        case "perplexity": return "Perplexity"
        case "cohere": return "Cohere"
        case "concentrate": return "Concentrate AI"
        case "ollama-cloud": return "Ollama Cloud"
        case "ollama": return "Ollama (Local)"
        case "vllm": return "vLLM (Local)"
        case "sglang": return "SGLang (Local)"
        case "lmstudio": return "LM Studio (Local)"
        case "custom": return "Custom Endpoint"
        default: return "Claude Code"
        }
    }
    static func providerCategory(for id: String) -> String {
        if ["ollama", "vllm", "sglang", "lmstudio", "apple-playground"].contains(id) { return "local zero-key offline" }
        if ["openai", "openai-codex", "anthropic", "claude-subscription-directsdk-experimental", "gemini", "deepseek", "deepseek-anthropic", "xai", "mistral", "qwen", "qwen-anthropic", "moonshot", "kimi-code", "zai", "zai-anthropic", "minimax", "minimax-anthropic", "meta"].contains(id) {
            return "frontier direct"
        }
        return "router gateway cloud"
    }
    static let providers: Set<String> = [
        "openai", "openai-codex", "claude-subscription-directsdk-experimental", "anthropic",
        "gemini", "deepseek", "deepseek-anthropic", "openrouter", "orcarouter", "xai", "groq", "mistral",
        "qwen", "qwen-anthropic", "modelstudio-token-plan", "modelstudio-token-plan-anthropic",
        "modelstudio-coding-plan", "modelstudio-coding-plan-anthropic", "moonshot", "kimi-code",
        "zai", "zai-anthropic", "minimax", "minimax-anthropic", "volcengine", "wanjie-ark",
        "nvidia-nim", "together", "fireworks", "deepinfra", "atlascloud", "siliconflow", "siliconflow-CN",
        "cerebras", "baseten", "novita", "arcee", "xiaomi-mimo", "stepfun", "huggingface", "modelscope",
        "qianfan", "openmodel", "sakana", "longcat", "meta", "telecomjs", "edenai", "zenmux", "csdn",
        "perplexity", "cohere", "concentrate", "codewhale", "opencode", "opencode-go", "cline-pass",
        "nous", "ollama-cloud", "ollama", "vllm", "sglang", "lmstudio", "apple-playground", "custom"
    ]
    static func validModel(_ value: String) -> Bool {
        value.utf8.count <= 200 && value.range(of: "^~?[A-Za-z0-9][A-Za-z0-9._:/\\[\\]-]*\\z", options: .regularExpression) != nil
    }
    init?(_ data: [String: Any]) {
        guard let provider = data["provider"] as? String, Self.providers.contains(provider),
              let model = data["model"] as? String, Self.validModel(model),
              let revision = data["revision"] as? NSNumber, CFGetTypeID(revision) != CFBooleanGetTypeID(),
              revision.doubleValue >= 0, revision.doubleValue <= 9_007_199_254_740_991,
              revision.doubleValue.rounded(.towardZero) == revision.doubleValue else { return nil }
        let effort = data["reasoningEffort"] as? String ?? "default"
        guard data["reasoningEffort"] == nil || data["reasoningEffort"] is String,
              effort == "default" || Self.reasoningLevels.contains(effort) else { return nil }
        let options = data["reasoningOptions"] as? [String] ?? []
        guard data["reasoningOptions"] == nil || data["reasoningOptions"] is [String],
              options.count <= Self.reasoningLevels.count, Set(options).count == options.count,
              options.allSatisfy(Self.reasoningLevels.contains),
              data["reasoningOptions"] == nil || effort == "default" || options.contains(effort) else { return nil }
        self.reasoningEffort = effort; self.reasoningOptions = options
        busy = data["busy"] as? Bool ?? false; blocked = data["blocked"] as? String
        self.provider = provider; self.model = model; self.revision = revision.intValue
    }
}

struct WorkspaceModelCatalog {
    struct Provider {
        let id: String
        let label: String
        let detail: String
        let models: [String]
    }
    let selection: WorkspaceModelSelection
    let providers: [Provider]
    let recent: [WorkspaceModelSelection]
    let busy: Bool
    let blocked: String?
    init?(_ data: [String: Any]) {
        guard let selection = WorkspaceModelSelection(data), let raw = data["providers"] as? [[String: Any]],
              !raw.isEmpty, raw.count <= 128 else { return nil }
        var providers = [Provider](), ids = Set<String>()
        for item in raw {
            guard let id = item["id"] as? String, WorkspaceModelSelection.providers.contains(id), ids.insert(id).inserted,
                  let label = item["label"] as? String, !label.isEmpty, label.count <= 100,
                  let detail = item["detail"] as? String, detail.count <= 2000,
                  let models = item["models"] as? [String], models.count <= 1000,
                  models.allSatisfy(WorkspaceModelSelection.validModel) else { return nil }
            providers.append(Provider(id: id, label: label, detail: detail, models: models))
        }
        guard ids.contains(selection.provider) else { return nil }
        self.selection = selection; self.providers = providers; busy = data["busy"] as? Bool ?? false
        blocked = data["blocked"] as? String
        var recent = [WorkspaceModelSelection]()
        for item in (data["recent"] as? [[String: Any]] ?? []).prefix(8) {
            guard let value = WorkspaceModelSelection(item.merging(["revision": selection.revision]) { _, new in new }),
                  ids.contains(value.provider), !recent.contains(value) else { continue }
            recent.append(value)
        }
        self.recent = recent
    }
}

struct WorkspaceModelDiscovery {
    struct Model { let id: String; let label: String; let detail: String }
    let provider: String
    let models: [Model]
    let state: String
    let source: String
    let message: String
    let checkedAt: Date?
    let limited: Bool
    init?(_ data: [String: Any]) {
        guard let provider = data["provider"] as? String, WorkspaceModelSelection.providers.contains(provider),
              let state = data["state"] as? String, ["ready", "empty", "stale", "unavailable"].contains(state),
              let source = data["source"] as? String, !source.isEmpty, source.count <= 100,
              let message = data["message"] as? String, message.count <= 2000,
              let raw = data["models"] as? [[String: Any]], raw.count <= 1000 else { return nil }
        var models = [Model](), ids = Set<String>()
        for item in raw {
            guard let id = item["id"] as? String, WorkspaceModelSelection.validModel(id), ids.insert(id).inserted,
                  let label = item["label"] as? String, label.count <= 160,
                  let detail = item["detail"] as? String, detail.count <= 240 else { return nil }
            models.append(Model(id: id, label: label, detail: detail))
        }
        if let timestamp = data["checkedAt"] {
            guard let value = timestamp as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.doubleValue.isFinite, value.doubleValue >= 0 else { return nil }
            checkedAt = Date(timeIntervalSince1970: value.doubleValue / 1000)
        } else { checkedAt = nil }
        self.provider = provider; self.models = models; self.state = state
        self.source = source; self.message = message; limited = data["limited"] as? Bool ?? false
    }
    var summary: String {
        let time = checkedAt.map { " · Checked " + DateFormatter.localizedString(from: $0, dateStyle: .none, timeStyle: .short) } ?? ""
        switch state {
        case "ready": return "\(models.count) models from \(source)\(limited ? " · limited list" : "")\(time)"
        case "stale": return "Saved \(source) list · Could not refresh\(time)"
        case "empty": return "No models returned by \(source)"
        default: return "Models unavailable"
        }
    }
}
