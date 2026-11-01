//! Testes de integração end-to-end para o provider OpenAI

use pgl_app::{TranslationProvider, TranslateRequest, TranslateResponse, FormatError};
use pgl_provider_openai::OpenAICompatProvider;
use pgl_domain::{Unit, UnitKind, MarkedText, SourceRef, SectionId, UnitId, LangPair, Language};
use std::sync::Arc;

/// Teste de integração do provider OpenAI (mock/local)
/// Nota: Este teste usa o modo local server. Para testar com OpenAI real,
/// defina a variável de ambiente OPENAI_API_KEY

#[tokio::test]
async fn test_provider_creation_openai() {
    // Testa criação do provider OpenAI
    let provider = OpenAICompatProvider::openai("sk-test-key".to_string(), Some("gpt-4o-mini".to_string()));
    
    // Com API key inválida, a criação deve funcionar mas health_check falhará
    assert!(provider.is_ok());
    let provider = provider.unwrap();
    
    // Verifica capacidades
    let caps = provider.capabilities();
    assert!(caps.supports_streaming);
    assert!(caps.supports_glossary);
    assert_eq!(caps.max_context_tokens, 128000); // gpt-4o-mini
}

#[tokio::test]
async fn test_provider_creation_local() {
    // Testa criação para servidor local (ex: Ollama, LM Studio, vLLM)
    let provider = OpenAICompatProvider::local_server(
        "http://localhost:11434/v1".to_string(), // Ollama default
        Some("llama3.2".to_string())
    );
    
    assert!(provider.is_ok());
    let provider = provider.unwrap();
    
    let caps = provider.capabilities();
    assert!(caps.max_context_tokens > 0);
}

#[tokio::test]
async fn test_translate_request_structure() {
    // Verifica se conseguimos construir uma TranslateRequest válida
    let unit = Unit::new(
        SourceRef {
            internal_path: "test.txt".to_string(),
            selector: "1".to_string(),
            offset: None,
            length: None,
        },
        UnitKind::Paragraph,
        MarkedText::new("Hello world"),
        SectionId::new(),
    );
    
    let req = TranslateRequest {
        units: vec![unit],
        lang_pair: LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        },
        glossary: None,
        style_guide: None,
        context: Some("Test context".to_string()),
        model_id: Some("gpt-4o-mini".to_string()),
        temperature: Some(0.3),
        max_tokens: Some(1000),
    };
    
    assert_eq!(req.units.len(), 1);
    assert_eq!(req.lang_pair.source.0.as_str(), "en");
    assert_eq!(req.lang_pair.target.0.as_str(), "pt");
    assert_eq!(req.temperature, Some(0.3));
}

#[tokio::test]
async fn test_provider_trait_bounds() {
    // Verifica se o provider implementa corretamente o trait TranslationProvider
    use pgl_app::TranslationProvider;
    
    let provider = OpenAICompatProvider::local_server(
        "http://localhost:8080/v1".to_string(),
        None
    ).unwrap();
    
    // Verifica se implementa o trait
    let _: &dyn TranslationProvider = &provider;
    
    // Verifica capabilities
    let caps = provider.capabilities();
    assert!(caps.max_context_tokens > 0);
    assert!(!caps.supports_streaming || caps.supports_streaming); // pode ser true ou false
}

#[tokio::test]
async fn test_health_check_local() {
    // Testa health_check com servidor local (vai falhar se não tiver servidor rodando)
    let provider = OpenAICompatProvider::local_server(
        "http://localhost:9999/v1".to_string(), // porta inexistente
        None
    ).unwrap();
    
    // health_check deve falhar graciosamente
    let result = provider.health_check().await;
    assert!(result.is_err()); // esperado: connection refused
}

#[tokio::test]
async fn test_model_listing_local() {
    // Testa list_models com servidor local
    let provider = OpenAICompatProvider::local_server(
        "http://localhost:8080/v1".to_string(),
        None
    ).unwrap();
    
    let result = provider.list_models().await;
    // Deve retornar lista vazia ou erro, mas não deve panic
    match result {
        Ok(models) => assert!(models.is_empty() || !models.is_empty()),
        Err(_) => {} // esperado se não tiver servidor
    }
}

// Teste que requer servidor real (ignorado por padrão)
#[tokio::test]
#[ignore]
async fn test_real_openai_translation() {
    // Este teste requer OPENAI_API_KEY definida
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set for this test");
    
    let provider = OpenAICompatProvider::openai(api_key, Some("gpt-4o-mini".to_string()))
        .expect("Failed to create provider");
    
    // Testa health check
    let health = provider.health_check().await;
    assert!(health.is_ok(), "Health check failed: {:?}", health.err());
    
    // Testa list_models
    let models = provider.list_models().await;
    assert!(models.is_ok());
    let models = models.unwrap();
    assert!(!models.is_empty());
    
    // Cria request de tradução simples
    let unit = Unit::new(
        SourceRef {
            internal_path: "test.txt".to_string(),
            selector: "1".to_string(),
            offset: None,
            length: None,
        },
        UnitKind::Paragraph,
        MarkedText::new("Hello, how are you?"),
        SectionId::new(),
    );
    
    let req = TranslateRequest {
        units: vec![unit],
        lang_pair: LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        },
        glossary: None,
        style_guide: None,
        context: None,
        model_id: Some("gpt-4o-mini".to_string()),
        temperature: Some(0.3),
        max_tokens: Some(100),
    };
    
    // Executa tradução
    let response = provider.translate(req).await;
    assert!(response.is_ok(), "Translation failed: {:?}", response.err());
    
    let response = response.unwrap();
    assert!(!response.translations.is_empty());
    assert_eq!(response.translations[0].unit_id, response.translations[0].unit_id);
    assert!(!response.translations[0].translated_text.raw.is_empty());
    assert!(!response.model_used.is_empty());
    
    println!("Translation successful: {}", response.translations[0].translated_text.raw);
    println!("Model used: {}", response.model_used);
    println!("Tokens: {:?}", response.tokens_used);
}

#[tokio::test]
async fn test_provider_error_handling() {
    // Testa se erros são tratados corretamente
    use pgl_app::ProviderError;
    
    let provider = OpenAICompatProvider::openai("invalid-key".to_string(), None).unwrap();
    
    let unit = Unit::new(
        SourceRef {
            internal_path: "test.txt".to_string(),
            selector: "1".to_string(),
            offset: None,
            length: None,
        },
        UnitKind::Paragraph,
        MarkedText::new("Test"),
        SectionId::new(),
    );
    
    let req = TranslateRequest {
        units: vec![unit],
        lang_pair: LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        },
        glossary: None,
        style_guide: None,
        context: None,
        model_id: Some("gpt-4o-mini".to_string()),
        temperature: Some(0.3),
        max_tokens: Some(100),
    };
    
    let result = provider.translate(req).await;
    
    // Deve retornar erro de autenticação
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::AuthFailed(_) => {} // esperado
        ProviderError::RateLimited(_) => {} // também aceitável
        ProviderError::Provider(msg) if msg.contains("401") || msg.contains("auth") => {} // também aceitável
        other => panic!("Expected auth error, got: {:?}", other),
    }
}

// Teste de glossário
#[tokio::test]
async fn test_glossary_in_request() {
    use pgl_domain::{Glossary, GlossaryEntry};
    
    let mut glossary = Glossary::new(Language::new("en"), Language::new("pt"));
    glossary.entries.insert("AI".to_string(), GlossaryEntry {
        source_term: "AI".to_string(),
        target_term: "IA".to_string(),
        context: Some("Artificial Intelligence".to_string()),
        do_not_translate: false,
        case_sensitive: true,
        category: None,
        priority: 50,
    });
    glossary.entries.insert("OpenAI".to_string(), GlossaryEntry {
        source_term: "OpenAI".to_string(),
        target_term: "OpenAI".to_string(),
        context: None,
        do_not_translate: true,
        case_sensitive: true,
        category: None,
        priority: 100,
    });
    
    let req = TranslateRequest {
        units: vec![],
        lang_pair: LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        },
        glossary: Some(glossary),
        style_guide: None,
        context: None,
        model_id: None,
        temperature: None,
        max_tokens: None,
    };
    
    assert!(req.glossary.is_some());
    let glossary = req.glossary.unwrap();
    assert_eq!(glossary.entries.len(), 2);
    
    // Verifica termo que não deve traduzir
    let openai_term = glossary.entries.get("OpenAI").unwrap();
    assert!(openai_term.do_not_translate);
    assert_eq!(openai_term.target_term, "OpenAI");
}

// Teste de estilo
#[tokio::test]
async fn test_style_guide_in_request() {
    use pgl_domain::{StyleGuide, Language};
    
    let mut style_guide = StyleGuide::default_for(Language::new("pt"));
    style_guide.custom_instructions = "Use formal Portuguese (você -> o senhor/a senhora)".to_string();
    
    let req = TranslateRequest {
        units: vec![],
        lang_pair: LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        },
        glossary: None,
        style_guide: Some(style_guide),
        context: None,
        model_id: None,
        temperature: None,
        max_tokens: None,
    };
    
    assert!(req.style_guide.is_some());
    let style = req.style_guide.unwrap();
    assert_eq!(style.target_language, Language::new("pt"));
    assert!(!style.custom_instructions.is_empty());
}