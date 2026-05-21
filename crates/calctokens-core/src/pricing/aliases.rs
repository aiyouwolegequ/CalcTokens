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

    m.insert("gemini-3-flash-a", "gemini-3-flash-preview");
    m.insert("gemini-3-flash-preview", "gemini-3-flash-preview");
    m.insert("gemini-3-flash", "gemini-3-flash-preview");
    m.insert("gemini-3-flash-c", "gemini-3-flash-preview");
    m.insert("model_placeholder_m47", "gemini-3-flash-preview");

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

    m.insert("claude-opus-4-6-thinking", "claude-opus-4-6");
    m.insert("claude-opus-4.6-thinking", "claude-opus-4-6");
    m.insert("model_placeholder_m26", "claude-opus-4-6");
    m.insert("claude-opus-4-6", "claude-opus-4-6");
    m.insert("claude-opus-4.6", "claude-opus-4-6");

    m.insert("claude-opus-4-7", "claude-opus-4-7");
    m.insert("claude-opus-4.7", "claude-opus-4-7");

    m.insert("gpt-oss-120b-medium", "gpt-oss-120b-medium");
    m.insert("model_openai_gpt_oss_120b_medium", "gpt-oss-120b-medium");

    m.insert("claude-haiku-4-6", "claude-haiku-4-6");
    m.insert("claude-haiku-4.6", "claude-haiku-4-6");

    // DeepSeek and GPT models
    m.insert("deepseek-v4-pro", "deepseek-v4-pro");
    m.insert("deepseek-v3", "deepseek-v3");
    m.insert("deepseek-v3-0324", "deepseek-v3-0324");
    m.insert("deepseek-chat", "deepseek-chat");
    m.insert("deepseek-coder", "deepseek-coder");

    m.insert("gpt-5.5", "gpt-5.5");
    m.insert("gpt-5-5", "gpt-5.5");
    m.insert("gpt-5.4", "gpt-5.4");
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

    // Map pretty display names back to canonical pricing models (full-width brackets)
    m.insert("gemini-3.5-flash（high）", "gemini-3-flash-preview");
    m.insert("gemini-3.5-flash（medium）", "gemini-3-flash-preview");
    m.insert("gemini-3.1-pro（high）", "gemini-3.1-pro");
    m.insert("gemini-3.1-pro（low）", "gemini-3.1-pro");
    m.insert("claude-sonnet-4.6（thinking）", "claude-sonnet-4-6");
    m.insert("claude-opus-4.6（thinking）", "claude-opus-4-6");
    m.insert("claude-opus-4.7", "claude-opus-4-7");
    m.insert("gpt-oss-120b（medium）", "gpt-oss-120b-medium");

    // Map pretty display names back to canonical pricing models (half-width brackets)
    m.insert("gemini-3.5-flash(high)", "gemini-3-flash-preview");
    m.insert("gemini-3.5-flash(medium)", "gemini-3-flash-preview");
    m.insert("gemini-3.1-pro(high)", "gemini-3.1-pro");
    m.insert("gemini-3.1-pro(low)", "gemini-3.1-pro");
    m.insert("claude-sonnet-4.6(thinking)", "claude-sonnet-4-6");
    m.insert("claude-opus-4.6(thinking)", "claude-opus-4-6");
    m.insert("gpt-oss-120b(medium)", "gpt-oss-120b-medium");

    // Synthetic model variants (only where resolver needs help)
    m.insert("kimi-k2.5-nvfp4", "kimi-k2.5"); // Quantization variant → base model pricing
    m.insert("kimi-k2-instruct-0905", "kimi-k2.5"); // Specific version → base (avoids reseller)
    m
});

static PRETTY_NAMES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("gemini-3-flash-a", "Gemini-3.5-Flash（High）");
    m.insert("gemini-3-flash-preview", "Gemini-3.5-Flash（Medium）");
    m.insert("gemini-3-flash", "Gemini-3.5-Flash（Medium）");
    m.insert("gemini-3-flash-c", "Gemini-3.5-Flash（Medium）");
    m.insert("model_placeholder_m47", "Gemini-3.5-Flash（Medium）");

    m.insert("gemini-3.1-pro-high", "Gemini-3.1-Pro（High）");
    m.insert("gemini-3-pro-high", "Gemini-3.1-Pro（High）");
    m.insert("model_placeholder_m36", "Gemini-3.1-Pro（High）");

    m.insert("gemini-3.1-pro-low", "Gemini-3.1-Pro（Low）");
    m.insert("gemini-3-pro-low", "Gemini-3.1-Pro（Low）");
    m.insert("model_placeholder_m37", "Gemini-3.1-Pro（Low）");
    m.insert("gemini-3.1-pro", "Gemini-3.1-Pro（Low）");

    m.insert("claude-sonnet-4-6-thinking", "Claude-Sonnet-4.6（Thinking）");
    m.insert("claude-sonnet-4.6-thinking", "Claude-Sonnet-4.6（Thinking）");
    m.insert("model_placeholder_m35", "Claude-Sonnet-4.6（Thinking）");
    m.insert("claude-sonnet-4-6", "Claude-Sonnet-4.6（Thinking）");
    m.insert("claude-sonnet-4.6", "Claude-Sonnet-4.6（Thinking）");

    m.insert("claude-opus-4-6-thinking", "Claude-Opus-4.6（Thinking）");
    m.insert("claude-opus-4.6-thinking", "Claude-Opus-4.6（Thinking）");
    m.insert("model_placeholder_m26", "Claude-Opus-4.6（Thinking）");
    m.insert("claude-opus-4-6", "Claude-Opus-4.6（Thinking）");
    m.insert("claude-opus-4.6", "Claude-Opus-4.6（Thinking）");

    m.insert("claude-opus-4-7", "Claude-Opus-4.7");
    m.insert("claude-opus-4.7", "Claude-Opus-4.7");

    m.insert("gpt-oss-120b-medium", "GPT-OSS-120B（Medium）");
    m.insert("model_openai_gpt_oss_120b_medium", "GPT-OSS-120B（Medium）");

    // DeepSeek and GPT models pretty display names
    m.insert("deepseek-v4-pro", "DeepSeek-v4-Pro");
    m.insert("deepseek-v3", "DeepSeek-V3");
    m.insert("deepseek-v3-0324", "DeepSeek-V3-0324");
    m.insert("deepseek-chat", "DeepSeek-Chat");
    m.insert("deepseek-coder", "DeepSeek-Coder");

    m.insert("gpt-5.5", "GPT-5.5");
    m.insert("gpt-5-5", "GPT-5.5");
    m.insert("gpt-5.4", "GPT-5.4");
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
            Some("Claude-Opus-4.6（Thinking）")
        );
        assert_eq!(
            resolve_pretty_name("model_placeholder_m37"),
            Some("Gemini-3.1-Pro（Low）")
        );
        assert_eq!(
            resolve_pretty_name("MODEL_OPENAI_GPT_OSS_120B_MEDIUM"),
            Some("GPT-OSS-120B（Medium）")
        );
        assert_eq!(
            resolve_pretty_name("gemini-3-flash-c"),
            Some("Gemini-3.5-Flash（Medium）")
        );
        assert_eq!(
            resolve_pretty_name("claude-opus-4.6-thinking"),
            Some("Claude-Opus-4.6（Thinking）")
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
            Some("DeepSeek-v4-Pro")
        );
        assert_eq!(
            resolve_pretty_name("gpt-5.5"),
            Some("GPT-5.5")
        );
    }

    #[test]
    fn resolves_aliases_to_canonical() {
        assert_eq!(
            resolve_alias("gemini-3-flash"),
            Some("gemini-3-flash-preview")
        );
        assert_eq!(
            resolve_alias("Claude-Opus-4.6（Thinking）"),
            Some("claude-opus-4-6")
        );
        assert_eq!(
            resolve_alias("Claude-Opus-4.7"),
            Some("claude-opus-4-7")
        );
        assert_eq!(
            resolve_alias("DeepSeek-v4-Pro"),
            Some("deepseek-v4-pro")
        );
        assert_eq!(
            resolve_alias("GPT-5.5"),
            Some("gpt-5.5")
        );
    }
}
