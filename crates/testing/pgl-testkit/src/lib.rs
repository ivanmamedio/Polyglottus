//! Test Kit - FakeProvider, fixtures, helpers

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod fake_provider;
pub mod fixtures;
pub mod helpers;
pub mod contract_tests;

use pgl_app::{TranslationProvider, ProviderError, Capabilities, TranslateRequest, TranslateResponse, ModelInfo, Language, TokenUsage, UnitTranslation, QaWarning};
use pgl_domain::{Unit, UnitId, MarkedText, LangPair, Glossary, StyleGuide};
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;

/// FakeProvider determinístico para testes
///
/// Inverte o texto ou adiciona prefixo para simular tradução
/// sem precisar de modelo real, GPU, rede, etc.
#[derive(Debug, Clone)]
pub struct FakeProvider {
    mode: FakeMode,
    delay_ms: u64,
    should_fail: bool,
    fail_after: Option<usize>,
    call_count: std::sync::atomic::AtomicUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeMode {
    /// Inverte cada palavra
    ReverseWords,
    /// Adiciona prefixo "TRADUZIDO: "
    Prefix,
    /// Ecoa o original
    Echo,
    /// Retorna JSON estruturado
    Structured,
    /// Simula erro de placeholder
    PlaceholderError,
    /// Simula repetição (para testar QA)
    Repetition,
}

impl FakeProvider {
    pub fn new(mode: FakeMode) -> Self {
        Self {
            mode,
            delay_ms: 0,
            should_fail: false,
            fail_after: None,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub fn fail_after(mut self, n: usize) -> Self {
        self.fail_after = Some(n);
        self
    }

    fn generate_translation(&self, unit: &Unit) -> Result<MarkedText, ProviderError> {
        match self.mode {
            FakeMode::ReverseWords => {
                let words: Vec<&str> = unit.text.raw.split_whitespace().collect();
                let reversed: Vec<String> = words.iter().rev().map(|w| w.to_string()).collect();
                Ok(MarkedText::new(reversed.join(" ")))
            }
            FakeMode::Prefix => {
                Ok(MarkedText::new(format!("TRADUZIDO: {}", unit.text.raw)))
            }
            FakeMode::Echo => {
                Ok(unit.text.clone())
            }
            FakeMode::Structured => {
                Ok(MarkedText::new(format!("[{}] {}", unit.kind, unit.text.raw)))
            }
            FakeMode::PlaceholderError => {
                // Retorna texto sem placeholders (vai falhar QA)
                Ok(MarkedText::new("TRANSLATION WITHOUT PLACEHOLDERS"))
            }
            FakeMode::Repetition => {
                Ok(MarkedText::new("word word word word word word word word word word"))
            }
        }
    }
}

#[async_trait]
impl TranslationProvider for FakeProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            max_context_tokens: 8192,
            supported_languages: vec![
                Language::new("en"), Language::new("pt"), Language::new("es"),
                Language::new("fr"), Language::new("de"), Language::new("it"),
            ],
            is_local: true,
            supports_streaming: false,
            supports_glossary: true,
            default_model: Some("fake".into()),
        }
    }

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError> {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if self.should_fail || self.fail_after.map_or(false, |n| count >= n) {
            return Err(ProviderError::Provider("Simulated failure".into()));
        }

        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }

        let translations = req.units.iter().map(|unit| {
            let translated = self.generate_translation(unit).unwrap();
            UnitTranslation {
                unit_id: unit.id,
                translated_text: translated,
                confidence: Some(0.95),
                qa_warnings: Vec::new(),
            }
        }).collect();

        Ok(TranslateResponse {
            translations,
            tokens_used: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
            latency_ms: self.delay_ms,
            model_used: "fake".into(),
        })
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![ModelInfo {
            id: "fake".into(),
            name: "Fake Provider".into(),
            languages: vec![Language::new("en"), Language::new("pt")],
            max_context: 8192,
            is_local: true,
            size_gb: None,
            license: Some("MIT".into()),
        }])
    }
}

/// Cria FakeProvider padrão para testes
pub fn default_fake_provider() -> Arc<FakeProvider> {
    Arc::new(FakeProvider::new(FakeMode::Prefix))
}

/// Cria FakeProvider que falha
pub fn failing_fake_provider() -> Arc<FakeProvider> {
    Arc::new(FakeProvider::new(FakeMode::Prefix).with_failure())
}

/// Cria FakeProvider com erro de placeholder
pub fn placeholder_error_provider() -> Arc<FakeProvider> {
    Arc::new(FakeProvider::new(FakeMode::PlaceholderError))
}

/// Cria FakeProvider com repetição (para testar QA)
pub fn repetition_provider() -> Arc<FakeProvider> {
    Arc::new(FakeProvider::new(FakeMode::Repetition))
}

/// Fixtures de teste
pub mod fixtures {
    use super::*;
    use pgl_domain::{Unit, UnitKind, SectionId, SourceRef, Language, MarkedText, UnitId};

    /// Cria unidade de teste simples
    pub fn make_unit(text: &str) -> Unit {
        Unit::new(
            SourceRef {
                internal_path: "test.xhtml".into(),
                selector: "//p[1]".into(),
                offset: None,
                length: None,
            },
            UnitKind::Paragraph,
            MarkedText::new(text),
            SectionId::new(),
        )
    }

    /// Cria unidade com marcação
    pub fn make_marked_unit(html: &str) -> Unit {
        let marked = MarkedText::from_html(html).unwrap();
        Unit::new(
            SourceRef {
                internal_path: "test.xhtml".into(),
                selector: "//p[1]".into(),
                offset: None,
                length: None,
            },
            UnitKind::Paragraph,
            marked,
            SectionId::new(),
        )
    }

    /// Cria request de tradução de teste
    pub fn make_translate_request(units: Vec<Unit>) -> TranslateRequest {
        TranslateRequest {
            units,
            lang_pair: LangPair {
                source: Language::new("en"),
                target: Language::new("pt"),
            },
            glossary: None,
            style_guide: None,
            context: None,
            model_id: None,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Glossário de teste
    pub fn test_glossary() -> Glossary {
        let mut g = Glossary::new(Language::new("en"), Language::new("pt"));
        g.add_entry(pgl_domain::GlossaryEntry {
            source_term: "Hello".into(),
            target_term: "Olá".into(),
            context: None,
            category: Some("greeting".into()),
            case_sensitive: false,
            do_not_translate: false,
            priority: 80,
        });
        g.add_entry(pgl_domain::GlossaryEntry {
            source_term: "World".into(),
            target_term: "Mundo".into(),
            context: None,
            category: None,
            case_sensitive: false,
            do_not_translate: false,
            priority: 50,
        });
        g
    }
}

/// Helpers para testes de integração
pub mod helpers {
    use super::*;
    use pgl_app::{ImportBook, PlanTranslation, ExecuteJob, ExportBook, FormatRegistry};
    use pgl_domain::{Language, ExportOptions, OutputFormat, JobConfig};
    use pgl_fmt_epub::{EpubReader, EpubWriter};
    use pgl_fmt_text::{TxtReader, TxtWriter};
    use pgl_storage::SqliteRepository;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Cria registry de formatos para testes
    pub fn test_format_registry() -> FormatRegistry {
        let mut registry = FormatRegistry::new();
        registry.register_reader(Arc::new(EpubReader::new()));
        registry.register_writer(Arc::new(EpubWriter::new()));
        registry.register_reader(Arc::new(TxtReader::new()));
        registry.register_writer(Arc::new(TxtWriter::new()));
        registry
    }

    /// Cria repositório temporário para testes
    pub async fn test_repo() -> (TempDir, Arc<SqliteRepository>) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let repo = Arc::new(SqliteRepository::new(&db_path).await.unwrap());
        (temp, repo)
    }

    /// Cria ImportBook para testes
    pub fn test_import_book(registry: FormatRegistry, repo: Arc<SqliteRepository>) -> ImportBook {
        ImportBook::new(registry.readers().to_vec(), repo)
    }

    /// Cria PlanTranslation para testes
    pub fn test_planner(catalog: Arc<dyn pgl_app::ModelCatalog>) -> PlanTranslation {
        PlanTranslation::new(catalog)
    }

    /// Cria ExecuteJob para testes
    pub fn test_executor(
        provider: Arc<dyn pgl_app::TranslationProvider>,
        repo: Arc<SqliteRepository>,
    ) -> ExecuteJob<_, pgl_domain::QaChecker, _, _> {
        ExecuteJob::new(
            provider,
            Arc::new(pgl_domain::QaChecker::new(Default::default())),
            repo,
            Arc::new(TestEventSink),
        )
    }

    /// Event sink silencioso para testes
    struct TestEventSink;
    #[async_trait::async_trait]
    impl pgl_app::EventSink for TestEventSink {
        async fn emit(&self, _: pgl_app::AppEvent) {}
    }
}

/// Testes de contrato para adaptadores
pub mod contract_tests {
    use super::*;
    use pgl_app::{FormatReader, FormatWriter, SourceDocument, TranslationProvider, OcrEngine, ProjectRepository, SecretStore, ModelCatalog};
    use pgl_domain::{Unit, UnitKind, SectionId, MarkedText, SourceRef, Language};

    /// Testa contrato FormatReader
    pub async fn test_format_reader<R: FormatReader>(reader: &R, test_file: &std::path::Path) {
        let doc = reader.open(test_file).await.expect("Failed to open");
        let mut units = Vec::new();
        while let Some(unit_result) = doc.units().next() {
            units.push(unit_result.expect("Failed to read unit"));
        }
        assert!(!units.is_empty(), "Should extract at least one unit");
    }

    /// Testa contrato FormatWriter
    pub async fn test_format_writer<W: FormatWriter>(
        writer: &W,
        source: &std::path::Path,
        translations: &dyn pgl_app::TranslationLookup,
        out: &std::path::Path,
    ) {
        let options = ExportOptions::default();
        let report = writer.write(source, translations, &options, out).await.expect("Failed to write");
        assert!(out.exists(), "Output file should exist");
        assert!(report.output_size > 0, "Output should not be empty");
    }

    /// Testa contrato TranslationProvider
    pub async fn test_translation_provider<P: TranslationProvider>(provider: &P) {
        let caps = provider.capabilities();
        assert!(caps.max_context_tokens > 0);
        assert!(!caps.supported_languages.is_empty());

        provider.health_check().await.expect("Health check failed");

        let models = provider.list_models().await.expect("List models failed");
        // Pode ser vazio para alguns provedores
    }

    /// Testa contrato OcrEngine
    pub async fn test_ocr_engine<O: OcrEngine>(engine: &O) {
        assert!(!engine.supported_languages().is_empty());

        // Testa com imagem simples (1x1 pixel PNG)
        let png_1x1 = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\xd7c\xf8\xff\xff?\x00\x05\xfe\x02\xfe\xa7\xd5\x85\xd0\x00\x00\x00\x00IEND\xaeB`\x82";
        let result = engine.recognize(png_1x1, "image/png", None).await;
        // Pode falhar por imagem sem texto, mas não deve panicar
        let _ = result;
    }

    /// Testa contrato ProjectRepository
    pub async fn test_project_repository<R: ProjectRepository>(repo: &R) {
        use pgl_domain::{Project, JobConfig};
        use std::path::PathBuf;

        let project = Project::new("Test", PathBuf::from("test.epub"), "EPUB");
        repo.save_project(&project).await.expect("Save project failed");

        let loaded = repo.load_project(&project.id).await.expect("Load project failed");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "Test");

        let projects = repo.list_projects().await.expect("List projects failed");
        assert!(!projects.is_empty());

        repo.delete_project(&project.id).await.expect("Delete project failed");
    }

    /// Testa contrato SecretStore
    pub async fn test_secret_store<S: SecretStore>(store: &S) {
        store.set("test_key", "test_value").await.expect("Set secret failed");
        let value = store.get("test_key").await.expect("Get secret failed");
        assert_eq!(value, Some("test_value".to_string()));

        store.delete("test_key").await.expect("Delete secret failed");
        let value = store.get("test_key").await.expect("Get secret failed");
        assert_eq!(value, None);
    }

    /// Testa contrato ModelCatalog
    pub async fn test_model_catalog<M: ModelCatalog>(catalog: &M) {
        let hw = catalog.detect_hardware().await.expect("Detect hardware failed");
        assert!(hw.cpu_cores > 0);
        assert!(hw.ram_gb > 0.0);

        let rec = catalog.recommend_model(&hw, &pgl_domain::LangPair {
            source: Language::new("en"),
            target: Language::new("pt"),
        }).await.expect("Recommend model failed");
        assert!(!rec.model_id.is_empty());
        assert!(rec.download_size_gb > 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_provider_basic() {
        let provider = default_fake_provider();
        let req = fixtures::make_translate_request(vec![fixtures::make_unit("Hello world")]);
        let resp = provider.translate(req).await.unwrap();
        assert_eq!(resp.translations.len(), 1);
        assert!(resp.translations[0].translated_text.raw.starts_with("TRADUZIDO:"));
    }

    #[tokio::test]
    async fn fake_provider_reverse() {
        let provider = Arc::new(FakeProvider::new(FakeMode::ReverseWords));
        let req = fixtures::make_translate_request(vec![fixtures::make_unit("Hello world")]);
        let resp = provider.translate(req).await.unwrap();
        assert_eq!(resp.translations[0].translated_text.raw, "world Hello");
    }

    #[tokio::test]
    async fn fake_provider_failure() {
        let provider = failing_fake_provider();
        let req = fixtures::make_translate_request(vec![fixtures::make_unit("Test")]);
        let result = provider.translate(req).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_fixtures() {
        let unit = fixtures::make_unit("Test");
        assert_eq!(unit.text.raw, "Test");
        assert_eq!(unit.kind, UnitKind::Paragraph);
    }
}