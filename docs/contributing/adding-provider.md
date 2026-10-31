# Como Adicionar um Provedor de Tradução

> Este guia explica como adicionar um novo provedor de inferência (LLM/MT) no Polyglottus.

---

## Visão Geral

A porta `TranslationProvider` (em `pgl-app`) define a interface:

```rust
#[async_trait]
pub trait TranslationProvider: Send + Sync + 'static {
    fn capabilities(&self) -> Capabilities;
    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError>;
    async fn health_check(&self) -> Result<(), ProviderError> { Ok(()) }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> { Ok(vec![]) }
}
```

---

## Passo a Passo

### 1. Crie o Crate

```bash
mkdir -p crates/adapters/providers/pgl-provider-meuprovider/src
```

**`Cargo.toml`:**
```toml
[package]
name = "pgl-provider-meuprovider"
version = "0.1.0"
edition = "2021"
description = "Provedor MeuProvider"
license = "MIT OR Apache-2.0"

[dependencies]
pgl-app = { path = "../../../app/pgl-app" }
pgl-domain = { path = "../../../domain/pgl-domain" }
reqwest = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
mockito = "1.2"
```

### 2. Implemente o Provider

```rust
// src/lib.rs
use pgl_app::{TranslationProvider, ProviderError, Capabilities, TranslateRequest, TranslateResponse, ModelInfo, Language, TokenUsage, UnitTranslation, QaWarning};
use pgl_domain::{Unit, UnitId, MarkedText, LangPair, Glossary, StyleGuide};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug};

pub struct MeuProvider {
    config: MeuProviderConfig,
    client: Client,
    default_model: String,
}

#[derive(Debug, Clone)]
pub struct MeuProviderConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

impl Default for MeuProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.meuprovider.com/v1".into(),
            api_key: None,
            timeout_secs: 120,
        }
    }
}

impl MeuProvider {
    pub fn new(config: MeuProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| ProviderError::Network(e.into()))?;
        
        Ok(Self {
            config,
            client,
            default_model: "meu-modelo-v1".into(),
        })
    }

    fn build_request(&self, req: &TranslateRequest) -> MeuRequest {
        // Constrói request no formato da API
        // Inclui glossário, style guide, contexto
        MeuRequest { ... }
    }

    fn parse_response(&self, resp: MeuResponse, req: &TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        // Converte resposta para TranslateResponse
        // Extrai translations, tokens, latency
    }
}

#[async_trait]
impl TranslationProvider for MeuProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            max_context_tokens: 32768,
            supported_languages: vec![
                Language::new("en"), Language::new("pt"), Language::new("es"),
                Language::new("fr"), Language::new("de"), Language::new("it"),
            ],
            is_local: self.config.base_url.contains("localhost") || self.config.base_url.contains("127.0.0.1"),
            supports_streaming: false,
            supports_glossary: true,
            default_model: Some(self.default_model.clone()),
        }
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let start = std::time::Instant::now();
        let request = self.build_request(&req);
        
        let url = format!("{}/translate", self.config.base_url.trim_end_matches('/'));
        let mut rb = self.client.post(&url).json(&request);
        
        if let Some(key) = &self.config.api_key {
            rb = rb.bearer_auth(key);
        }
        
        let response = rb.send().await
            .map_err(|e| ProviderError::Network(e.into()))?;
        
        let status = response.status();
        if !status.is_success() {
            let error = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimited(Duration::from_secs(60)));
            }
            return Err(ProviderError::Provider(format!("HTTP {}: {}", status, error)));
        }
        
        let meu_resp: MeuResponse = response.json().await
            .map_err(|e| ProviderError::Provider(format!("Invalid JSON: {}", e)))?;
        
        let mut result = self.parse_response(meu_resp, &req)?;
        result.latency_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let url = format!("{}/health", self.config.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await.map_err(|e| ProviderError::Network(e.into()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Provider("Health check failed".into()));
        }
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        // Opcional: lista modelos disponíveis na API
        Ok(vec![])
    }
}

// Tipos de request/response da API
#[derive(Serialize)]
struct MeuRequest { ... }

#[derive(Deserialize)]
struct MeuResponse { ... }
```

### 3. Registre no App

Nos binários (`pgl-cli`, `apps/desktop/src-tauri`):

```rust
let provider = MeuProvider::new(MeuProviderConfig {
    base_url: "https://api.meuprovider.com/v1".into(),
    api_key: Some("minha-chave".into()),
    ..Default::default()
})?;

// Use no ExecuteJob
let executor = ExecuteJob::new(
    Arc::new(provider),
    qa_checker,
    repo,
    events,
);
```

### 4. Testes de Contrato

```rust
// tests/contract.rs
use pgl_testkit::contract_tests::test_translation_provider;
use pgl_testkit::FakeProvider;

#[tokio::test]
async fn test_meuprovider_contract() {
    // Testa com FakeProvider primeiro (contrato)
    let provider = FakeProvider::new(FakeMode::Prefix);
    test_translation_provider(&provider).await;
}

#[tokio::test]
#[ignore] // Requer API key real
async fn test_meuprovider_real() {
    let provider = MeuProvider::new(MeuProviderConfig {
        base_url: std::env::var("MEU_API_URL").unwrap(),
        api_key: Some(std::env::var("MEU_API_KEY").unwrap()),
        ..Default::default()
    }).unwrap();
    
    test_translation_provider(&provider).await;
}
```

### 5. Suporte a Streaming (Opcional)

Se a API suporta streaming, implemente:

```rust
async fn translate_stream(&self, req: TranslateRequest) -> Result<StreamingTranslateResponse, ProviderError> {
    // Retorna stream de UnitTranslation conforme chegam
}
```

---

## Checklist de PR

- [ ] Crate criado em `crates/adapters/providers/pgl-provider-<nome>/`
- [ ] Implementa `TranslationProvider` trait
- [ ] `capabilities()` retorna info correta (incluindo `is_local`)
- [ ] `translate()` trata erros: `RateLimited`, `ContextTooLong`, `AuthFailed`
- [ ] `health_check()` funciona
- [ ] Suporta `glossary` e `style_guide` no prompt
- [ ] Trata `model_id` customizado
- [ ] Testes de contrato com `FakeProvider` passando
- [ ] Testes de integração (mockado com `mockito`)
- [ ] Documenta variáveis de ambiente necessárias
- [ ] `cargo deny check` passa
- [ ] `cargo clippy` passa
- [ ] README do crate com exemplos de uso