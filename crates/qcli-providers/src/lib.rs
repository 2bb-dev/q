//! LLM provider integrations: OpenAI-compatible, Anthropic, Codex (OpenAI CLI).
//!
//! (`#[tauri::command]`, `tauri::AppHandle`, `State<...>`) were stripped;
//! public functions take their dependencies as parameters instead.
//!
//! Dormant in v1 — wired up by the providers plan (and codex plan).

#![allow(dead_code)]

pub mod codex;
pub mod models;
pub mod upgrade;

/// The meta-prompt used by every provider's upgrade flow.
///
pub const META_PROMPT: &str = r#"You are a prompt engineering expert. Your task is to transform a raw, casual prompt into a well-structured, effective prompt. Apply these principles:

1. **Role**: Define who the AI should act as (if applicable)
2. **Context**: Add necessary background or framing
3. **Task**: Make the instruction clear, specific, and unambiguous
4. **Constraints**: Add boundaries, what to avoid, length/format limits
5. **Output format**: Specify the expected response structure (bullets, markdown, JSON, etc.)
6. **Chain-of-thought**: Add "think step by step" if the task requires reasoning

Rules:
- Return ONLY the improved prompt text, no explanations or commentary
- Preserve the user's original intent completely
- Don't over-complicate simple prompts — if it's already clear, just refine it slightly
- Keep the upgraded prompt concise but comprehensive
- Do not wrap the prompt in quotes or code blocks"#;

/// Minimal HTTP helper: attach a `Bearer` auth header if the key is non-empty.
///
pub fn bearer_request(builder: reqwest::RequestBuilder, api_key: &str) -> reqwest::RequestBuilder {
    let key = api_key.trim();
    if key.is_empty() {
        builder
    } else {
        builder.header("Authorization", format!("Bearer {}", key))
    }
}

/// Build the `/models` URL for an OpenAI-compatible endpoint.
pub fn openai_models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

/// Build the Ollama `/api/tags` URL. Handles `/v1` and `/api` suffixes so the
/// config form accepts either shape.
pub fn ollama_tags_url(base_url: &str) -> String {
    let normalized = base_url.trim_end_matches('/');
    if let Some(root) = normalized.strip_suffix("/v1") {
        format!("{}/api/tags", root)
    } else if normalized.ends_with("/api") {
        format!("{}/tags", normalized)
    } else {
        format!("{}/api/tags", normalized)
    }
}
