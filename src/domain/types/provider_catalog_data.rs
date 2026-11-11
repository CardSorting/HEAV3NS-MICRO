// [LAYER: DOMAIN]
use crate::plumbing::hash_and_math::levenshtein;
use serde::{Deserialize, Serialize};

/// Canonical wire transport dialect across all 65+ providers (`Codewhale wire.rs` parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireDialect {
    ChatCompletions,
    AnthropicMessages,
    OpenAiResponses,
    GoogleGemini,
}

impl WireDialect {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::AnthropicMessages => "anthropic_messages",
            Self::OpenAiResponses => "openai_responses",
            Self::GoogleGemini => "google_gemini",
        }
    }

    pub fn parse_override(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "chat" | "chat_completions" | "openai" | "openai_chat" => Some(Self::ChatCompletions),
            "anthropic" | "anthropic_messages" | "messages" | "claude" => Some(Self::AnthropicMessages),
            "responses" | "openai_responses" | "codex" => Some(Self::OpenAiResponses),
            "gemini" | "google" | "google_gemini" => Some(Self::GoogleGemini),
            _ => None,
        }
    }
}

/// Billing semantics declared per provider descriptor (`Codewhale Honest Pricing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBillingClass {
    Metered,
    LocalNoBill,
    Subscription,
}

/// Static sovereign metadata for a canonical LLM provider (`Codewhale provider_descriptors.json` distilled).
#[derive(Debug, Clone, Copy)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub env_vars: &'static [&'static str],
    pub endpoint_env: Option<&'static str>,
    pub default_endpoint: &'static str,
    pub default_model: &'static str,
    pub wire_dialect: WireDialect,
    pub responses_profile: Option<&'static str>,
    pub billing_class: ProviderBillingClass,
    pub secret_slot: &'static str,
    pub catalog_vendor: &'static str,
    pub requires_api_key: bool,
    pub credential_url: &'static str,
    pub model_aware: bool,
    pub retired: bool,
}

pub(crate) fn normalize_provider_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.contains('-') && !trimmed.contains('_') && !trimmed.contains('/') {
        let mut camel_kebab = String::with_capacity(trimmed.len() + 4);
        let mut prev_lower = false;
        for ch in trimmed.chars() {
            if ch.is_ascii_uppercase() && prev_lower {
                camel_kebab.push('-');
            }
            camel_kebab.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
        if camel_kebab != trimmed.to_ascii_lowercase() {
            return camel_kebab;
        }
    }
    trimmed.to_ascii_lowercase().replace('_', "-")
}

/// Looks up a [`ProviderDescriptor`] by canonical ID, camelCase RPC prefix (`clinePass`, `siliconflowCN`), alias, or shared `secret_slot`.
pub fn lookup_provider_descriptor(raw: &str) -> Option<&'static ProviderDescriptor> {
    let norm = normalize_provider_token(raw);
    if norm.is_empty() {
        return None;
    }
    PROVIDER_DESCRIPTORS
        .iter()
        .find(|d| {
            normalize_provider_token(d.id) == norm
                || d.aliases.iter().any(|a| normalize_provider_token(a) == norm)
        })
        .or_else(|| {
            PROVIDER_DESCRIPTORS
                .iter()
                .find(|d| normalize_provider_token(d.secret_slot) == norm)
        })
}

macro_rules! p {
    ($id:expr, $name:expr, $aliases:expr, $envs:expr, $ep_env:expr, $ep:expr, $model:expr, $wire:ident, $rprof:expr, $bill:ident, $slot:expr, $vendor:expr, $req_key:expr, $url:expr) => {
        ProviderDescriptor {
            id: $id,
            display_name: $name,
            aliases: $aliases,
            env_vars: $envs,
            endpoint_env: $ep_env,
            default_endpoint: $ep,
            default_model: $model,
            wire_dialect: WireDialect::$wire,
            responses_profile: $rprof,
            billing_class: ProviderBillingClass::$bill,
            secret_slot: $slot,
            catalog_vendor: $vendor,
            requires_api_key: $req_key,
            credential_url: $url,
            model_aware: false,
            retired: false,
        }
    };
    (model_aware: $id:expr, $name:expr, $aliases:expr, $envs:expr, $ep_env:expr, $ep:expr, $model:expr, $wire:ident, $rprof:expr, $bill:ident, $slot:expr, $vendor:expr, $req_key:expr, $url:expr) => {
        ProviderDescriptor {
            id: $id,
            display_name: $name,
            aliases: $aliases,
            env_vars: $envs,
            endpoint_env: $ep_env,
            default_endpoint: $ep,
            default_model: $model,
            wire_dialect: WireDialect::$wire,
            responses_profile: $rprof,
            billing_class: ProviderBillingClass::$bill,
            secret_slot: $slot,
            catalog_vendor: $vendor,
            requires_api_key: $req_key,
            credential_url: $url,
            model_aware: true,
            retired: false,
        }
    };
}

/// Complete sovereign provider registry (`65` active providers + `1` retired tombstone = `66` descriptors; full `Codewhale provider_descriptors.json` parity).
pub const PROVIDER_DESCRIPTORS: &[ProviderDescriptor] = &[
    p!("openai", "OpenAI API", &["openai-api", "gpt", "chatgpt-api"], &["OPENAI_API_KEY"], Some("OPENAI_BASE_URL"), "https://api.openai.com/v1", "gpt-5.6-terra", ChatCompletions, Some("standard"), Metered, "openai", "openai", true, "https://platform.openai.com/api-keys"),
    p!("openai-codex", "OpenAI Codex (Responses)", &["codex", "chatgpt", "openai-responses"], &["CODEX_API_KEY", "OPENAI_API_KEY"], Some("OPENAI_BASE_URL"), "https://api.openai.com/v1", "gpt-5.4", OpenAiResponses, Some("codex"), Subscription, "openai-codex", "openai", true, "https://platform.openai.com/api-keys"),
    p!("anthropic", "Anthropic Claude", &["claude", "anthropic-api"], &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"], Some("ANTHROPIC_BASE_URL"), "https://api.anthropic.com/v1", "claude-sonnet-5.5", AnthropicMessages, None, Metered, "anthropic", "anthropic", true, "https://console.anthropic.com/settings/keys"),
    p!("claude-subscription-directsdk-experimental", "Claude Subscription", &["claude-subscription", "claude-max", "claude-code"], &["ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY"], Some("ANTHROPIC_BASE_URL"), "https://api.anthropic.com/v1", "claude-sonnet-4.6", AnthropicMessages, None, Subscription, "claude-subscription", "anthropic", true, "https://claude.ai/settings"),
    p!("gemini", "Google Gemini", &["google", "google-gemini", "ai-studio", "gemini-api"], &["GEMINI_API_KEY", "GOOGLE_API_KEY"], Some("GEMINI_BASE_URL"), "https://generativelanguage.googleapis.com/v1beta", "gemini-4-carbon", GoogleGemini, None, Metered, "gemini", "google", true, "https://aistudio.google.com/apikey"),
    p!(model_aware: "deepseek", "DeepSeek", &["deepseek-chat", "deepseek-v4"], &["DEEPSEEK_API_KEY"], Some("DEEPSEEK_BASE_URL"), "https://api.deepseek.com/v1", "deepseek-v4-pro", ChatCompletions, Some("deepseek_stateless_plain_reasoning"), Metered, "deepseek", "deepseek", true, "https://platform.deepseek.com/api_keys"),
    p!("deepseek-anthropic", "DeepSeek (Anthropic Wire)", &["deepseek-claude"], &["DEEPSEEK_API_KEY"], Some("DEEPSEEK_ANTHROPIC_BASE_URL"), "https://api.deepseek.com/anthropic/v1", "deepseek-chat", AnthropicMessages, None, Metered, "deepseek", "deepseek", true, "https://platform.deepseek.com/api_keys"),
    p!("openrouter", "OpenRouter", &["or"], &["OPENROUTER_API_KEY"], Some("OPENROUTER_BASE_URL"), "https://openrouter.ai/api/v1", "anthropic/claude-sonnet-5.5", ChatCompletions, None, Metered, "openrouter", "openrouter", true, "https://openrouter.ai/keys"),
    p!("orcarouter", "OrcaRouter", &["orca-router", "orca_router"], &["ORCAROUTER_API_KEY"], Some("ORCAROUTER_BASE_URL"), "https://api.orcarouter.ai/v1", "deepseek/deepseek-v4.1-flash-1017", ChatCompletions, None, Metered, "orcarouter", "openrouter", true, "https://www.orcarouter.ai"),
    p!("xai", "xAI Grok", &["x-ai", "grok"], &["XAI_API_KEY"], Some("XAI_BASE_URL"), "https://api.x.ai/v1", "grok-4.7", ChatCompletions, None, Metered, "xai", "x-ai", true, "https://console.x.ai"),
    p!("groq", "Groq", &["groqcloud"], &["GROQ_API_KEY"], Some("GROQ_BASE_URL"), "https://api.groq.com/openai/v1", "llama-3.3-70b-versatile", ChatCompletions, None, Metered, "groq", "groq", true, "https://console.groq.com/keys"),
    p!("mistral", "Mistral AI", &["mistralai", "codestral"], &["MISTRAL_API_KEY"], Some("MISTRAL_BASE_URL"), "https://api.mistral.ai/v1", "mistral-large-latest", ChatCompletions, None, Metered, "mistral", "mistralai", true, "https://console.mistral.ai/api-keys"),
    p!("qwen", "Alibaba Qwen (DashScope)", &["dashscope", "alibaba", "aliyun", "tongyi"], &["DASHSCOPE_API_KEY", "QWEN_API_KEY"], Some("DASHSCOPE_BASE_URL"), "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", "qwen3.8-max-prime", ChatCompletions, None, Metered, "qwen", "qwen", true, "https://bailian.console.aliyun.com"),
    p!("qwen-anthropic", "Qwen (Anthropic Wire)", &["dashscope-anthropic"], &["DASHSCOPE_API_KEY", "QWEN_API_KEY"], None, "https://dashscope-intl.aliyuncs.com/apps/anthropic/v1", "qwen3.8-max-prime", AnthropicMessages, None, Metered, "qwen", "qwen", true, "https://bailian.console.aliyun.com"),
    p!("modelstudio-token-plan", "Alibaba Cloud Model Studio", &["modelstudio", "alibaba-token-plan", "dashscope-token-plan", "modelstudio_token_plan"], &["MODELSTUDIO_API_KEY", "DASHSCOPE_API_KEY"], Some("MODELSTUDIO_BASE_URL"), "https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1", "qwen3.8-max", ChatCompletions, None, Subscription, "modelstudio-token-plan", "qwen", true, "https://modelstudio.console.alibabacloud.com/"),
    p!("modelstudio-token-plan-anthropic", "Model Studio (Token Plan, Anthropic)", &["alibaba-token-plan-anthropic", "modelstudio_token_plan_anthropic"], &["MODELSTUDIO_API_KEY", "DASHSCOPE_API_KEY"], None, "https://token-plan.ap-southeast-1.maas.aliyuncs.com/apps/anthropic", "qwen3.8-max", AnthropicMessages, None, Subscription, "modelstudio-token-plan", "qwen", true, "https://modelstudio.console.alibabacloud.com/"),
    p!("modelstudio-coding-plan", "Model Studio (Coding Plan)", &["alibaba-coding-plan", "dashscope-coding-plan", "modelstudio_coding_plan"], &["MODELSTUDIO_API_KEY", "DASHSCOPE_API_KEY"], None, "https://coding-intl.dashscope.aliyuncs.com/v1", "qwen3.8-max", ChatCompletions, None, Subscription, "modelstudio-token-plan", "qwen", true, "https://modelstudio.console.alibabacloud.com/"),
    p!("modelstudio-coding-plan-anthropic", "Model Studio (Coding Plan, Anthropic)", &["alibaba-coding-plan-anthropic", "modelstudio_coding_plan_anthropic"], &["MODELSTUDIO_API_KEY", "DASHSCOPE_API_KEY"], None, "https://coding-intl.dashscope.aliyuncs.com/apps/anthropic", "qwen3.8-max", AnthropicMessages, None, Subscription, "modelstudio-token-plan", "qwen", true, "https://modelstudio.console.alibabacloud.com/"),
    p!("moonshot", "Moonshot Kimi", &["moonshotai", "kimi"], &["MOONSHOT_API_KEY", "KIMI_API_KEY"], Some("MOONSHOT_BASE_URL"), "https://api.moonshot.ai/v1", "kimi-k3.1-pro", ChatCompletions, None, Metered, "moonshot", "moonshotai", true, "https://platform.moonshot.ai/console/api-keys"),
    p!("kimi-code", "Kimi Code (Anthropic Wire)", &["kimi-coding", "moonshot-anthropic"], &["KIMI_CODE_API_KEY", "MOONSHOT_API_KEY"], None, "https://api.kimi.com/coding/v1", "kimi-k2.7-code", AnthropicMessages, None, Subscription, "kimi-code", "moonshotai", true, "https://www.kimi.com/code"),
    p!("zai", "Z.ai GLM", &["z-ai", "zhipu", "glm", "bigmodel"], &["ZAI_API_KEY", "ZHIPU_API_KEY"], Some("ZAI_BASE_URL"), "https://api.z.ai/api/paas/v4", "glm-5.3-flashx-legal", ChatCompletions, None, Metered, "zai", "z-ai", true, "https://z.ai/manage-apikey/apikey-list"),
    p!("zai-anthropic", "Z.ai GLM (Anthropic Wire)", &["glm-anthropic", "zhipu-anthropic"], &["ZAI_API_KEY", "ZHIPU_API_KEY"], None, "https://api.z.ai/api/anthropic/v1", "glm-5.3-flashx-legal", AnthropicMessages, None, Metered, "zai", "z-ai", true, "https://z.ai/manage-apikey/apikey-list"),
    p!("minimax", "MiniMax", &["minimax-io"], &["MINIMAX_API_KEY"], Some("MINIMAX_BASE_URL"), "https://api.minimax.io/v1", "MiniMax-M2.5", ChatCompletions, None, Metered, "minimax", "minimax", true, "https://platform.minimax.io"),
    p!("minimax-anthropic", "MiniMax (Anthropic Wire)", &["minimax-claude"], &["MINIMAX_API_KEY"], None, "https://api.minimax.io/anthropic/v1", "MiniMax-M2.5", AnthropicMessages, None, Metered, "minimax", "minimax", true, "https://platform.minimax.io"),
    p!("volcengine", "Volcengine Ark", &["ark", "doubao", "bytedance"], &["ARK_API_KEY", "VOLCENGINE_API_KEY"], Some("ARK_BASE_URL"), "https://ark.cn-beijing.volces.com/api/v3", "doubao-seed-2.0-pro", ChatCompletions, None, Metered, "volcengine", "bytedance", true, "https://console.volcengine.com/ark"),
    p!("wanjie-ark", "Wanjie Ark", &["wanjie", "wanjie_ark", "ark-wanjie", "wanjieark", "wanjie-maas"], &["WANJIE_ARK_API_KEY", "WANJIE_API_KEY", "WANJIE_MAAS_API_KEY"], Some("WANJIE_ARK_BASE_URL"), "https://maas-openapi.wanjiedata.com/api/v1", "deepseek-reasoner", ChatCompletions, None, Metered, "wanjie-ark", "deepseek", true, "https://wanjiedata.com"),
    p!("nvidia-nim", "NVIDIA NIM", &["nvidia", "nim", "build-nvidia"], &["NVIDIA_API_KEY", "NGC_API_KEY"], Some("NVIDIA_BASE_URL"), "https://integrate.api.nvidia.com/v1", "nvidia/llama-3.3-nemotron-super-49b-v1", ChatCompletions, None, Metered, "nvidia-nim", "nvidia", true, "https://build.nvidia.com"),
    p!("together", "Together AI", &["togetherai", "together-ai"], &["TOGETHER_API_KEY"], Some("TOGETHER_BASE_URL"), "https://api.together.xyz/v1", "deepseek-ai/DeepSeek-V3", ChatCompletions, None, Metered, "together", "together", true, "https://api.together.ai/settings/api-keys"),
    p!("fireworks", "Fireworks AI", &["fireworks-ai", "fw"], &["FIREWORKS_API_KEY"], Some("FIREWORKS_BASE_URL"), "https://api.fireworks.ai/inference/v1", "accounts/fireworks/models/deepseek-v3", ChatCompletions, None, Metered, "fireworks", "fireworks", true, "https://fireworks.ai/account/api-keys"),
    p!("deepinfra", "DeepInfra", &["deep-infra"], &["DEEPINFRA_API_KEY"], Some("DEEPINFRA_BASE_URL"), "https://api.deepinfra.com/v1/openai", "deepseek-ai/DeepSeek-V3", ChatCompletions, None, Metered, "deepinfra", "deepinfra", true, "https://deepinfra.com/dash/api_keys"),
    p!("atlascloud", "AtlasCloud", &["atlas-cloud", "atlas_cloud", "atlas"], &["ATLASCLOUD_API_KEY"], Some("ATLASCLOUD_BASE_URL"), "https://api.atlascloud.ai/v1", "deepseek-ai/deepseek-v4-flash", ChatCompletions, None, Metered, "atlascloud", "deepseek", true, "https://www.atlascloud.ai/console/api-keys"),
    p!("siliconflow", "SiliconFlow", &["sf", "silicon-flow"], &["SILICONFLOW_API_KEY"], Some("SILICONFLOW_BASE_URL"), "https://api.siliconflow.com/v1", "deepseek-ai/DeepSeek-V3", ChatCompletions, None, Metered, "siliconflow", "deepseek", true, "https://cloud.siliconflow.com/account/ak"),
    p!("siliconflow-CN", "SiliconFlow (CN)", &["siliconflow-cn", "siliconflow_cn", "siliconflowcn"], &["SILICONFLOW_API_KEY"], Some("SILICONFLOW_CN_BASE_URL"), "https://api.siliconflow.cn/v1", "deepseek-ai/DeepSeek-V4-Pro", ChatCompletions, None, Metered, "siliconflow", "deepseek", true, "https://cloud.siliconflow.cn/account/ak"),
    p!("cerebras", "Cerebras", &["cerebras-ai"], &["CEREBRAS_API_KEY"], Some("CEREBRAS_BASE_URL"), "https://api.cerebras.ai/v1", "llama-3.3-70b", ChatCompletions, None, Metered, "cerebras", "cerebras", true, "https://cloud.cerebras.ai"),
    p!("baseten", "Baseten", &["baseten-ai"], &["BASETEN_API_KEY"], Some("BASETEN_BASE_URL"), "https://inference.baseten.co/v1", "deepseek-ai/DeepSeek-V3", ChatCompletions, None, Metered, "baseten", "deepseek", true, "https://app.baseten.co/settings/api_keys"),
    p!("novita", "Novita AI", &["novita-ai", "novitaai"], &["NOVITA_API_KEY"], Some("NOVITA_BASE_URL"), "https://api.novita.ai/v3/openai", "deepseek/deepseek-v3", ChatCompletions, None, Metered, "novita", "deepseek", true, "https://novita.ai/settings/key-management"),
    p!("arcee", "Arcee AI", &["arcee-ai", "arcee_ai"], &["ARCEE_API_KEY"], Some("ARCEE_BASE_URL"), "https://api.arcee.ai/api/v1", "trinity-large-thinking", ChatCompletions, None, Metered, "arcee", "arcee-ai", true, "https://chat.arcee.ai/account/api-keys"),
    p!("xiaomi-mimo", "Xiaomi MiMo", &["xiaomi", "mimo"], &["MIMO_API_KEY", "XIAOMI_API_KEY"], Some("MIMO_BASE_URL"), "https://api.xiaomimimo.com/v1", "mimo-v2-flash", ChatCompletions, None, Metered, "xiaomi-mimo", "xiaomi", true, "https://platform.xiaomimimo.com"),
    p!("stepfun", "StepFun", &["step", "step-ai"], &["STEPFUN_API_KEY"], Some("STEPFUN_BASE_URL"), "https://api.stepfun.com/v1", "step-3", ChatCompletions, None, Metered, "stepfun", "stepfun", true, "https://platform.stepfun.com"),
    p!("huggingface", "Hugging Face Router", &["hf", "hf-inference"], &["HF_TOKEN", "HUGGINGFACE_API_KEY"], Some("HF_BASE_URL"), "https://router.huggingface.co/v1", "Qwen/Qwen3-Coder-480B-A35B-Instruct", ChatCompletions, None, Metered, "huggingface", "qwen", true, "https://huggingface.co/settings/tokens"),
    p!("modelscope", "ModelScope", &["model-scope", "model_scope", "modelscope-cn"], &["MODELSCOPE_API_KEY"], Some("MODELSCOPE_BASE_URL"), "https://api-inference.modelscope.cn/v1", "Qwen/Qwen3.5-397B-A17B", ChatCompletions, None, Metered, "modelscope", "qwen", true, "https://modelscope.cn/my/myaccesstoken"),
    p!("qianfan", "Baidu Qianfan", &["baidu-qianfan", "baidu_qianfan", "baidu"], &["QIANFAN_API_KEY", "BAIDU_QIANFAN_API_KEY"], Some("QIANFAN_BASE_URL"), "https://api.baiduqianfan.ai/v1", "ernie-4.0-turbo-8k", ChatCompletions, None, Metered, "qianfan", "baidu", true, "https://console.bce.baidu.com/iam/#/iam/apikey/list"),
    p!("openmodel", "OpenModel", &["open-model", "open_model"], &["OPENMODEL_API_KEY"], Some("OPENMODEL_BASE_URL"), "https://api.openmodel.ai", "deepseek-v4-flash", AnthropicMessages, None, Metered, "openmodel", "deepseek", true, "https://console.openmodel.ai/api-keys"),
    p!("sakana", "Sakana AI (Fugu)", &["sakana-ai", "sakana_ai", "fugu"], &["FUGU_API_KEY", "SAKANA_API_KEY"], Some("SAKANA_BASE_URL"), "https://api.sakana.ai/v1", "fugu", ChatCompletions, None, Metered, "sakana", "sakana", true, "https://console.sakana.ai/"),
    p!("longcat", "Meituan LongCat", &["long-cat", "long_cat", "meituan-longcat", "meituan"], &["LONGCAT_API_KEY"], Some("LONGCAT_BASE_URL"), "https://api.longcat.chat/openai/v1", "LongCat-2.0", ChatCompletions, None, Metered, "longcat", "longcat", true, "https://longcat.chat/platform/api_keys"),
    p!("meta", "Meta Model API", &["meta-ai", "meta_ai", "meta-model-api", "muse", "muse-spark"], &["META_MODEL_API_KEY", "MODEL_API_KEY"], Some("META_BASE_URL"), "https://api.meta.ai/v1", "muse-spark-1.3", ChatCompletions, None, Metered, "meta", "meta", true, "https://developers.facebook.com"),
    p!("telecomjs", "TelecomJS TokenHub", &["telecom-js", "telecom_js", "telecomjs-cn", "tokenhub"], &["TELECOMJS_API_KEY"], Some("TELECOMJS_BASE_URL"), "https://aigw.telecomjs.com/v1", "deepseek-v4-pro", ChatCompletions, None, Metered, "telecomjs", "deepseek", true, "https://aigw.telecomjs.com/"),
    p!("edenai", "Eden AI", &["eden-ai", "eden_ai"], &["EDENAI_API_KEY"], Some("EDENAI_BASE_URL"), "https://api.edenai.run/v3", "deepseek/deepseek-v4-pro", ChatCompletions, None, Metered, "edenai", "openrouter", true, "https://app.edenai.run/admin/account/developer"),
    p!("zenmux", "ZenMux", &["zen-mux", "zen_mux"], &["ZENMUX_API_KEY"], Some("ZENMUX_BASE_URL"), "https://zenmux.ai/api/v1", "deepseek/deepseek-v4.1-flash", ChatCompletions, None, Metered, "zenmux", "openrouter", true, "https://zenmux.ai/settings/keys"),
    p!("csdn", "CSDN Starmap", &["csdn-ai", "csdn_ai", "csdn-coding-plan", "starmap"], &["CSDN_API_KEY"], Some("CSDN_BASE_URL"), "https://ai.csdn.net/api/model/v1", "glm_for_coding", ChatCompletions, None, Metered, "csdn", "z-ai", true, "https://ai.csdn.net"),
    p!("perplexity", "Perplexity Sonar", &["pplx", "sonar"], &["PERPLEXITY_API_KEY", "PPLX_API_KEY"], None, "https://api.perplexity.ai", "sonar-reasoning-pro", ChatCompletions, None, Metered, "perplexity", "perplexity", true, "https://www.perplexity.ai/settings/api"),
    p!("cohere", "Cohere", &["command-r", "command-a"], &["COHERE_API_KEY", "CO_API_KEY"], None, "https://api.cohere.ai/compatibility/v1", "command-a-03-2025", ChatCompletions, None, Metered, "cohere", "cohere", true, "https://dashboard.cohere.com/api-keys"),
    p!("concentrate", "Concentrate AI", &["concentrate-ai"], &["CONCENTRATE_API_KEY"], Some("CONCENTRATE_BASE_URL"), "https://api.concentrate.ai/v1", "gpt-5.4", OpenAiResponses, Some("concentrate"), Metered, "concentrate", "openai", true, "https://concentrate.ai"),
    p!(model_aware: "codewhale", "Codewhale", &["codewhale-api", "codewhale_api", "cw-api", "codewhale-cloud"], &["CODEWHALE_API_KEY"], Some("CODEWHALE_API_BASE"), "https://api.codewhale.net/v1", "deepseek/deepseek-v4-pro", ChatCompletions, None, Metered, "codewhale", "openrouter", true, "https://codewhale.net"),
    p!(model_aware: "opencode", "OpenCode Zen", &["opencode-zen", "opencode_zen", "zen"], &["OPENCODE_API_KEY"], Some("OPENCODE_BASE_URL"), "https://opencode.ai/zen/v1", "claude-sonnet-4.6", ChatCompletions, None, Metered, "opencode", "anthropic", true, "https://opencode.ai/zen"),
    p!(model_aware: "opencode-go", "OpenCode Go", &["opencode_go", "opencodego"], &["OPENCODE_GO_API_KEY"], Some("OPENCODE_GO_BASE_URL"), "https://opencode.ai/zen/go/v1", "deepseek-v4-pro", ChatCompletions, None, Subscription, "opencode-go", "deepseek", true, "https://opencode.ai/auth"),
    p!("cline-pass", "Cline Pass", &["cline", "clinepass"], &["CLINE_API_KEY", "CLINE_PASS_API_KEY"], Some("CLINE_BASE_URL"), "https://api.cline.bot/api/v1", "anthropic/claude-sonnet-4.6", ChatCompletions, None, Metered, "cline-pass", "openrouter", true, "https://app.cline.bot"),
    p!("nous", "Nous Research", &["nousresearch", "hermes"], &["NOUS_API_KEY"], Some("NOUS_BASE_URL"), "https://inference-api.nousresearch.com/v1", "Hermes-4-70B", ChatCompletions, None, Metered, "nous", "nousresearch", true, "https://portal.nousresearch.com"),
    p!("ollama-cloud", "Ollama Cloud", &["ollama_cloud", "ollamacloud"], &["OLLAMA_CLOUD_API_KEY", "OLLAMA_API_KEY"], Some("OLLAMA_CLOUD_BASE_URL"), "https://ollama.com/v1", "gpt-oss:120b", ChatCompletions, None, Metered, "ollama-cloud", "ollama", true, "https://ollama.com/settings/keys"),
    p!("ollama", "Ollama (Local)", &["ollama-local"], &["OLLAMA_API_KEY"], Some("OLLAMA_BASE_URL"), "http://127.0.0.1:11434/v1", "qwen2.5-coder:14b", ChatCompletions, None, LocalNoBill, "ollama", "local", false, "https://ollama.com"),
    p!("vllm", "vLLM (Local)", &["vllm-local"], &["VLLM_API_KEY"], Some("VLLM_BASE_URL"), "http://127.0.0.1:8000/v1", "Qwen/Qwen2.5-Coder-32B-Instruct", ChatCompletions, None, LocalNoBill, "vllm", "local", false, "https://docs.vllm.ai"),
    p!("sglang", "SGLang (Local)", &["sglang-local"], &["SGLANG_API_KEY"], Some("SGLANG_BASE_URL"), "http://127.0.0.1:30000/v1", "Qwen/Qwen2.5-Coder-32B-Instruct", ChatCompletions, None, LocalNoBill, "sglang", "local", false, "https://docs.sglang.ai"),
    p!("lmstudio", "LM Studio (Local)", &["lm-studio", "lms"], &["LM_STUDIO_API_KEY"], Some("LM_STUDIO_BASE_URL"), "http://127.0.0.1:1234/v1", "qwen2.5-coder-14b-instruct", ChatCompletions, None, LocalNoBill, "lmstudio", "local", false, "https://lmstudio.ai"),
    p!("apple-playground", "On-Device Playground", &["apple", "foundation-models", "apple-intelligence"], &["HEAV3NS_LOCAL_MODEL_TOKEN"], Some("HEAV3NS_LOCAL_MODEL_URL"), "http://127.0.0.1:11434/v1", "apple-foundation-local", ChatCompletions, None, LocalNoBill, "apple-playground", "local", false, "https://developer.apple.com/documentation/foundationmodels"),
    p!("custom", "Custom OpenAI Gateway", &["openai-compatible", "litellm", "gateway"], &["CUSTOM_LLM_API_KEY", "LITELLM_API_KEY"], Some("CUSTOM_LLM_BASE_URL"), "http://127.0.0.1:4000/v1", "gpt-5.4", ChatCompletions, None, Metered, "custom", "custom", false, "https://docs.litellm.ai"),
    ProviderDescriptor {
        id: "antigravity",
        display_name: "Antigravity (Retired)",
        aliases: &["agy"],
        env_vars: &[],
        endpoint_env: None,
        default_endpoint: "https://generativelanguage.googleapis.com/v1beta/openai",
        default_model: "gemini-3-pro",
        wire_dialect: WireDialect::ChatCompletions,
        responses_profile: None,
        billing_class: ProviderBillingClass::Metered,
        secret_slot: "antigravity",
        catalog_vendor: "google",
        requires_api_key: true,
        credential_url: "https://aistudio.google.com/apikey",
        model_aware: false,
        retired: true,
    },
];

pub const LEGACY_ANTIGRAVITY_TOMBSTONE_MESSAGE: &str =
    "Provider 'antigravity' ('agy') is retired and disabled; switch to 'gemini' (GEMINI_API_KEY) instead.";

/// Returns a fail-closed remediation message when `raw` names a retired provider tombstone (`antigravity` / `agy`).
pub fn retired_provider_remediation(raw: &str) -> Option<&'static str> {
    let desc = lookup_provider_descriptor(raw)?;
    if desc.retired {
        Some(LEGACY_ANTIGRAVITY_TOMBSTONE_MESSAGE)
    } else {
        None
    }
}

/// Returns `true` when `raw` resolves to a retired provider tombstone.
#[inline]
pub fn is_retired_provider(raw: &str) -> bool {
    retired_provider_remediation(raw).is_some()
}

/// OpenCode Go's closed transport roster: `(canonical_wire_id, WireDialect)` (`Codewhale opencode_go.rs` parity).
pub const OPENCODE_GO_TRANSPORTS: &[(&str, WireDialect)] = &[
    ("deepseek-v4-pro", WireDialect::ChatCompletions),
    ("grok-4.5", WireDialect::ChatCompletions),
    ("glm-5.2", WireDialect::ChatCompletions),
    ("glm-5.1", WireDialect::ChatCompletions),
    ("kimi-k3", WireDialect::ChatCompletions),
    ("kimi-k2.7-code", WireDialect::ChatCompletions),
    ("kimi-k2.6", WireDialect::ChatCompletions),
    ("deepseek-v4-flash", WireDialect::ChatCompletions),
    ("mimo-v2.5", WireDialect::ChatCompletions),
    ("mimo-v2.5-pro", WireDialect::ChatCompletions),
    ("glm-5.3-flash", WireDialect::ChatCompletions),
    ("glm-5.3", WireDialect::ChatCompletions),
    ("longcat-2.0", WireDialect::ChatCompletions),
    ("deepseek-v4-flash-vision-exp", WireDialect::ChatCompletions),
    ("hy4-preview", WireDialect::ChatCompletions),
    ("hy3", WireDialect::ChatCompletions),
    ("omen-alpha", WireDialect::ChatCompletions),
    ("deepseek-v4.1-flash", WireDialect::ChatCompletions),
    ("grok-4.6", WireDialect::OpenAiResponses),
    ("gpt-5.6-luna", WireDialect::OpenAiResponses),
    ("muse-spark-1.3-contributor", WireDialect::OpenAiResponses),
    ("muse-spark-1.2-contributor", WireDialect::OpenAiResponses),
    ("minimax-m3", WireDialect::AnthropicMessages),
    ("minimax-m2.7", WireDialect::AnthropicMessages),
    ("minimax-m2.5", WireDialect::AnthropicMessages),
    ("qwen3.8-max", WireDialect::AnthropicMessages),
    ("qwen3.8-flash", WireDialect::AnthropicMessages),
    ("qwen3.7-max", WireDialect::AnthropicMessages),
    ("qwen3.7-plus", WireDialect::AnthropicMessages),
    ("qwen3.6-plus", WireDialect::AnthropicMessages),
];

/// Canonicalizes an OpenCode Go model selector and resolves its documented wire dialect (`None` if unsupported).
pub fn resolve_opencode_go_model(model: &str) -> Option<(&'static str, WireDialect)> {
    let norm = model.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let stripped = norm.strip_prefix("opencode-go/").unwrap_or(&norm);
    let aliased = match stripped {
        "grok-4-5" => "grok-4.5",
        "glm-5-2" => "glm-5.2",
        "glm-5-1" => "glm-5.1",
        "kimi-k2-7-code" => "kimi-k2.7-code",
        "kimi-k2-6" => "kimi-k2.6",
        "deepseek-v4pro" => "deepseek-v4-pro",
        "deepseek-v4flash" => "deepseek-v4-flash",
        "deepseek-flash" => "deepseek-v4.1-flash",
        "mimo-v2-5" => "mimo-v2.5",
        "mimo-v2-5-pro" => "mimo-v2.5-pro",
        other => other,
    };
    OPENCODE_GO_TRANSPORTS.iter().copied().find(|(id, _)| *id == aliased)
}

/// Canonicalizes an OpenCode Go model ID (`Codewhale opencode_go_model_id` parity).
#[inline]
pub fn opencode_go_model_id(model: &str) -> Option<&'static str> {
    resolve_opencode_go_model(model).map(|(id, _)| id)
}

fn is_official_deepseek_endpoint(endpoint: &str) -> bool {
    let trimmed = endpoint.trim().trim_end_matches('/').to_ascii_lowercase();
    matches!(
        trimmed.as_str(),
        ""
            | "https://api.deepseek.com"
            | "https://api.deepseek.com/v1"
            | "https://api.deepseek.com/beta"
            | "https://api.deepseeki.com"
            | "https://api.deepseeki.com/v1"
            | "https://api.deepseeki.com/beta"
    )
}

/// Resolves per-model wire dialect and normalized wire model ID for `model_aware` & gateway providers.
pub fn resolve_model_aware_wire_and_model(
    provider_id: &str,
    raw_model: &str,
    endpoint: &str,
    default_wire: WireDialect,
) -> (WireDialect, String) {
    let trimmed = raw_model.trim();
    let canon_prov = lookup_provider_descriptor(provider_id)
        .map(|d| d.id)
        .unwrap_or(provider_id);

    match canon_prov {
        "concentrate" => {
            let stripped = trimmed.strip_prefix("concentrate/").unwrap_or(trimmed);
            (WireDialect::OpenAiResponses, stripped.to_string())
        }
        "codewhale" => {
            if trimmed.to_ascii_lowercase().starts_with("anthropic/") {
                (WireDialect::AnthropicMessages, trimmed.to_string())
            } else {
                (WireDialect::ChatCompletions, trimmed.to_string())
            }
        }
        "opencode-go" => {
            if let Some((canonical_id, dialect)) = resolve_opencode_go_model(trimmed) {
                (dialect, canonical_id.to_string())
            } else {
                (default_wire, trimmed.to_string())
            }
        }
        "opencode" => {
            let unprefixed = trimmed
                .strip_prefix("opencode/")
                .or_else(|| trimmed.strip_prefix("opencode-zen/"))
                .unwrap_or(trimmed);
            let low = unprefixed.to_ascii_lowercase();
            if low == "gpt-5.6" {
                return (WireDialect::OpenAiResponses, "gpt-5.6-sol".to_string());
            }
            if low.contains("muse-spark")
                || low.starts_with("gpt-5")
                || low.starts_with("gpt-6")
                || matches!(low.as_str(), "grok-4.7" | "grok-4.6" | "grok-4.5" | "grok-build-0.1")
            {
                (WireDialect::OpenAiResponses, unprefixed.to_string())
            } else if low.starts_with("claude-")
                || matches!(
                    low.as_str(),
                    "qwen3.8-flash" | "qwen3.7-max" | "qwen3.7-plus" | "qwen3.6-plus" | "qwen3.5-plus"
                )
            {
                (WireDialect::AnthropicMessages, unprefixed.to_string())
            } else {
                (WireDialect::ChatCompletions, unprefixed.to_string())
            }
        }
        "deepseek" => {
            let low = trimmed.to_ascii_lowercase();
            if is_official_deepseek_endpoint(endpoint) && matches!(low.as_str(), "deepseek-flash" | "deepseek-v4-flash") {
                (WireDialect::OpenAiResponses, trimmed.to_string())
            } else {
                (WireDialect::ChatCompletions, trimmed.to_string())
            }
        }
        "gemini" => {
            let ep_low = endpoint.trim().to_ascii_lowercase();
            let path_part = ep_low.split(['?', '#']).next().unwrap_or(&ep_low).trim_end_matches('/');
            if path_part.ends_with("/openai") || path_part.contains("/openai/") || path_part.ends_with("/chat/completions") {
                (WireDialect::ChatCompletions, trimmed.to_string())
            } else {
                (default_wire, trimmed.to_string())
            }
        }
        _ => (default_wire, trimmed.to_string()),
    }
}

/// Fuzzy-matches a mistyped provider selector against active sovereign providers (`Levenshtein <= 2`).
pub fn suggest_provider_slug(raw: &str) -> Option<&'static str> {
    let norm = normalize_provider_token(raw);
    if norm.len() < 2 {
        return None;
    }
    let mut best: Option<(usize, &'static str)> = None;
    for desc in PROVIDER_DESCRIPTORS {
        if desc.retired {
            continue;
        }
        let d_id = normalize_provider_token(desc.id);
        let mut min_d = levenshtein(&norm, &d_id);
        if d_id.starts_with(&norm) || norm.starts_with(&d_id) {
            min_d = min_d.min(1);
        }
        for alias in desc.aliases {
            let a_norm = normalize_provider_token(alias);
            min_d = min_d.min(levenshtein(&norm, &a_norm));
        }
        let max_allowed = if norm.len() <= 4 { 1 } else { 2 };
        if min_d <= max_allowed && best.is_none_or(|(prev, _)| min_d < prev) {
            best = Some((min_d, desc.id));
        }
    }
    best.map(|(_, id)| id)
}
