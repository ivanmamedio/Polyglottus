//! Testes de integração end-to-end simplificados

use pgl_app::{
    TranslateRequest, TranslateResponse, LangPair, Language,
    ExportOptions, OutputFormat,
    JobConfig, Unit, UnitKind, MarkedText,
    SourceRef, SectionId, UnitId,
};
use pgl_domain::{Project, Job, Glossary, GlossaryEntry, StyleGuide, QaChecker as QaCheck, QaConfig, QaWarningCode, QaSeverity};
use pgl_provider_openai::OpenAICompatProvider;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::fs;
use uuid::Uuid;

/// Helper para criar arquivo TXT temporário
async fn create_temp_txt(content: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("source.txt");
    fs::write(&file_path, content).await.unwrap();
    (dir, file_path)
}

/// Teste com glossário
#[tokio::test]
async fn test_glossary_integration() {
    use pgl_domain::{Glossary, GlossaryEntry};
    
    let mut glossary = pgl_domain::Glossary::new(Language::new("en"), Language::new("pt"));
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
    
    let req = pgl_app::TranslateRequest {
        units: vec![],
        lang_pair: pgl_domain::LangPair {
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
    
    let openai_term = glossary.entries.get("OpenAI").unwrap();
    assert!(openai_term.do_not_translate);
    assert_eq!(openai_term.target_term, "OpenAI");
    
    println!("✓ Teste com glossário OK");
}

/// Teste com style guide
#[tokio::test]
async fn test_style_guide_integration() {
    let style_guide = pgl_domain::StyleGuide::default_for(Language::new("pt"));
    
    let req = pgl_app::TranslateRequest {
        units: vec![],
        lang_pair: pgl_domain::LangPair {
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
    
    println!("✓ Teste com style guide OK");
}

/// Teste de validação QA
#[tokio::test]
async fn test_qa_validation() {
    use pgl_domain::QaWarningCode;
    
    let qa_checker = QaCheck::new(QaConfig::default());
    
    // Cria unidade com placeholder usando HTML (que gera placeholders)
    let unit = Unit::new(
        SourceRef {
            internal_path: "test.txt".to_string(),
            selector: "1".to_string(),
            offset: None,
            length: None,
        },
        UnitKind::Paragraph,
        MarkedText::from_html("Press <b>⟦1⟧OK⟦/1⟧</b> to continue").unwrap(),
        SectionId::new(),
        Uuid::nil(),
    );
    
    let translated = MarkedText::from_html("Pressione <b>⟦1⟧OK⟦/1⟧</b> para continuar").unwrap();
    let unit_with_translation = Unit {
        translation: Some(translated),
        ..unit.clone()
    };
    let result = QaCheck::new(QaConfig::default()).check(&unit, &LangPair {
        source: Language::new("en"),
        target: Language::new("pt"),
    }, None);
    
    assert!(result.warnings.iter().all(|w| w.code != QaWarningCode::PlaceholderMismatch));
    
    let bad_unit = Unit {
        translation: Some(MarkedText::new("Pressione OK para continuar")),
        ..unit
    };
    let result = QaCheck::new(QaConfig::default()).check(&bad_unit, &LangPair {
        source: Language::new("en"),
        target: Language::new("pt"),
    }, None);
    
    assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::PlaceholderMismatch));
    
    println!("✓ Validação QA OK");
}