use once_cell::sync::Lazy;
use std::collections::HashMap;

static MODEL_ALIASES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("big-pickle", "glm-4.7");
    m.insert("big pickle", "glm-4.7");
    m.insert("bigpickle", "glm-4.7");
    m.insert("k2p5", "kimi-k2-thinking");
    m.insert("k2-p5", "kimi-k2-thinking");
    m.insert("kimi-k2.5-thinking", "kimi-k2-thinking");
    m.insert("kimi-for-coding", "kimi-k2.5");

    // Gemini 3.1 Flash (Gemini CLI)
    m.insert("gemini-3-flash-preview", "gemini-3-flash-preview");

    // Gemini 3.5 Flash (Antigravity)
    m.insert("gemini-3-flash-a", "gemini-3.5-flash");
    m.insert("gemini-3-flash-high", "gemini-3.5-flash");
    m.insert("gemini-3-flash", "gemini-3.5-flash");
    m.insert("gemini-3-flash-c", "gemini-3.5-flash");
    m.insert("model_placeholder_m47", "gemini-3.5-flash");
    m.insert("model_placeholder_m132", "gemini-3.5-flash");
    m.insert("model_placeholder_m18", "gemini-3.5-flash");
    m.insert("gemini-3.5-flash", "gemini-3.5-flash");

    m.insert("gemini-3.1-pro-high", "gemini-3.1-pro");
    m.insert("gemini-3-pro-high", "gemini-3-pro");
    m.insert("model_placeholder_m36", "gemini-3.1-pro");

    m.insert("gemini-3.1-pro-low", "gemini-3.1-pro");
    m.insert("gemini-3-pro-low", "gemini-3-pro");
    m.insert("model_placeholder_m37", "gemini-3.1-pro");
    m.insert("gemini-3.1-pro", "gemini-3.1-pro");

    m.insert("claude-sonnet-4-6-thinking", "claude-sonnet-4-6");
    m.insert("claude-sonnet-4.6-thinking", "claude-sonnet-4-6");
    m.insert("model_placeholder_m35", "claude-sonnet-4-6");
    m.insert("claude-sonnet-4-6", "claude-sonnet-4-6");
    m.insert("claude-sonnet-4.6", "claude-sonnet-4-6");

    m.insert("claude-sonnet-4-8", "claude-sonnet-4-8");
    m.insert("claude-sonnet-4.8", "claude-sonnet-4-8");

    m.insert("claude-opus-4-6-thinking", "claude-opus-4-6");
    m.insert("claude-opus-4.6-thinking", "claude-opus-4-6");
    m.insert("model_placeholder_m26", "claude-opus-4-6");
    m.insert("claude-opus-4-6", "claude-opus-4-6");
    m.insert("claude-opus-4.6", "claude-opus-4-6");

    m.insert("claude-opus-4-7", "claude-opus-4-7");
    m.insert("claude-opus-4.7", "claude-opus-4-7");

    m.insert("claude-opus-4-8", "claude-opus-4-8");
    m.insert("claude-opus-4.8", "claude-opus-4-8");

    m.insert("gpt-oss-120b", "gpt-oss-120b");
    m.insert("model_openai_gpt_oss_120b_medium", "gpt-oss-120b");

    m.insert("claude-haiku-4-6", "claude-haiku-4-6");
    m.insert("claude-haiku-4.6", "claude-haiku-4-6");

    // Claude Haiku 4.5 variants
    m.insert("claude-haiku-4-5", "claude-haiku-4-5");
    m.insert("claude-haiku-4.5", "claude-haiku-4-5");
    m.insert("claude-haiku-4-5-20251001", "claude-haiku-4-5");

    // DeepSeek models
    m.insert("deepseek-v4-pro", "deepseek-v4-pro");
    m.insert("deepseek-v4-flash", "deepseek-v4-flash");
    m.insert("deepseek-v3", "deepseek-v3");
    m.insert("deepseek-v3-0324", "deepseek-v3-0324");
    m.insert("deepseek-chat", "deepseek-chat");
    m.insert("deepseek-coder", "deepseek-coder");

    // Claude Sonnet 4.5 variants
    m.insert("claude-sonnet-4-5", "claude-sonnet-4-5");
    m.insert("claude-sonnet-4.5", "claude-sonnet-4-5");
    m.insert("claude-sonnet-4-5-20250929", "claude-sonnet-4-5");
    m.insert("claude-sonnet-4-5-thinking", "claude-sonnet-4-5");
    m.insert("claude-sonnet-4.5-thinking", "claude-sonnet-4-5");

    // GPT models
    m.insert("gpt-5.5", "gpt-5.5");
    m.insert("gpt-5-5", "gpt-5.5");
    m.insert("gpt-5.4", "gpt-5.4");
    m.insert("gpt-5.4-mini", "gpt-5.4-mini");
    m.insert("gpt-5-4-mini", "gpt-5.4-mini");
    m.insert("gpt-5.3-codex", "gpt-5.3-codex");
    m.insert("gpt-5-3-codex", "gpt-5.3-codex");
    m.insert("gpt-5.2", "gpt-5.2");
    m.insert("gpt-5-2", "gpt-5.2");
    m.insert("gpt-5.1", "gpt-5.1");
    m.insert("gpt-5-1", "gpt-5.1");
    m.insert("gpt-5-codex", "gpt-5-codex");
    m.insert("gpt-5.1-codex", "gpt-5.1-codex");
    m.insert("gpt-5.1-codex-max", "gpt-5.1-codex-max");
    m.insert("gpt-5-nano", "gpt-5-nano");
    m.insert("gpt-5", "gpt-5");
    m.insert("gpt-4o", "gpt-4o");
    m.insert("gpt-4o-mini", "gpt-4o-mini");
    m.insert("gpt-4-turbo", "gpt-4-turbo");
    m.insert("gpt-4", "gpt-4");

    // Kimi models
    m.insert("kimi-for-coding", "kimi-k2.5");
    m.insert("kimi-k2.5", "kimi-k2.5");
    m.insert("kimi-k2.6", "kimi-k2.6");
    m.insert("kimi-k2-thinking", "kimi-k2-thinking");

    // MiniMax models
    m.insert("minimax-m2.7-highspeed", "minimax-m2.7-highspeed");
    m.insert("minimax-m2.7", "minimax-m2.7");
    m.insert("minimax-m2.5-highspeed", "minimax-m2.5-highspeed");
    m.insert("minimax-m2.5", "minimax-m2.5");
    m.insert("minimax-m2.5-free", "minimax-m2.5-free");

    // Doubao models
    m.insert("doubao-seed-code", "doubao-seed-code");

    // GLM models
    m.insert("z-ai/glm-5.1", "z-ai/glm-5.1");
    m.insert("glm-5.1", "z-ai/glm-5.1");

    // Gemini preview variants
    m.insert("gemini-3.1-pro-preview", "gemini-3.1-pro");
    m.insert("gemini-3-flash-preview", "gemini-3-flash-preview");

    // Map pretty display names back to canonical pricing models (full-width brackets)
    m.insert("gemini-3.5-flash（high）", "gemini-3.5-flash");
    m.insert("gemini-3.5-flash（medium）", "gemini-3.5-flash");
    m.insert("gemini-3.1-pro（high）", "gemini-3.1-pro");
    m.insert("gemini-3.1-pro（low）", "gemini-3.1-pro");
    m.insert("claude-sonnet-4.6（thinking）", "claude-sonnet-4-6");
    m.insert("claude-opus-4.6（thinking）", "claude-opus-4-6");
    m.insert("claude-opus-4.7", "claude-opus-4-7");
    m.insert("gpt-oss-120b（medium）", "gpt-oss-120b");

    // Map pretty display names back to canonical pricing models (half-width brackets)
    m.insert("gemini-3.5-flash(high)", "gemini-3.5-flash");
    m.insert("gemini-3.5-flash(medium)", "gemini-3.5-flash");
    m.insert("gemini-3.1-pro(high)", "gemini-3.1-pro");
    m.insert("gemini-3.1-pro(low)", "gemini-3.1-pro");
    m.insert("claude-sonnet-4.6(thinking)", "claude-sonnet-4-6");
    m.insert("claude-opus-4.6(thinking)", "claude-opus-4-6");
    m.insert("gpt-oss-120b(medium)", "gpt-oss-120b");

    // Synthetic model variants (only where resolver needs help)
    m.insert("kimi-k2.5-nvfp4", "kimi-k2.5"); // Quantization variant → base model pricing
    m.insert("kimi-k2-instruct-0905", "kimi-k2.5"); // Specific version → base (avoids reseller)
    m
});

static PRETTY_NAMES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("gemini-3-flash-preview", "Gemini-3.1-Flash");
    m.insert("gemini-3-flash-a", "Gemini-3.5-Flash");
    m.insert("gemini-3-flash", "Gemini-3.5-Flash");
    m.insert("gemini-3-flash-c", "Gemini-3.5-Flash");
    m.insert("model_placeholder_m47", "Gemini-3.5-Flash");
    m.insert("model_placeholder_m132", "Gemini-3.5-Flash");
    m.insert("model_placeholder_m18", "Gemini-3.5-Flash");

    m.insert("gemini-3.1-pro-high", "Gemini-3.1-Pro（High）");
    m.insert("gemini-3-pro-high", "Gemini-3.1-Pro（High）");
    m.insert("model_placeholder_m36", "Gemini-3.1-Pro（High）");

    m.insert("gemini-3.1-pro-low", "Gemini-3.1-Pro（Low）");
    m.insert("gemini-3-pro-low", "Gemini-3.1-Pro（Low）");
    m.insert("model_placeholder_m37", "Gemini-3.1-Pro（Low）");
    m.insert("gemini-3.1-pro", "Gemini-3.1-Pro");

    m.insert("claude-sonnet-4-6-thinking", "Claude-Sonnet-4.6");
    m.insert("claude-sonnet-4.6-thinking", "Claude-Sonnet-4.6");
    m.insert("model_placeholder_m35", "Claude-Sonnet-4.6");
    m.insert("claude-sonnet-4-6", "Claude-Sonnet-4.6");
    m.insert("claude-sonnet-4.6", "Claude-Sonnet-4.6");

    m.insert("claude-sonnet-4-8", "Claude-Sonnet-4.8");
    m.insert("claude-sonnet-4.8", "Claude-Sonnet-4.8");

    m.insert("claude-opus-4-6-thinking", "Claude-Opus-4.6");
    m.insert("claude-opus-4.6-thinking", "Claude-Opus-4.6");
    m.insert("model_placeholder_m26", "Claude-Opus-4.6");
    m.insert("claude-opus-4-6", "Claude-Opus-4.6");
    m.insert("claude-opus-4.6", "Claude-Opus-4.6");

    m.insert("claude-opus-4-7", "Claude-Opus-4.7");
    m.insert("claude-opus-4.7", "Claude-Opus-4.7");

    m.insert("claude-opus-4-8", "Claude-Opus-4.8");
    m.insert("claude-opus-4.8", "Claude-Opus-4.8");

    m.insert("gpt-oss-120b", "GPT-OSS-120B");
    m.insert("model_openai_gpt_oss_120b_medium", "GPT-OSS-120B");

    // DeepSeek models
    m.insert("deepseek-v4-pro", "DeepSeek-V4-Pro");
    m.insert("deepseek-v4-flash", "DeepSeek-V4-Flash");
    m.insert("deepseek-v3", "DeepSeek-V3");
    m.insert("deepseek-v3-0324", "DeepSeek-V3-0324");
    m.insert("deepseek-chat", "DeepSeek-Chat");
    m.insert("deepseek-coder", "DeepSeek-Coder");

    // Claude Haiku 4.5
    m.insert("claude-haiku-4-5", "Claude-Haiku-4.5");
    m.insert("claude-haiku-4.5", "Claude-Haiku-4.5");
    m.insert("claude-haiku-4-5-20251001", "Claude-Haiku-4.5");

    // Claude Sonnet 4.5
    m.insert("claude-sonnet-4-5", "Claude-Sonnet-4.5");
    m.insert("claude-sonnet-4.5", "Claude-Sonnet-4.5");
    m.insert("claude-sonnet-4-5-20250929", "Claude-Sonnet-4.5");
    m.insert("claude-sonnet-4-5-thinking", "Claude-Sonnet-4.5");
    m.insert("claude-sonnet-4.5-thinking", "Claude-Sonnet-4.5");

    // GPT models
    m.insert("gpt-5.5", "GPT-5.5");
    m.insert("gpt-5-5", "GPT-5.5");
    m.insert("gpt-5.4", "GPT-5.4");
    m.insert("gpt-5.4-mini", "GPT-5.4-Mini");
    m.insert("gpt-5-4-mini", "GPT-5.4-Mini");
    m.insert("gpt-5.3-codex", "GPT-5.3-Codex");
    m.insert("gpt-5-3-codex", "GPT-5.3-Codex");
    m.insert("gpt-5.2", "GPT-5.2");
    m.insert("gpt-5-2", "GPT-5.2");
    m.insert("gpt-5.1", "GPT-5.1");
    m.insert("gpt-5-1", "GPT-5.1");
    m.insert("gpt-5-codex", "GPT-5-Codex");
    m.insert("gpt-5.1-codex", "GPT-5.1-Codex");
    m.insert("gpt-5.1-codex-max", "GPT-5.1-Codex-Max");
    m.insert("gpt-5-nano", "GPT-5-Nano");
    m.insert("gpt-5", "GPT-5");
    m.insert("gpt-4o", "GPT-4o");
    m.insert("gpt-4o-mini", "GPT-4o-Mini");
    m.insert("gpt-4-turbo", "GPT-4-Turbo");
    m.insert("gpt-4", "GPT-4");

    // Kimi models
    m.insert("kimi-for-coding", "Kimi-K2.5");
    m.insert("kimi-k2.5", "Kimi-K2.5");
    m.insert("kimi-k2.6", "Kimi-K2.6");
    m.insert("kimi-k2-thinking", "Kimi-K2-Thinking");
    m.insert("k2p5", "Kimi-K2-Thinking");
    m.insert("k2-p5", "Kimi-K2-Thinking");
    m.insert("kimi-latest", "Kimi-Latest");
    m.insert("kimi-k2.5-nvfp4", "Kimi-K2.5");
    m.insert("kimi-k2-instruct-0905", "Kimi-K2.5");

    // MiniMax models
    m.insert("minimax-m2.7-highspeed", "MiniMax-M2.7-Highspeed");
    m.insert("minimax-m2.7", "MiniMax-M2.7");
    m.insert("minimax-m2.5-highspeed", "MiniMax-M2.5-Highspeed");
    m.insert("minimax-m2.5", "MiniMax-M2.5");
    m.insert("minimax-m2.5-free", "MiniMax-M2.5-Free");

    // Doubao models
    m.insert("doubao-seed-code", "Doubao-Seed-Code");

    // GLM models
    m.insert("z-ai/glm-5.1", "GLM-5.1");
    m.insert("glm-5.1", "GLM-5.1");

    // Gemini preview variants
    m.insert("gemini-3.1-pro-preview", "Gemini-3.1-Pro");
    m.insert("gemini-3-flash-preview", "Gemini-3.1-Flash");
    m.insert("gemini-3.5-flash", "Gemini-3.5-Flash");

    m
});

pub fn resolve_alias(model_id: &str) -> Option<&'static str> {
    MODEL_ALIASES.get(model_id.to_lowercase().as_str()).copied()
}

pub fn resolve_pretty_name(model_id: &str) -> Option<&'static str> {
    PRETTY_NAMES.get(model_id.to_lowercase().as_str()).copied()
}

#[cfg(test)]
mod tests {
    use super::{resolve_alias, resolve_pretty_name};

    #[test]
    fn resolves_antigravity_placeholders() {
        assert_eq!(
            resolve_pretty_name("MODEL_PLACEHOLDER_M26"),
            Some("Claude-Opus-4.6")
        );
        assert_eq!(
            resolve_pretty_name("model_placeholder_m37"),
            Some("Gemini-3.1-Pro（Low）")
        );
        assert_eq!(
            resolve_pretty_name("MODEL_PLACEHOLDER_M132"),
            Some("Gemini-3.5-Flash")
        );
        assert_eq!(
            resolve_pretty_name("MODEL_OPENAI_GPT_OSS_120B_MEDIUM"),
            Some("GPT-OSS-120B")
        );
        assert_eq!(
            resolve_pretty_name("gemini-3-flash-c"),
            Some("Gemini-3.5-Flash")
        );
        assert_eq!(
            resolve_pretty_name("claude-opus-4.6-thinking"),
            Some("Claude-Opus-4.6")
        );
        assert_eq!(
            resolve_pretty_name("claude-opus-4-7"),
            Some("Claude-Opus-4.7")
        );
        assert_eq!(
            resolve_pretty_name("claude-opus-4.7"),
            Some("Claude-Opus-4.7")
        );
        assert_eq!(
            resolve_pretty_name("deepseek-v4-pro"),
            Some("DeepSeek-V4-Pro")
        );
        assert_eq!(resolve_pretty_name("gpt-5.5"), Some("GPT-5.5"));
        assert_eq!(
            resolve_pretty_name("claude-sonnet-4-5-20250929"),
            Some("Claude-Sonnet-4.5")
        );
        assert_eq!(resolve_pretty_name("gpt-5.4-mini"), Some("GPT-5.4-Mini"));
        assert_eq!(resolve_pretty_name("gpt-5.3-codex"), Some("GPT-5.3-Codex"));
        assert_eq!(
            resolve_pretty_name("deepseek-v4-flash"),
            Some("DeepSeek-V4-Flash")
        );
        assert_eq!(resolve_pretty_name("kimi-for-coding"), Some("Kimi-K2.5"));
        assert_eq!(
            resolve_pretty_name("MiniMax-M2.7-highspeed"),
            Some("MiniMax-M2.7-Highspeed")
        );
        assert_eq!(
            resolve_pretty_name("minimax-m2.5-free"),
            Some("MiniMax-M2.5-Free")
        );
        assert_eq!(
            resolve_pretty_name("doubao-seed-code"),
            Some("Doubao-Seed-Code")
        );
        assert_eq!(
            resolve_pretty_name("gemini-3.1-pro-preview"),
            Some("Gemini-3.1-Pro")
        );
    }

    #[test]
    fn resolves_aliases_to_canonical() {
        assert_eq!(resolve_alias("gemini-3-flash"), Some("gemini-3.5-flash"));
        assert_eq!(resolve_alias("Gemini-3.5-Flash"), Some("gemini-3.5-flash"));
        assert_eq!(
            resolve_alias("Claude-Opus-4.6（Thinking）"),
            Some("claude-opus-4-6")
        );
        assert_eq!(resolve_alias("Claude-Opus-4.7"), Some("claude-opus-4-7"));
        assert_eq!(resolve_alias("DeepSeek-v4-Pro"), Some("deepseek-v4-pro"));
        assert_eq!(resolve_alias("GPT-5.5"), Some("gpt-5.5"));
    }
}
