//! Prompt upgrade flows for OpenAI-compatible and Anthropic APIs.
//!
//! `run_anthropic_upgrade`, `upgrade_prompt`. The `#[tauri::command]` macro
//! was stripped; the public `upgrade_prompt` now takes a plain struct.

#![allow(dead_code)]

use serde::Deserialize;

use crate::{bearer_request, META_PROMPT};

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct AnthropicTextBlock {
    #[serde(default)]
    text: String,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicTextBlock>,
}

/// Call an OpenAI-compatible `/chat/completions` endpoint with the meta-prompt.
///
/// Returns the assistant's response text, or a human-readable error string
/// (matching the original Tauri-command contract for easy surface-up).
pub async fn run_openai_compatible_upgrade(
    base_url: String,
    api_key: String,
    model: String,
    raw_prompt: String,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": META_PROMPT },
            { "role": "user", "content": raw_prompt }
        ],
        "temperature": 0.7,
        "max_tokens": 2048
    });

    let client = reqwest::Client::new();
    let resp = bearer_request(client.post(&url), &api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let chat: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    chat.choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| "No response content".to_string())
}

/// Call Anthropic's `/messages` endpoint with the meta-prompt as system.
pub async fn run_anthropic_upgrade(
    base_url: String,
    api_key: String,
    model: String,
    raw_prompt: String,
) -> Result<String, String> {
    let url = format!("{}/messages", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "system": META_PROMPT,
        "max_tokens": 2048,
        "messages": [
            { "role": "user", "content": raw_prompt }
        ]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("x-api-key", api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let message: AnthropicMessageResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    message
        .content
        .into_iter()
        .find(|block| block.kind == "text" && !block.text.is_empty())
        .map(|block| block.text)
        .ok_or_else(|| "No response content".to_string())
}

/// Top-level dispatcher that matches the provider preset string and runs the
/// right upgrade flow. Codex is handled by `crate::codex::run_codex_upgrade`.
pub async fn upgrade_prompt(
    provider_preset: String,
    base_url: String,
    api_key: String,
    model: String,
    raw_prompt: String,
) -> Result<String, String> {
    match provider_preset.trim() {
        "openai_codex" => crate::codex::run_codex_upgrade(model, raw_prompt).await,
        "anthropic" => run_anthropic_upgrade(base_url, api_key, model, raw_prompt).await,
        _ => run_openai_compatible_upgrade(base_url, api_key, model, raw_prompt).await,
    }
}
