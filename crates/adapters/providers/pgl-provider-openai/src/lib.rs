//! Provedor OpenAI-compatível

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{TranslationProvider, ProviderError, Capabilities, TranslateRequest, TranslateResponse, ModelInfo, Language, TokenUsage, UnitTranslation, QaWarning};
use pgl_domain::{Unit, UnitId, MarkedText, LangPair, Glossary, StyleGuide};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug, warn};

/// Configuração do provedor OpenAI-compatível
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompatConfig {
    pub base_url: String,          // Ex: "http://localhost:8080/v1"
    pub api_key: Option<String>,   // Chave da API (opcional para servidores locais)
    pub organization: Option<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for OpenAICompatConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: None,
            organization: None,
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Provedor OpenAI-compatível
pub struct OpenAICompatProvider {
    config: OpenAICompatConfig,
    client: Client,
    default_model: String,
}

impl OpenAICompatProvider {
    pub fn new(config: OpenAICompatConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| ProviderError::Network(e.into()))?;

        Ok(Self {
            config,
            client,
            default_model: "gpt-3.5-turbo".into(),
        })
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.default_model = model;
        self
    }

    /// Cria provedor para servidor local (llama-server, LM Studio, etc.)
    pub fn local_server(base_url: String, model: Option<String>) -> Result<Self, ProviderError> {
        let config = OpenAICompatConfig {
            base_url,
            api_key: None,
            ..Default::default()
        };
        let mut provider = Self::new(config)?;
        if let Some(m) = model {
            provider.default_model = m;
        }
        Ok(provider)
    }

    /// Cria provedor para OpenAI API
    pub fn openai(api_key: String, model: Option<String>) -> Result<Self, ProviderError> {
        let config = OpenAICompatConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key: Some(api_key),
            ..Default::default()
        };
        let mut provider = Self::new(config)?;
        if let Some(m) = model {
            provider.default_model = m;
        }
        Ok(provider)
    }

    fn build_request(&self, req: &TranslateRequest) -> OpenAIRequest {
        let model = req.model_id.as_deref().unwrap_or(&self.default_model);

        // Constrói mensagens
        let mut messages = Vec::new();

        // System prompt com glossário e guia de estilo
        let mut system_prompt = self.build_system_prompt(req);
        if let Some(ctx) = &req.context {
            system_prompt.push_str(&format!("\n\nContexto adicional: {}", ctx));
        }
        messages.push(ChatMessage {
            role: "system".into(),
            content: system_prompt,
        });

        // User message com textos a traduzir
        let user_content = self.build_user_prompt(req);
        messages.push(ChatMessage {
            role: "user".into(),
            content: user_content,
        });

        OpenAIRequest {
            model: model.to_string(),
            messages,
            temperature: req.temperature.unwrap_or(0.3),
            max_tokens: req.max_tokens,
            stream: false,
        }
    }

    fn build_system_prompt(&self, req: &TranslateRequest) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "You are a professional translator. Translate from {} to {}.",
            req.lang_pair.source, req.lang_pair.target
        ));

        parts.push("Rules:".into());
        parts.push("1. Preserve all markup placeholders exactly (e.g., ⟦1⟧...⟦/1⟧)".into());
        parts.push("2. Maintain the same formatting, numbers, dates, and proper nouns.".into());
        parts.push("3. Do not add explanations, only the translation.".into());
        parts.push("4. Match the tone and style of the original.".into());

        if let Some(glossary) = &req.glossary {
            parts.push("\nGlossary (MUST follow):".into());
            for entry in glossary.terms_sorted() {
                if entry.do_not_translate {
                    parts.push(format!("- '{}' → DO NOT TRANSLATE (keep as '{}')", entry.source_term, entry.source_term));
                } else {
                    parts.push(format!("- '{}' → '{}'", entry.source_term, entry.target_term));
                }
            }
        }

        if let Some(style) = &req.style_guide {
            parts.push("\nStyle Guide:".into());
            parts.push(style.to_system_prompt());
        }

        parts.join("\n")
    }

    fn build_user_prompt(&self, req: &TranslateRequest) -> String {
        let mut parts = Vec::new();

        parts.push(format!("Translate the following {} segments:", req.units.len()));

        for (i, unit) in req.units.iter().enumerate() {
            parts.push(format!("\n--- Segment {} (ID: {}) ---", i + 1, unit.id));
            parts.push(format!("Type: {:?}", unit.kind));
            parts.push(format!("Text: {}", unit.text.raw));
        }

        parts.push("\n\nReturn JSON with translations array matching the segment order.".into());

        parts.join("\n")
    }

    fn parse_response(&self, response: OpenAIResponse, req: &TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let content = response.choices.first()
            .ok_or_else(|| ProviderError::Provider("Empty response".into()))?
            .message.content.clone();

        // Tenta parsear JSON
        let translations = self.extract_translations(&content, req)?;

        Ok(TranslateResponse {
            translations,
            tokens_used: response.usage.map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            latency_ms: 0, // Preenchido pelo caller
            model_used: response.model,
        })
    }

    fn extract_translations(&self, content: &str, req: &TranslateRequest) -> Result<Vec<UnitTranslation>, ProviderError> {
        // Tenta parsear como JSON estruturado
        if let Ok(parsed) = serde_json::from_str::<OpenAITranslationResponse>(content) {
            return Ok(parsed.translations.into_iter().enumerate().map(|(i, t)| {
                let unit_id = req.units.get(i).map(|u| u.id).unwrap_or_else(UnitId::new);
                UnitTranslation {
                    unit_id,
                    translated_text: MarkedText::new(t.translated_text),
                    confidence: t.confidence,
                    qa_warnings: Vec::new(),
                }
            }).collect());
        }

        // Fallback: assume resposta simples (um texto por linha)
        let lines: Vec<&str> = content.lines()
            .filter(|l| !l.trim().is_empty())
            .collect();

        Ok(req.units.iter().zip(lines).map(|(unit, text)| {
            UnitTranslation {
                unit_id: unit.id,
                translated_text: MarkedText::new(text),
                confidence: Some(0.8),
                qa_warnings: Vec::new(),
            }
        }).collect())
    }
}

#[async_trait]
impl TranslationProvider for OpenAICompatProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            max_context_tokens: 128000, // GPT-4 Turbo / modelos locais grandes
            supported_languages: vec![
                Language::new("en"), Language::new("pt"), Language::new("pt-BR"),
                Language::new("es"), Language::new("fr"), Language::new("de"),
                Language::new("it"), Language::new("ja"), Language::new("ko"),
                Language::new("zh"), Language::new("ru"), Language::new("ar"),
            ],
            is_local: self.config.base_url.contains("localhost") || self.config.base_url.contains("127.0.0.1"),
            supports_streaming: true,
            supports_glossary: true,
            default_model: Some(self.default_model.clone()),
        }
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let start = std::time::Instant::now();
        let request = self.build_request(&req);

        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let mut rb = self.client.post(&url).json(&request);

        if let Some(key) = &self.config.api_key {
            rb = rb.bearer_auth(key);
        }
        if let Some(org) = &self.config.organization {
            rb = rb.header("OpenAI-Organization", org);
        }

        let response = rb.send().await
            .map_err(|e| ProviderError::Network(e.into()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimited(Duration::from_secs(60)));
            }
            return Err(ProviderError::Provider(format!("HTTP {}: {}", status, error_text)));
        }

        let openai_resp: OpenAIResponse = response.json().await
            .map_err(|e| ProviderError::Provider(format!("Invalid JSON: {}", e)))?;

        let mut result = self.parse_response(openai_resp, &req)?;
        result.latency_ms = start.elapsed().as_millis() as u64;

        Ok(result)
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let mut rb = self.client.get(&url);
        if let Some(key) = &self.config.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await.map_err(|e| ProviderError::Network(e.into()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Provider("Health check failed".into()));
        }
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let mut rb = self.client.get(&url);
        if let Some(key) = &self.config.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await.map_err(|e| ProviderError::Network(e.into()))?;
        let models_resp: ModelsResponse = resp.json().await
            .map_err(|e| ProviderError::Provider(format!("Invalid JSON: {}", e)))?;

        Ok(models_resp.data.into_iter().map(|m| ModelInfo {
            id: m.id.clone(),
            name: m.id,
            languages: vec![Language::new("en")], // Placeholder
            max_context: 4096,
            is_local: self.capabilities().is_local,
            size_gb: None,
            license: None,
        }).collect())
    }
}

// ============================================================================
// Tipos de request/response OpenAI
// ============================================================================

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
    model: String,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelData>,
}

#[derive(Deserialize)]
struct ModelData {
    id: String,
    object: String,
    owned_by: String,
}

// Resposta estruturada esperada
#[derive(Deserialize)]
struct OpenAITranslationResponse {
    translations: Vec<TranslationItem>,
}

#[derive(Deserialize)]
struct TranslationItem {
    translated_text: String,
    confidence: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_creation() {
        let provider = OpenAICompatProvider::local_server("http://localhost:8080/v1".into(), None);
        assert!(provider.is_ok());
    }

    #[test]
    fn system_prompt_building() {
        let provider = OpenAICompatProvider::local_server("http://localhost:8080/v1".into(), None).unwrap();
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
        let prompt = provider.build_system_prompt(&req);
        assert!(prompt.contains("professional translator"));
        assert!(prompt.contains("⟦1⟧"));
    }
}