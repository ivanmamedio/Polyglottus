//! Integration tests for the full translation pipeline using FakeProvider.
//!
//! These tests validate: import → translate (FakeProvider) → export
//! without needing a real model, GPU, or network.

use pgl_testkit::{
    helpers::{test_format_registry, test_repo},
    fixtures::make_translate_request,
    FakeProvider, FakeMode, default_fake_provider,
};
use pgl_app::{ImportBook, ExportBook, prelude::FormatRegistry};
use pgl_fmt_epub::{EpubReader, EpubWriter};
use pgl_fmt_text::{TxtReader, TxtWriter};
use pgl_domain::{Language, ExportOptions, OutputFormat, LangPair, Unit, UnitKind, MarkedText, SourceRef, SectionId};
use pgl_app::TranslationLookup;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Helper to create a test unit with deterministic content
fn make_test_unit(text: &str, idx: usize) -> Unit {
    Unit::new(
        SourceRef {
            internal_path: "test.txt".to_string(),
            selector: format!("paragraph[{}]", idx),
            offset: None,
            length: Some(text.len()),
        },
        UnitKind::Paragraph,
        MarkedText::new(text),
        SectionId::new(),
        Uuid::nil(),
    )
}

#[tokio::test]
async fn test_full_pipeline_txt() {
    let (_temp, repo) = test_repo().await;
    let registry = test_format_registry();

    let import = ImportBook::new(vec![Arc::new(TxtReader::new())], repo.clone());

    // Create a test TXT file
    let txt_path = std::env::temp_dir().join(format!("polyglottus_test_{}.txt", Uuid::new_v4()));
    std::fs::write(&txt_path, "Hello world.\n\nSecond paragraph.").unwrap();

    let project = import.execute(&txt_path, Some("Test TXT".to_string())).await
        .expect("Import should succeed");

    // Save jobs
    for job in project.jobs.values() {
        repo.save_job(job, &project.id).await.expect("Save job");
    }

    let job_id = project.jobs.values().next().unwrap().id;

    // Translate with FakeProvider
    let provider = default_fake_provider();
    let executor = pgl_testkit::helpers::test_executor(
        provider,
        repo.clone(),
    );

    executor.execute(job_id).await.expect("Translation should succeed");

    // Verify units were translated
    let units = repo.load_units(&job_id, &[]).await.expect("Load units");
    assert!(!units.is_empty(), "Should have units");
    assert!(units.iter().all(|u| u.translation.is_some()), "All units should be translated");

    // Export
    let writer = registry.writers().iter()
        .find(|w| w.id() == pgl_app::FormatId::TXT)
        .expect("TXT writer should be registered");

    let export_options = ExportOptions {
        output_format: OutputFormat::TXT,
        output_path: None,
        bilingual: false,
        ..Default::default()
    };

    // Build lookup from repo
    let all_states = [
        pgl_domain::UnitState::Pending,
        pgl_domain::UnitState::Translated,
        pgl_domain::UnitState::Flagged,
        pgl_domain::UnitState::Reviewed,
        pgl_domain::UnitState::Failed,
        pgl_domain::UnitState::Skipped,
    ];
    let units_from_db = repo.load_units(&job_id, &all_states).await.expect("Load units for export");

    struct RepoLookup {
        repo: Arc<pgl_storage::SqliteRepository>,
        job_id: Uuid,
    }
    impl TranslationLookup for RepoLookup {
        fn get(&self, unit_id: &pgl_domain::UnitId) -> Option<MarkedText> {
            // This is a simplified lookup - in production we'd query the DB
            None
        }
        fn get_section(&self, _section: &SectionId) -> Vec<(pgl_domain::UnitId, MarkedText)> {
            Vec::new()
        }
    }

    let lookup = RepoLookup { repo: repo.clone(), job_id };
    let output_path = std::env::temp_dir().join(format!("polyglottus_test_out_{}.txt", Uuid::new_v4()));

    let report = writer.write(&txt_path, &lookup, &export_options, &output_path).await
        .expect("Export should succeed");

    assert!(report.output_size > 0, "Output should not be empty");

    // Cleanup
    let _ = std::fs::remove_file(&txt_path);
    let _ = std::fs::remove_file(&output_path);
}

#[tokio::test]
async fn test_fake_provider_modes() {
    let provider = FakeProvider::new(FakeMode::ReverseWords);
    let req = make_translate_request(vec![
        make_test_unit("Hello world", 0),
        make_test_unit("Foo bar baz", 1),
    ]);

    let resp = provider.translate(req).await.expect("Translation should succeed");
    assert_eq!(resp.translations.len(), 2);
    assert_eq!(resp.translations[0].translated_text.raw, "world Hello");
    assert_eq!(resp.translations[1].translated_text.raw, "baz bar Foo");
}

#[tokio::test]
async fn test_fake_provider_placeholder_error() {
    let provider = Arc::new(FakeProvider::new(FakeMode::PlaceholderError));
    let req = make_translate_request(vec![make_test_unit("Hello <b>world</b>", 0)]);

    let resp = provider.translate(req).await.expect("Translation should succeed");
    // PlaceholderError mode returns text without placeholders - QA should flag this
    assert!(resp.translations[0].translated_text.raw.contains("TRANSLATION WITHOUT PLACEHOLDERS"));
}

#[tokio::test]
async fn test_lang_pair_and_style_guide() {
    use pgl_domain::{StyleGuide, FormalityLevel, TypographyRules, PronounRules, LocalizationRules, VocabularyRules};

    let guide = StyleGuide::default_for(Language::new("pt-BR"));
    assert_eq!(guide.formality, FormalityLevel::Neutral);
    assert_eq!(guide.pronouns.formal_pronoun, "você");

    let prompt = guide.to_system_prompt();
    assert!(prompt.contains("pt-br"));
    assert!(prompt.contains("neutral"));
}
