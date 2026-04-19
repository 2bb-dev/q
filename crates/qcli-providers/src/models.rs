//! Model discovery across OpenAI-compatible, Anthropic, and Ollama endpoints.
//!
//! Ported from `a3io/q:src-tauri/src/lib.rs::list_provider_models` and its
//! supporting model/response types.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::{bearer_request, ollama_tags_url, openai_models_url};

#[derive(Debug, Clone, Serialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub label: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelDescriptor>,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
    #[serde(default)]
    owned_by: String,
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
}

/// Query the configured provider for its list of models.
pub async fn list_provider_models(
    provider_preset: String,
    base_url: String,
    api_key: String,
) -> Result<ModelListResponse, String> {
    let client = reqwest::Client::new();
    let provider = provider_preset.trim();

    let models: Vec<ModelDescriptor> = match provider {
        "anthropic" => {
            let url = format!("{}/models", base_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .header("x-api-key", api_key.trim())
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, text));
            }

            let payload: AnthropicModelsResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            payload
                .data
                .into_iter()
                .map(|model| ModelDescriptor {
                    label: model.id.clone(),
                    note: if model.display_name.is_empty() {
                        "anthropic /v1/models".to_string()
                    } else {
                        model.display_name
                    },
                    id: model.id,
                })
                .collect()
        }
        "ollama" => {
            let url = ollama_tags_url(&base_url);
            let resp = bearer_request(client.get(&url), &api_key)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, text));
            }

            let payload: OllamaTagsResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            payload
                .models
                .into_iter()
                .filter_map(|model| {
                    let id = if model.name.is_empty() {
                        model.model
                    } else {
                        model.name
                    };
                    if id.is_empty() {
                        None
                    } else {
                        Some(ModelDescriptor {
                            label: id.clone(),
                            note: "ollama /api/tags".to_string(),
                            id,
                        })
                    }
                })
                .collect()
        }
        _ => {
            let url = openai_models_url(&base_url);
            let resp = bearer_request(client.get(&url), &api_key)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, text));
            }

            let payload: OpenAiModelsResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            payload
                .data
                .into_iter()
                .map(|model| ModelDescriptor {
                    label: model.id.clone(),
                    note: if model.owned_by.is_empty() {
                        "compatible /models".to_string()
                    } else {
                        model.owned_by
                    },
                    id: model.id,
                })
                .collect()
        }
    };

    if models.is_empty() {
        return Err("No models returned".to_string());
    }

    Ok(ModelListResponse { models })
}
