//! Integration tests for the full translation pipeline using FakeProvider.
//!
//! These tests validate: import → translate (FakeProvider) → unit storage
//! without needing a real model, GPU, or network.

use pgl_testkit::{
    helpers::{test_repo},
    fixtures::make_translate_request,
    FakeProvider, FakeMode, default_fake_provider,
};
use pgl_app::{ImportBook, ProjectRepository};
use pgl_fmt_text::{TxtReader, TxtWriter};
use pgl_domain::{Language, ExportOptions, OutputFormat, LangPair, Unit, UnitKind, MarkedText, SourceRef, SectionId, UnitState, StyleGuide};
use pgl_app::{TranslationLookup, TranslationProvider};
use pgl_domain::style_guide::{FormalityLevel, TypographyRules};
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

    let import = ImportBook::new(vec![Arc::new(TxtReader::new())], repo.clone());

    // Create a test TXT file
    let txt_path = std::env::temp_dir().join(format!("polyglottus_test_{}.txt", Uuid::new_v4()));
    std::fs::write(&txt_path, "Hello world.\n\nSecond paragraph.").unwrap();

    let project = import.execute(&txt_path, Some("Test TXT".to_string())).await
        .expect("Import should succeed");

    // Get job_id and units from the job in memory
    let job_id = project.jobs.values().next().unwrap().id;
    let job = project.jobs.values().next().unwrap().clone();
    assert!(!job.units.is_empty(), "Job should have units");

    // Save job to DB
    ProjectRepository::save_job(repo.as_ref(), &job, &project.id).await.expect("Save job");

    // Translate and save the first unit
    let provider = FakeProvider::new(FakeMode::Prefix);
    let first_unit = job.units.values().next().unwrap().clone();
    let req = make_translate_request(vec![first_unit.clone()]);
    let resp = TranslationProvider::translate(&provider, req).await.expect("Translation should succeed");
    if let Some(trans) = resp.translations.first() {
        let mut unit_mut = first_unit.clone();
        unit_mut.apply_translation(trans.translated_text.clone()).expect("Apply translation");
        ProjectRepository::save_unit(repo.as_ref(), &unit_mut, &job_id).await.expect("Save unit");
    }

    // Verify the unit was saved to DB by loading it back
    let saved_unit = ProjectRepository::load_unit(repo.as_ref(), &first_unit.id).await
        .expect("Load unit")
        .expect("Unit should exist in DB");
    assert!(saved_unit.translation.is_some(), "Unit should have translation");

    // Cleanup
    let _ = std::fs::remove_file(&txt_path);
}

#[tokio::test]
async fn test_fake_provider_modes() {
    let provider = FakeProvider::new(FakeMode::ReverseWords);
    let req = make_translate_request(vec![
        make_test_unit("Hello world", 0),
        make_test_unit("Foo bar baz", 1),
    ]);

    let resp = TranslationProvider::translate(&provider, req).await.expect("Translation should succeed");
    assert_eq!(resp.translations.len(), 2);
    assert_eq!(resp.translations[0].translated_text.raw, "world Hello");
    assert_eq!(resp.translations[1].translated_text.raw, "baz bar Foo");
}

#[tokio::test]
async fn test_fake_provider_placeholder_error() {
    let provider = FakeProvider::new(FakeMode::PlaceholderError);
    let req = make_translate_request(vec![make_test_unit("Hello <b>world</b>", 0)]);

    let resp = TranslationProvider::translate(&provider, req).await.expect("Translation should succeed");
    // PlaceholderError mode returns text without placeholders - QA should flag this
    assert!(resp.translations[0].translated_text.raw.contains("TRANSLATION WITHOUT PLACEHOLDERS"));
}

#[tokio::test]
async fn test_lang_pair_and_style_guide() {
    let guide = StyleGuide::default_for(Language::new("pt-BR"));
    assert_eq!(guide.formality, FormalityLevel::Neutral);
    assert_eq!(guide.pronouns.formal_pronoun, "você");

    let prompt = guide.to_system_prompt();
    assert!(prompt.contains("pt-br"));
    assert!(prompt.contains("neutral"));
}
