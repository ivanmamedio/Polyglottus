//! Provedor Ollama

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{TranslationProvider, ProviderError, Capabilities, TranslateRequest, TranslateResponse, ModelInfo, Language, TokenUsage, UnitTranslation};
use pgl_domain::{Unit, UnitId, MarkedText, LangPair, Glossary, StyleGuide};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug, warn};

/// Configuração do provedor Ollama
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,          // Ex: "http://localhost:11434"
    pub timeout_secs: u64,
    pub keep_alive: String,        // "5m", "-1" (permanente), etc.
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            timeout_secs: 120,
            keep_alive: "5m".into(),
        }
    }
}

/// Provedor Ollama
pub struct OllamaProvider {
    config: OllamaConfig,
    client: Client,
    default_model: String,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| ProviderError::Network(e.into()))?;

        Ok(Self {
            config,
            client,
            default_model: "llama3".into(),
        })
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.default_model = model;
        self
    }

    fn build_prompt(&self, req: &TranslateRequest) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "You are a professional translator. Translate from {} to {}.",
            req.lang_pair.source, req.lang_pair.target
        ));

        parts.push("Rules:".into());
        parts.push("1. Preserve all markup placeholders exactly (e.g., ⟦1⟧...⟦/1⟧)".into());
        parts.push("2. Maintain the same formatting, numbers, dates, and proper nouns.".into());
        parts.push("3. Do not add explanations, only the translation.".into());

        if let Some(glossary) = &req.glossary {
            parts.push("\nGlossary (MUST follow):".into());
            for entry in glossary.terms_sorted() {
                if entry.do_not_translate {
                    parts.push(format!("- '{}' → DO NOT TRANSLATE", entry.source_term));
                } else {
                    parts.push(format!("- '{}' → '{}'", entry.source_term, entry.target_term));
                }
            }
        }

        if let Some(style) = &req.style_guide {
            parts.push("\nStyle Guide:".into());
            parts.push(style.to_system_prompt());
        }

        if let Some(ctx) = &req.context {
            parts.push(format!("\nContext: {}", ctx));
        }

        parts.push("\nSegments to translate:".into());
        for (i, unit) in req.units.iter().enumerate() {
            parts.push(format!("\n--- Segment {} ---", i + 1));
            parts.push(format!("Type: {:?}", unit.kind));
            parts.push(format!("Text: {}", unit.text.raw));
        }

        parts.push("\n\nReturn ONLY a JSON object with 'translations' array of objects with 'translated_text' and optional 'confidence' fields.".into());

        parts.join("\n")
    }

    fn parse_response(&self, response: OllamaGenerateResponse, req: &TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let content = response.response;

        // Tenta parsear JSON
        if let Ok(parsed) = serde_json::from_str::<OllamaTranslationResponse>(&content) {
            return Ok(TranslateResponse {
                translations: parsed.translations.into_iter().enumerate().map(|(i, t)| {
                    let unit_id = req.units.get(i).map(|u| u.id).unwrap_or_else(UnitId::new);
                    UnitTranslation {
                        unit_id,
                        translated_text: MarkedText::new(t.translated_text),
                        confidence: t.confidence,
                        qa_warnings: Vec::new(),
                    }
                }).collect(),
                tokens_used: response.usage.map(|u| TokenUsage {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                }),
                latency_ms: 0,
                model_used: response.model,
            });
        }

        // Fallback: linhas simples
        let lines: Vec<&str> = content.lines()
            .filter(|l| !l.trim().is_empty())
            .collect();

        Ok(TranslateResponse {
            translations: req.units.iter().zip(lines).map(|(unit, text)| {
                UnitTranslation {
                    unit_id: unit.id,
                    translated_text: MarkedText::new(text),
                    confidence: Some(0.8),
                    qa_warnings: Vec::new(),
                }
            }).collect(),
            tokens_used: None,
            latency_ms: 0,
            model_used: response.model,
        })
    }
}

#[async_trait]
impl TranslationProvider for OllamaProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            max_context_tokens: 8192, // Ollama default
            supported_languages: vec![
                Language::new("en"), Language::new("pt"), Language::new("pt-BR"),
                Language::new("es"), Language::new("fr"), Language::new("de"),
                Language::new("it"), Language::new("ja"), Language::new("ko"),
                Language::new("zh"), Language::new("ru"), Language::new("ar"),
            ],
            is_local: true,
            supports_streaming: true,
            supports_glossary: true,
            default_model: Some(self.default_model.clone()),
        }
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let start = std::time::Instant::now();
        let model = req.model_id.as_deref().unwrap_or(&self.default_model);
        let prompt = self.build_prompt(&req);

        let request = OllamaGenerateRequest {
            model: model.to_string(),
            prompt,
            stream: false,
            keep_alive: Some(self.config.keep_alive.clone()),
            options: Some(OllamaOptions {
                temperature: req.temperature.unwrap_or(0.3),
                num_predict: req.max_tokens.map(|t| t as i32),
            }),
        };

        let url = format!("{}/api/generate", self.config.base_url.trim_end_matches('/'));
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.into()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimited(Duration::from_secs(60)));
            }
            return Err(ProviderError::Provider(format!("HTTP {}: {}", status, error_text)));
        }

        let ollama_resp: OllamaGenerateResponse = response.json().await
            .map_err(|e| ProviderError::Provider(format!("Invalid JSON: {}", e)))?;

        let mut result = self.parse_response(ollama_resp, &req)?;
        result.latency_ms = start.elapsed().as_millis() as u64;

        Ok(result)
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let url = format!("{}/api/tags", self.config.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await.map_err(|e| ProviderError::Network(e.into()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Provider("Ollama health check failed".into()));
        }
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let url = format!("{}/api/tags", self.config.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await.map_err(|e| ProviderError::Network(e.into()))?;
        let tags_resp: OllamaTagsResponse = resp.json().await
            .map_err(|e| ProviderError::Provider(format!("Invalid JSON: {}", e)))?;

        Ok(tags_resp.models.into_iter().map(|m| ModelInfo {
            id: m.name.clone(),
            name: m.name,
            languages: vec![Language::new("en")],
            max_context: 8192,
            is_local: true,
            size_gb: Some((m.size as f64 / 1_000_000_000.0) as f32),
            license: None,
        }).collect())
    }
}

// ============================================================================
// Tipos Ollama API
// ============================================================================

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(rename = "total_duration")]
    total_duration_ns: Option<u64>,
    #[serde(rename = "load_duration")]
    load_duration_ns: Option<u64>,
    #[serde(rename = "prompt_eval_count")]
    prompt_eval_count: Option<u32>,
    #[serde(rename = "eval_count")]
    eval_count: Option<u32>,
    usage: Option<OllamaUsage>,
}

#[derive(Deserialize)]
struct OllamaUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
    size: u64,
    digest: String,
    details: OllamaModelDetails,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    format: String,
    family: String,
    families: Vec<String>,
    parameter_size: String,
    quantization_level: String,
}

// Resposta estruturada esperada
#[derive(Deserialize)]
struct OllamaTranslationResponse {
    translations: Vec<OllamaTranslationItem>,
}

#[derive(Deserialize)]
struct OllamaTranslationItem {
    translated_text: String,
    confidence: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_creation() {
        let provider = OllamaProvider::new(OllamaConfig::default());
        assert!(provider.is_ok());
    }

    #[test]
    fn prompt_building() {
        let provider = OllamaProvider::new(OllamaConfig::default()).unwrap();
        let req = TranslateRequest {
            units: vec![],
            lang_pair: LangPair { source: Language::new("en"), target: Language::new("pt") },
            glossary: None,
            style_guide: None,
            context: None,
            model_id: None,
            temperature: None,
            max_tokens: None,
        };
        let prompt = provider.build_prompt(&req);
        assert!(prompt.contains("professional translator"));
        assert!(prompt.contains("⟦1⟧"));
    }
}