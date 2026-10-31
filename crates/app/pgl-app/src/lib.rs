//! Casos de uso da aplicação + Portas (traits) para adaptadores
//!
//! Este crate define a lógica de negócio (use cases) e as interfaces
//! (traits) que os adaptadores de infraestrutura devem implementar.
//! O domínio puro (pgl-domain) não conhece estas traits.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod events;
pub mod ports;
pub mod use_cases;

use std::path::Path;
use std::sync::Arc;
use pgl_domain::prelude::*;

/// Erros da camada de aplicação
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Format not supported: {0}")]
    UnsupportedFormat(String),

    #[error("Format error: {0}")]
    Format(#[from] FormatError),

    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("OCR error: {0}")]
    Ocr(#[from] OcrError),

    #[error("Secrets error: {0}")]
    Secrets(#[from] SecretsError),

    #[error("Models error: {0}")]
    Models(#[from] ModelsError),

    #[error("Project not found: {0}")]
    ProjectNotFound(Uuid),

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Re-export dos tipos de domínio mais usados
pub use pgl_domain::{
    Capabilities, ExportOptions, ExportReport, FidelityReport, Glossary, GlossaryEntry,
    Job, JobConfig, JobProgress, JobState, LangPair, Language, MarkedText, OutputFormat,
    Project, ProjectMetadata, QaResult, QaSeverity, QaWarning, QaWarningCode, SectionId,
    Segment, SegmentationRule, SourceRef, StyleGuide, TokenUsage, TranslateRequest,
    TranslateResponse, Unit, UnitId, UnitKind, UnitState, UnitTranslation,
};

/// Prelude conveniente
pub mod prelude {
    pub use crate::{
        AppError, FormatReader, FormatWriter, FormatError, ProviderError, OcrEngine,
        OcrError, ProjectRepository, StorageError, SecretStore, SecretsError,
        TranslationProvider, ModelCatalog, ModelsError, EventSink,
        ImportBook, PlanTranslation, ExecuteJob, ReviewUnit, ExportBook,
    };
    pub use pgl_domain::prelude::*;
}

// ============================================================================
// PORTAS (Traits) - Interface para adaptadores
// ============================================================================

/// Erro de formato
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("Sniffing failed: {0}")]
    SniffingFailed(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Fidelity check failed: {0}")]
    FidelityError(String),
}

/// Leitor de formato: extrai unidades de forma incremental (streaming)
#[async_trait::async_trait]
pub trait FormatReader: Send + Sync + 'static {
    /// Identificador único do formato (ex: "epub", "docx")
    fn id(&self) -> FormatId;

    /// Tenta detectar se é este formato (magic bytes, extensão)
    fn sniff(&self, name: &str, head: &[u8]) -> Confidence;

    /// Abre documento e retorna iterator de unidades
    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError>;

    /// Extensões de arquivo suportadas
    fn extensions(&self) -> &'static [&'static str];

    /// Tipos MIME suportados
    fn mime_types(&self) -> &'static [&'static str] { &[] }
}

/// Identificador de formato
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FormatId(pub &'static str);

impl FormatId {
    pub const EPUB: FormatId = FormatId("epub");
    pub const HTML: FormatId = FormatId("html");
    pub const TXT: FormatId = FormatId("txt");
    pub const MD: FormatId = FormatId("md");
    pub const SRT: FormatId = FormatId("srt");
    pub const VTT: FormatId = FormatId("vtt");
    pub const DOCX: FormatId = FormatId("docx");
    pub const PPTX: FormatId = FormatId("pptx");
    pub const XLSX: FormatId = FormatId("xlsx");
    pub const ODT: FormatId = FormatId("odt");
    pub const ODP: FormatId = FormatId("odp");
    pub const ODS: FormatId = FormatId("ods");
    pub const FB2: FormatId = FormatId("fb2");
    pub const PDF: FormatId = FormatId("pdf");
    pub const PO: FormatId = FormatId("po");
    pub const XLIFF: FormatId = FormatId("xliff");
    pub const TMX: FormatId = FormatId("tmx");
    pub const JSON_I18N: FormatId = FormatId("json-i18n");
}

/// Confiança na detecção (0-100)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Confidence(pub u8);

impl Confidence {
    pub fn none() -> Self { Self(0) }
    pub fn low() -> Self { Self(25) }
    pub fn medium() -> Self { Self(50) }
    pub fn high() -> Self { Self(75) }
    pub fn certain() -> Self { Self(100) }
    pub fn value(&self) -> u8 { self.0 }
}

/// Documento fonte aberto (produz unidades via streaming)
#[async_trait::async_trait]
pub trait SourceDocument: Send + Sync {
    /// Iterator de unidades (streaming - não carrega tudo na memória)
    fn units(&mut self) -> Box<dyn Iterator<Item = Result<Unit, FormatError>> + '_>;

    /// Assets referenciados (imagens, fontes, CSS, etc.)
    fn assets(&self) -> Vec<AssetRef>;

    /// Relatório de fidelidade do formato
    fn fidelity(&self) -> FidelityReport;

    /// Metadados do documento
    fn metadata(&self) -> DocumentMetadata;
}

/// Referência a asset (imagem, fonte, etc.)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AssetRef {
    pub id: String,
    pub internal_path: String,
    pub mime_type: String,
    pub size: u64,
    pub hash: String, // SHA-256 para deduplicação
}

/// Metadados do documento
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<Language>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub page_count: Option<usize>,
    pub word_count: Option<usize>,
}

/// Escritor de formato: aplica traduções ao original (patch no lugar) ou regenera
#[async_trait::async_trait]
pub trait FormatWriter: Send + Sync + 'static {
    fn id(&self) -> FormatId;

    /// Aplica traduções e gera arquivo de saída
    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError>;

    /// Verifica se suporta saída bilíngue
    fn supports_bilingual(&self) -> bool { false }

    /// Formatos de saída suportados
    fn output_formats(&self) -> &'static [OutputFormat];
}

/// Lookup de traduções para escrita
pub trait TranslationLookup: Send + Sync {
    fn get(&self, unit_id: &UnitId) -> Option<MarkedText>;
    fn get_section(&self, section: &SectionId) -> Vec<(UnitId, MarkedText)>;
}

/// Provedor de tradução
#[async_trait::async_trait]
pub trait TranslationProvider: Send + Sync + 'static {
    fn capabilities(&self) -> Capabilities;

    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError>;

    /// Health check
    async fn health_check(&self) -> Result<(), ProviderError> { Ok(()) }

    /// Lista modelos disponíveis
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> { Ok(vec![]) }
}

/// Erro de provedor
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limited: retry after {0:?}")]
    RateLimited(std::time::Duration),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Context too long: {0} tokens (max {1})")]
    ContextTooLong(usize, usize),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Timeout")]
    Timeout,
}

/// Info de modelo
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub languages: Vec<Language>,
    pub max_context: usize,
    pub is_local: bool,
    pub size_gb: Option<f32>,
    pub license: Option<String>,
}

/// Motor OCR
#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn supported_languages(&self) -> &'static [Language];

    async fn recognize(&self, image: &[u8], mime: &str, lang_hint: Option<&Language>)
        -> Result<OcrResult, OcrError>;
}

/// Resultado de OCR
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
    pub language: Option<Language>,
    pub bounding_boxes: Vec<BoundingBox>,
}

/// Bounding box de texto detectado
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub confidence: f32,
}

/// Erro de OCR
#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("Recognition failed: {0}")]
    RecognitionFailed(String),

    #[error("Language not supported: {0}")]
    LanguageNotSupported(String),

    #[error("Engine error: {0}")]
    Engine(String),
}

/// Repositório de projetos (persistência)
#[async_trait::async_trait]
pub trait ProjectRepository: Send + Sync + 'static {
    async fn save_project(&self, project: &Project) -> Result<(), StorageError>;
    async fn load_project(&self, id: &Uuid) -> Result<Option<Project>, StorageError>;
    async fn delete_project(&self, id: &Uuid) -> Result<(), StorageError>;
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError>;

    async fn save_job(&self, job: &Job) -> Result<(), StorageError>;
    async fn load_job(&self, id: &Uuid) -> Result<Option<Job>, StorageError>;
    async fn list_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError>;

    async fn save_unit(&self, unit: &Unit) -> Result<(), StorageError>;
    async fn load_unit(&self, id: &UnitId) -> Result<Option<Unit>, StorageError>;
    async fn load_units(&self, job_id: &Uuid, states: &[UnitState]) -> Result<Vec<Unit>, StorageError>;
    async fn update_unit_state(&self, id: &UnitId, state: UnitState) -> Result<(), StorageError>;
    async fn update_unit_translation(&self, id: &UnitId, translation: &MarkedText, warnings: &[QaWarning]) -> Result<(), StorageError>;
}

/// Erro de storage
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(String),
}

/// Resumo de projeto para listagem
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub source_file: String,
    pub source_format: String,
    pub created_at: String,
    pub updated_at: String,
    pub overall_progress: u8,
}

/// Resumo de job para listagem
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub target_language: Language,
    pub state: JobState,
    pub progress: JobProgress,
    pub created_at: String,
    pub updated_at: String,
}

/// Cofre de segredos (chaves de API)
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync + 'static {
    async fn get(&self, key: &str) -> Result<Option<String>, SecretsError>;
    async fn set(&self, key: &str, value: &str) -> Result<(), SecretsError>;
    async fn delete(&self, key: &str) -> Result<(), SecretsError>;
    async fn list(&self) -> Result<Vec<String>, SecretsError>;
}

/// Erro de segredos
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Backend error: {0}")]
    Backend(String),
}

/// Catálogo de modelos
#[async_trait::async_trait]
pub trait ModelCatalog: Send + Sync + 'static {
    async fn detect_hardware(&self) -> Result<HardwareProfile, ModelsError>;
    async fn recommend_model(&self, profile: &HardwareProfile, lang_pair: &LangPair) -> Result<ModelRecommendation, ModelsError>;
    async fn download_model(&self, model_id: &str, progress: &dyn Fn(u64, u64)) -> Result<ModelInfo, ModelsError>;
    async fn list_installed(&self) -> Result<Vec<ModelInfo>, ModelsError>;
    async fn remove_model(&self, model_id: &str) -> Result<(), ModelsError>;
    async fn get_model_path(&self, model_id: &str) -> Result<Option<std::path::PathBuf>, ModelsError>;
}

/// Perfil de hardware detectado
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HardwareProfile {
    pub cpu_cores: usize,
    pub ram_gb: f32,
    pub gpu: Option<GpuInfo>,
    pub platform: String,
    pub arch: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_gb: f32,
    pub cuda: bool,
    pub metal: bool,
    pub vulkan: bool,
}

/// Recomendação de modelo
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelRecommendation {
    pub model_id: String,
    pub reason: String,
    pub estimated_speed: String, // "fast", "medium", "slow"
    pub estimated_ram_gb: f32,
    pub quality_tier: QualityTier,
    pub download_size_gb: f32,
    pub license: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTier {
    Draft,
    Good,
    High,
    Premium,
}

/// Erro de modelos
#[derive(Debug, thiserror::Error)]
pub enum ModelsError {
    #[error("Model not found: {0}")]
    NotFound(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Insufficient resources: need {need}, have {have}")]
    InsufficientResources { need: String, have: String },

    #[error("License not accepted: {0}")]
    LicenseNotAccepted(String),

    #[error("Hardware detection failed: {0}")]
    HardwareDetectionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Event sink para progresso, logs, etc.
#[async_trait::async_trait]
pub trait EventSink: Send + Sync + 'static {
    async fn emit(&self, event: AppEvent);
}

/// Eventos da aplicação
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AppEvent {
    JobStarted { job_id: Uuid },
    JobProgress { job_id: Uuid, progress: JobProgress },
    JobPaused { job_id: Uuid },
    JobResumed { job_id: Uuid },
    JobCompleted { job_id: Uuid, report: ExportReport },
    JobFailed { job_id: Uuid, error: String },
    JobCancelled { job_id: Uuid },

    UnitTranslated { job_id: Uuid, unit_id: UnitId },
    UnitFlagged { job_id: Uuid, unit_id: UnitId, warnings: Vec<QaWarning> },
    UnitFailed { job_id: Uuid, unit_id: UnitId, error: String },

    ModelDownloading { model_id: String, progress: f32 },
    ModelDownloaded { model_id: String },
    ModelLoadError { model_id: String, error: String },

    Log { level: String, target: String, message: String },
}

// ============================================================================
// CASOS DE USO (Use Cases)
// ============================================================================

/// Importa livro e cria projeto
pub struct ImportBook<R: FormatReader, Repo: ProjectRepository> {
    readers: Vec<Arc<R>>,
    repo: Arc<Repo>,
}

impl<R: FormatReader, Repo: ProjectRepository> ImportBook<R, Repo> {
    pub fn new(readers: Vec<Arc<R>>, repo: Arc<Repo>) -> Self {
        Self { readers, repo }
    }

    pub async fn execute(&self, file_path: &Path, project_name: Option<String>) -> Result<Project, AppError> {
        // Detecta formato
        let reader = self.detect_format(file_path).await?;
        let mut doc = reader.open(file_path).await?;

        // Cria projeto
        let name = project_name.unwrap_or_else(|| {
            file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        let mut project = Project::new(name, file_path.to_path_buf(), reader.id().0.to_string());

        // Extrai unidades (streaming)
        let mut units = Vec::new();
        while let Some(unit_result) = doc.units().next() {
            units.push(unit_result?);
        }

        // Cria job padrão
        let job_id = project.add_job(JobConfig::default());
        if let Some(job) = project.job_mut(&job_id) {
            job.add_units(units);
        }

        // Salva
        self.repo.save_project(&project).await?;

        Ok(project)
    }

    async fn detect_format(&self, path: &Path) -> Result<Arc<R>, AppError> {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let head = std::fs::read(path)?.into_iter().take(1024).collect::<Vec<_>>();

        let mut best = (Confidence::none(), None);
        for reader in &self.readers {
            let conf = reader.sniff(name, &head);
            if conf > best.0 {
                best = (conf, Some(reader.clone()));
            }
        }

        best.1.ok_or_else(|| AppError::UnsupportedFormat(format!("No reader for {}", name)))
    }
}

/// Planeja tradução (estima tempo, custo, valida config)
pub struct PlanTranslation<M: ModelCatalog> {
    catalog: Arc<M>,
}

impl<M: ModelCatalog> PlanTranslation<M> {
    pub fn new(catalog: Arc<M>) -> Self {
        Self { catalog }
    }

    pub async fn execute(&self, job: &Job) -> Result<TranslationPlan, AppError> {
        let profile = self.catalog.detect_hardware().await?;
        let recommendation = self.catalog.recommend_model(&profile, &job.config.lang_pair).await?;

        let total_units = job.units.len();
        let total_chars: usize = job.units.values().map(|u| u.text.raw.len()).sum();

        // Estimativa grosseira
        let tokens_estimate = total_chars / 3; // ~3-4 chars/token
        let time_per_1k_tokens = match recommendation.estimated_speed.as_str() {
            "fast" => 2.0,
            "medium" => 5.0,
            _ => 10.0,
        };
        let estimated_seconds = (tokens_estimate as f64 / 1000.0) * time_per_1k_tokens / job.config.max_parallel as f64;

        Ok(TranslationPlan {
            total_units,
            total_chars,
            estimated_tokens: tokens_estimate,
            estimated_time_secs: estimated_seconds as u64,
            recommended_model: recommendation,
            hardware_profile: profile,
            warnings: vec![],
        })
    }
}

/// Plano de tradução com estimativas
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationPlan {
    pub total_units: usize,
    pub total_chars: usize,
    pub estimated_tokens: usize,
    pub estimated_time_secs: u64,
    pub recommended_model: ModelRecommendation,
    pub hardware_profile: HardwareProfile,
    pub warnings: Vec<String>,
}

/// Executa job de tradução
pub struct ExecuteJob<P: TranslationProvider, Q: QaChecker, Repo: ProjectRepository, E: EventSink> {
    provider: Arc<P>,
    qa_checker: Arc<Q>,
    repo: Arc<Repo>,
    events: Arc<E>,
}

impl<P, Q, Repo, E> ExecuteJob<P, Q, Repo, E>
where
    P: TranslationProvider,
    Q: QaChecker,
    Repo: ProjectRepository,
    E: EventSink,
{
    pub fn new(provider: Arc<P>, qa_checker: Arc<Q>, repo: Arc<Repo>, events: Arc<E>) -> Self {
        Self { provider, qa_checker, repo, events }
    }

    pub async fn execute(&self, job_id: Uuid) -> Result<(), AppError> {
        let mut job = self.repo.load_job(&job_id).await?
            .ok_or(AppError::JobNotFound(job_id))?;

        if !matches!(job.state, JobState::Created | JobState::Paused) {
            return Err(AppError::Validation("Job cannot be started".into()));
        }

        job.start();
        self.repo.save_job(&job).await?;
        self.events.emit(AppEvent::JobStarted { job_id }).await;

        // Loop de processamento em lotes
        while job.can_resume() {
            job.update_progress();
            self.repo.save_job(&job).await?;
            self.events.emit(AppEvent::JobProgress { job_id, progress: job.progress.clone() }).await;

            let batch = job.pending_units(job.config.batch_size);
            if batch.is_empty() { break; }

            // Prepara requisição
            let req = TranslateRequest {
                units: batch.iter().map(|u| (*u).clone()).collect(),
                lang_pair: job.config.lang_pair.clone(),
                glossary: job.config.glossary.clone(),
                style_guide: job.config.style_guide.clone(),
                context: self.build_context(&job, &batch).await,
                model_id: Some(job.config.model_id.clone()),
                temperature: Some(job.config.temperature),
                max_tokens: Some(job.config.max_tokens),
            };

            // Tradução
            let response = match self.provider.translate(req).await {
                Ok(r) => r,
                Err(e) => {
                    // Marca unidades como falhadas
                    for unit in batch {
                        if let Some(u) = job.unit_mut(&unit.id) {
                            u.fail();
                        }
                    }
                    self.repo.save_job(&job).await?;
                    return Err(AppError::Provider(e));
                }
            };

            // Aplica traduções e QA
            for trans in response.translations {
                if let Some(unit) = job.unit_mut(&trans.unit_id) {
                    unit.apply_translation(trans.translated_text)?;

                    // QA
                    if job.config.enable_qa {
                        let qa_result = self.qa_checker.check(unit, &job.config.lang_pair, job.config.glossary.as_ref());
                        if !qa_result.passed {
                            if qa_result.has_errors {
                                unit.fail();
                            } else {
                                unit.flag(qa_result.warnings.clone());
                            }
                        }
                    }

                    self.repo.save_unit(unit).await?;
                    self.events.emit(AppEvent::UnitTranslated { job_id, unit_id: trans.unit_id }).await;
                }
            }

            // Pequena pausa para não saturar
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Verifica se completou
        if job.units.values().all(|u| !matches!(u.state, UnitState::Pending)) {
            // Exporta
            // (implementar exportação aqui)
            job.complete(ExportReport {
                output_path: "".into(),
                format: OutputFormat::SameAsSource,
                units_processed: job.units.len(),
                units_with_warnings: job.units.values().filter(|u| u.state == UnitState::Flagged).count(),
                units_failed: job.units.values().filter(|u| u.state == UnitState::Failed).count(),
                output_size: 0,
                export_time_ms: 0,
                warnings: vec![],
            });
        }

        self.repo.save_job(&job).await?;
        self.events.emit(AppEvent::JobCompleted {
            job_id,
            report: job.export_report.clone().unwrap(),
        }).await;

        Ok(())
    }

    async fn build_context(&self, job: &Job, batch: &[&Unit]) -> Option<String> {
        // Resumo rolante da seção atual
        if let Some(section) = batch.first().map(|u| u.section) {
            let section_units: Vec<_> = job.units.values()
                .filter(|u| u.section == section)
                .map(|u| u.text.raw.clone())
                .take(5)
                .collect();
            if !section_units.is_empty() {
                return Some(format!("Section context: {}", section_units.join(" | ")));
            }
        }
        None
    }
}

/// QA Checker trait (implementado no domínio)
pub trait QaChecker: Send + Sync {
    fn check(&self, unit: &Unit, lang_pair: &LangPair, glossary: Option<&Glossary>) -> QaResult;
}

/// Revisa unidade manualmente
pub struct ReviewUnit<Repo: ProjectRepository> {
    repo: Arc<Repo>,
}

impl<Repo: ProjectRepository> ReviewUnit<Repo> {
    pub fn new(repo: Arc<Repo>) -> Self {
        Self { repo }
    }

    pub async fn approve(&self, unit_id: UnitId) -> Result<(), AppError> {
        let mut unit = self.repo.load_unit(&unit_id).await?
            .ok_or(AppError::Validation("Unit not found".into()))?;
        unit.mark_reviewed();
        self.repo.save_unit(&unit).await
    }

    pub async fn edit(&self, unit_id: UnitId, new_translation: MarkedText) -> Result<(), AppError> {
        let mut unit = self.repo.load_unit(&unit_id).await?
            .ok_or(AppError::Validation("Unit not found".into()))?;
        unit.apply_translation(new_translation)?;
        unit.mark_reviewed();
        self.repo.save_unit(&unit).await
    }

    pub async fn retraduze(&self, unit_id: UnitId) -> Result<(), AppError> {
        let mut unit = self.repo.load_unit(&unit_id).await?
            .ok_or(AppError::Validation("Unit not found".into()))?;
        unit.reset();
        self.repo.save_unit(&unit).await
    }
}

/// Exporta livro traduzido
pub struct ExportBook<W: FormatWriter, Repo: ProjectRepository> {
    writers: Vec<Arc<W>>,
    repo: Arc<Repo>,
}

impl<W: FormatWriter, Repo: ProjectRepository> ExportBook<W, Repo> {
    pub fn new(writers: Vec<Arc<W>>, repo: Arc<Repo>) -> Self {
        Self { writers, repo }
    }

    pub async fn execute(&self, job_id: Uuid, options: ExportOptions) -> Result<ExportReport, AppError> {
        let job = self.repo.load_job(&job_id).await?
            .ok_or(AppError::JobNotFound(job_id))?;

        // Encontra writer apropriado
        let writer = self.writers.iter()
            .find(|w| w.id() == FormatId(job.config.lang_pair.source.0.as_str())) // simplificado
            .ok_or_else(|| AppError::UnsupportedFormat("No writer for format".into()))?;

        // Constrói lookup de traduções
        let lookup = JobTranslationLookup { job: &job };

        // Exporta
        let report = writer.write(&std::path::Path::new(""), &lookup, &options, &std::path::Path::new("")).await?;

        Ok(report)
    }
}

struct JobTranslationLookup<'a> {
    job: &'a Job,
}

impl<'a> TranslationLookup for JobTranslationLookup<'a> {
    fn get(&self, unit_id: &UnitId) -> Option<MarkedText> {
        self.job.units.get(unit_id)?.translation.clone()
    }

    fn get_section(&self, section: &SectionId) -> Vec<(UnitId, MarkedText)> {
        self.job.units.iter()
            .filter(|(_, u)| u.section == *section && u.translation.is_some())
            .map(|(id, u)| (*id, u.translation.as_ref().unwrap().clone()))
            .collect()
    }
}

// Factory para criar readers/writers (registro único)
pub mod registry {
    use super::*;
    use std::sync::Arc;

    pub struct FormatRegistry {
        readers: Vec<Arc<dyn FormatReader>>,
        writers: Vec<Arc<dyn FormatWriter>>,
    }

    impl FormatRegistry {
        pub fn new() -> Self {
            Self { readers: Vec::new(), writers: Vec::new() }
        }

        pub fn register_reader(&mut self, reader: Arc<dyn FormatReader>) {
            self.readers.push(reader);
        }

        pub fn register_writer(&mut self, writer: Arc<dyn FormatWriter>) {
            self.writers.push(writer);
        }

        pub fn readers(&self) -> &[Arc<dyn FormatReader>] { &self.readers }
        pub fn writers(&self) -> &[Arc<dyn FormatWriter>] { &self.writers }

        pub fn find_reader(&self, path: &Path) -> Option<Arc<dyn FormatReader>> {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let head = std::fs::read(path).ok()?.into_iter().take(1024).collect::<Vec<_>>();

            self.readers.iter()
                .max_by_key(|r| r.sniff(name, &head))
                .cloned()
        }
    }

    impl Default for FormatRegistry {
        fn default() -> Self {
            Self::new()
        }
    }
}