//! Polyglottus Desktop - Tauri App

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{ImportBook, PlanTranslation, ExecuteJob, ExportBook, FormatRegistry, EventSink, AppEvent};
use pgl_domain::{Language, ExportOptions, OutputFormat, JobConfig, UnitId, UnitState, QaWarning};
use pgl_fmt_epub::{EpubReader, EpubWriter};
use pgl_fmt_ooxml::{DocxReader, DocxWriter, PptxReader, PptxWriter, XlsxReader, XlsxWriter};
use pgl_fmt_odf::{OdtReader, OdtWriter, OdpReader, OdpWriter, OdsReader, OdsWriter};
use pgl_fmt_text::{TxtReader, TxtWriter, MdReader, MdWriter, SrtReader, SrtWriter, VttReader, VttWriter, HtmlTextReader, HtmlTextWriter, PoReader, PoWriter, XliffReader, XliffWriter, TmxReader, TmxWriter, JsonI18nReader, JsonI18nWriter};
use pgl_fmt_pdf::{PdfReflowReader, PdfOverlayWriter};
use pgl_fmt_convert::{ConverterRegistry, ConvertReader, ConvertWriter};
use pgl_providers_openai::OpenAICompatProvider;
use pgl_providers_ollama::OllamaProvider;
use pgl_providers_embedded::EmbeddedProvider;
use pgl_ocr::OcrEngineFactory;
use pgl_storage::SqliteRepository;
use pgl_secrets::SystemSecretStore;
use pgl_models::ModelCatalogImpl;
use tauri::{Manager, State, Window, Emitter};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};
use specta::Type;
use ts_rs::TS;

// ============================================================================
// Tipos para IPC (serialização automática via specta/ts-rs)
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub source_file: String,
    pub source_format: String,
    pub created_at: String,
    pub updated_at: String,
    pub overall_progress: u8,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct JobSummary {
    pub id: String,
    pub project_id: String,
    pub target_language: String,
    pub state: String,
    pub progress: JobProgressDto,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct JobProgressDto {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub flagged: usize,
    pub pending: usize,
    pub percent: u8,
    pub elapsed_secs: u64,
    pub eta_secs: Option<u64>,
    pub units_per_second: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct TranslationPlanDto {
    pub total_units: usize,
    pub total_chars: usize,
    pub estimated_tokens: usize,
    pub estimated_time_secs: u64,
    pub recommended_model: ModelRecommendationDto,
    pub hardware_profile: HardwareProfileDto,
    pub warnings: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct ModelRecommendationDto {
    pub model_id: String,
    pub reason: String,
    pub estimated_speed: String,
    pub estimated_ram_gb: f32,
    pub quality_tier: String,
    pub download_size_gb: f32,
    pub license: String,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct HardwareProfileDto {
    pub cpu_cores: usize,
    pub ram_gb: f32,
    pub gpu: Option<GpuInfoDto>,
    pub platform: String,
    pub arch: String,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct GpuInfoDto {
    pub name: String,
    pub vram_gb: f32,
    pub cuda: bool,
    pub metal: bool,
    pub vulkan: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct ModelInfoDto {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub max_context: usize,
    pub is_local: bool,
    pub size_gb: Option<f32>,
    pub license: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct UnitDto {
    pub id: String,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub kind: String,
    pub state: String,
    pub section: String,
    pub qa_warnings: Vec<QaWarningDto>,
}

#[derive(serde::Serialize, serde::Deserialize, Type, TS, Clone, Debug)]
#[ts(export, export_to = "../ui/src/bindings/")]
pub struct QaWarningDto {
    pub code: String,
    pub message: String,
    pub severity: String,
}

// ============================================================================
// Estado global da aplicação
// ============================================================================

struct AppState {
    repo: Arc<SqliteRepository>,
    secrets: Arc<SystemSecretStore>,
    catalog: Arc<Mutex<ModelCatalogImpl>>,
    fmt_registry: Arc<FormatRegistry>,
    ocr_engine: Option<Arc<dyn pgl_app::OcrEngine>>,
}

impl AppState {
    async fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let db_path = data_dir.join("polyglottus.db");
        let repo = Arc::new(SqliteRepository::new(&db_path).await?);
        let secrets = Arc::new(SystemSecretStore::new("polyglottus").await?);

        let mut catalog = ModelCatalogImpl::new(pgl_models::CatalogConfig {
            models_dir: data_dir.join("models"),
            ..Default::default()
        })?;
        catalog.initialize().await?;
        let catalog = Arc::new(Mutex::new(catalog));

        // Registry de formatos
        let mut fmt_registry = FormatRegistry::new();
        fmt_registry.register_reader(Arc::new(EpubReader::new()));
        fmt_registry.register_writer(Arc::new(EpubWriter::new()));
        fmt_registry.register_reader(Arc::new(DocxReader::new()));
        fmt_registry.register_writer(Arc::new(DocxWriter::new()));
        fmt_registry.register_reader(Arc::new(PptxReader::new()));
        fmt_registry.register_writer(Arc::new(PptxWriter::new()));
        fmt_registry.register_reader(Arc::new(XlsxReader::new()));
        fmt_registry.register_writer(Arc::new(XlsxWriter::new()));
        fmt_registry.register_reader(Arc::new(OdtReader::new()));
        fmt_registry.register_writer(Arc::new(OdtWriter::new()));
        fmt_registry.register_reader(Arc::new(OdpReader::new()));
        fmt_registry.register_writer(Arc::new(OdpWriter::new()));
        fmt_registry.register_reader(Arc::new(OdsReader::new()));
        fmt_registry.register_writer(Arc::new(OdsWriter::new()));
        fmt_registry.register_reader(Arc::new(TxtReader::new()));
        fmt_registry.register_writer(Arc::new(TxtWriter::new()));
        fmt_registry.register_reader(Arc::new(MdReader::new()));
        fmt_registry.register_writer(Arc::new(MdWriter::new()));
        fmt_registry.register_reader(Arc::new(SrtReader::new()));
        fmt_registry.register_writer(Arc::new(SrtWriter::new()));
        fmt_registry.register_reader(Arc::new(VttReader::new()));
        fmt_registry.register_writer(Arc::new(VttWriter::new()));
        fmt_registry.register_reader(Arc::new(HtmlTextReader::new()));
        fmt_registry.register_writer(Arc::new(HtmlTextWriter::new()));
        fmt_registry.register_reader(Arc::new(PoReader::new()));
        fmt_registry.register_writer(Arc::new(PoWriter::new()));
        fmt_registry.register_reader(Arc::new(XliffReader::new()));
        fmt_registry.register_writer(Arc::new(XliffWriter::new()));
        fmt_registry.register_reader(Arc::new(TmxReader::new()));
        fmt_registry.register_writer(Arc::new(TmxWriter::new()));
        fmt_registry.register_reader(Arc::new(JsonI18nReader::new()));
        fmt_registry.register_writer(Arc::new(JsonI18nWriter::new()));
        fmt_registry.register_reader(Arc::new(PdfReflowReader::new()));
        fmt_registry.register_writer(Arc::new(PdfOverlayWriter::new()));

        // Conversores
        let converter_registry = ConverterRegistry::default();
        for converter in converter_registry.converters {
            fmt_registry.register_reader(Arc::new(ConvertReader::new(converter.clone())));
            fmt_registry.register_writer(Arc::new(ConvertWriter::new(converter)));
        }

        // OCR
        let ocr_engine = OcrEngineFactory::create_default().ok();

        Ok(Self {
            repo,
            secrets,
            catalog,
            fmt_registry: Arc::new(fmt_registry),
            ocr_engine,
        })
    }
}

// ============================================================================
// Tauri Event Sink
// ============================================================================

struct TauriEventSink {
    window: Window,
}

#[async_trait::async_trait]
impl EventSink for TauriEventSink {
    async fn emit(&self, event: AppEvent) {
        let _ = self.window.emit("app-event", event);
    }
}

// ============================================================================
// Comandos Tauri
// ============================================================================

#[tauri::command]
async fn import_book(
    state: State<'_, AppState>,
    window: Window,
    file_path: String,
    project_name: Option<String>,
) -> Result<ProjectSummary, String> {
    let import = ImportBook::new(state.fmt_registry.readers().to_vec(), state.repo.clone());
    let project = import.execute(&PathBuf::from(file_path), project_name).await
        .map_err(|e| e.to_string())?;

    // Emite evento
    window.emit("project-created", &project).ok();

    Ok(project.into())
}

#[tauri::command]
async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, String> {
    let projects = state.repo.list_projects().await.map_err(|e| e.to_string())?;
    Ok(projects.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn get_project(state: State<'_, AppState>, id: String) -> Result<Option<ProjectSummary>, String> {
    let project_id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let project = state.repo.load_project(&project_id).await.map_err(|e| e.to_string())?;
    Ok(project.map(Into::into))
}

#[tauri::command]
async fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let project_id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.repo.delete_project(&project_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_jobs(state: State<'_, AppState>, project_id: String) -> Result<Vec<JobSummary>, String> {
    let pid = uuid::Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let jobs = state.repo.list_jobs(&pid).await.map_err(|e| e.to_string())?;
    Ok(jobs.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn plan_translation(
    state: State<'_, AppState>,
    project_id: String,
    job_id: Option<String>,
) -> Result<TranslationPlanDto, String> {
    let pid = uuid::Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let project = state.repo.load_project(&pid).await.map_err(|e| e.to_string())?
        .ok_or("Project not found")?;

    let jid = if let Some(j) = job_id {
        uuid::Uuid::parse_str(&j).map_err(|e| e.to_string())?
    } else {
        project.active_job().map(|j| j.id).ok_or("No active job")?
    };

    let job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?
        .ok_or("Job not found")?;

    let catalog = state.catalog.lock().await;
    let planner = PlanTranslation::new(catalog.clone());
    let plan = planner.execute(&job).await.map_err(|e| e.to_string())?;

    Ok(plan.into())
}

#[tauri::command]
async fn start_translation(
    state: State<'_, AppState>,
    window: Window,
    job_id: String,
    provider: String,
    model: Option<String>,
    url: Option<String>,
    batch_size: Option<usize>,
    max_parallel: Option<usize>,
) -> Result<(), String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let mut job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?
        .ok_or("Job not found")?;

    if let Some(bs) = batch_size { job.config.batch_size = bs; }
    if let Some(mp) = max_parallel { job.config.max_parallel = mp; }

    // Cria provedor
    let translation_provider: Arc<dyn pgl_app::TranslationProvider> = match provider.as_str() {
        "openai" => {
            let key = state.secrets.get("openai").await.unwrap_or(None).unwrap_or_default();
            let mut p = OpenAICompatProvider::openai(key).map_err(|e| e.to_string())?;
            if let Some(u) = url { p.config.base_url = u; }
            if let Some(m) = model { p.default_model = m; }
            Arc::new(p)
        }
        "ollama" => {
            let mut p = OllamaProvider::new(pgl_providers_ollama::OllamaConfig {
                base_url: url.unwrap_or_else(|| "http://localhost:11434".into()),
                ..Default::default()
            }).map_err(|e| e.to_string())?;
            if let Some(m) = model { p.default_model = m; }
            Arc::new(p)
        }
        "embedded" => {
            let catalog = state.catalog.lock().await;
            let model_path = catalog.get_model_path(&model.unwrap_or_default()).await.map_err(|e| e.to_string())?.ok_or("Model not downloaded")?;
            let mut p = EmbeddedProvider::new(pgl_providers_embedded::EmbeddedConfig::default())
                .with_model_path(model_path);
            p.load_model().map_err(|e| e.to_string())?;
            Arc::new(p)
        }
        _ => return Err(format!("Unknown provider: {}", provider)),
    };

    // Atualiza job
    state.repo.save_job(&job).await.map_err(|e| e.to_string())?;

    // Executa em background
    let repo = state.repo.clone();
    let events = Arc::new(TauriEventSink { window: window.clone() });
    let qa_checker = Arc::new(pgl_domain::QaChecker::new(Default::default()));

    tauri::async_runtime::spawn(async move {
        let executor = ExecuteJob::new(translation_provider, qa_checker, repo, events);
        if let Err(e) = executor.execute(jid).await {
            error!("Translation failed: {}", e);
            let _ = window.emit("app-event", AppEvent::JobFailed { job_id: jid, error: e.to_string() });
        }
    });

    Ok(())
}

#[tauri::command]
async fn pause_translation(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let mut job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?.ok_or("Job not found")?;
    job.pause();
    state.repo.save_job(&job).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_translation(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let mut job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?.ok_or("Job not found")?;
    job.resume();
    state.repo.save_job(&job).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_translation(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let mut job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?.ok_or("Job not found")?;
    job.cancel();
    state.repo.save_job(&job).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_book(
    state: State<'_, AppState>,
    job_id: String,
    output_path: String,
    format: String,
    bilingual: bool,
) -> Result<pgl_domain::ExportReport, String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let job = state.repo.load_job(&jid).await.map_err(|e| e.to_string())?.ok_or("Job not found")?;

    let export_options = ExportOptions {
        output_format: match format.as_str() {
            "same" => OutputFormat::SameAsSource,
            "epub" => OutputFormat::EPUB,
            "docx" => OutputFormat::DOCX,
            "html" => OutputFormat::HTML,
            "txt" => OutputFormat::TXT,
            "md" => OutputFormat::MD,
            "pdf" => OutputFormat::PDF,
            _ => OutputFormat::SameAsSource,
        },
        bilingual,
        ..Default::default()
    };

    let writer = state.fmt_registry.writers().iter()
        .find(|w| w.id() == export_options.output_format.into() || w.id() == pgl_app::FormatId("any"))
        .ok_or("No writer for format")?;

    let lookup = JobTranslationLookup { repo: state.repo.clone(), job_id: jid };
    writer.write(&PathBuf::new(), &lookup, &export_options, &PathBuf::from(output_path)).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_units(state: State<'_, AppState>, job_id: String, states: Vec<String>) -> Result<Vec<UnitDto>, String> {
    let jid = uuid::Uuid::parse_str(&job_id).map_err(|e| e.to_string())?;
    let unit_states: Vec<UnitState> = states.iter().filter_map(|s| serde_json::from_str(s).ok()).collect();
    let units = state.repo.load_units(&jid, &unit_states).await.map_err(|e| e.to_string())?;
    Ok(units.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn update_unit(
    state: State<'_, AppState>,
    unit_id: String,
    translation: String,
    action: String, // "approve", "edit", "retranslate"
) -> Result<(), String> {
    let uid = UnitId(uuid::Uuid::parse_str(&unit_id).map_err(|e| e.to_string())?);
    let mut unit = state.repo.load_unit(&uid).await.map_err(|e| e.to_string())?.ok_or("Unit not found")?;

    match action.as_str() {
        "approve" => unit.mark_reviewed(),
        "edit" => {
            unit.apply_translation(pgl_domain::MarkedText::new(translation)).map_err(|e| e.to_string())?;
            unit.mark_reviewed();
        }
        "retranslate" => unit.reset(),
        _ => return Err("Invalid action".into()),
    }

    state.repo.save_unit(&unit).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfoDto>, String> {
    let catalog = state.catalog.lock().await;
    let models = catalog.list_installed().await.map_err(|e| e.to_string())?;
    Ok(models.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn recommend_model(state: State<'_, AppState>, target_lang: String) -> Result<ModelRecommendationDto, String> {
    let catalog = state.catalog.lock().await;
    let hw = catalog.detect_hardware().await.map_err(|e| e.to_string())?;
    let rec = catalog.recommend_model(&hw, &pgl_domain::LangPair {
        source: Language::new("auto"),
        target: Language::new(target_lang),
    }).await.map_err(|e| e.to_string())?;
    Ok(rec.into())
}

#[tauri::command]
async fn download_model(state: State<'_, AppState>, window: Window, model_id: String) -> Result<(), String> {
    let catalog = state.catalog.lock().await;
    catalog.download_model(&model_id, |downloaded, total| {
        let _ = window.emit("model-download-progress", serde_json::json!({
            "model_id": model_id,
            "downloaded": downloaded,
            "total": total,
            "percent": if total > 0 { (downloaded as f64 / total as f64 * 100.0) as u8 } else { 0 }
        }));
    }).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_model(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    let catalog = state.catalog.lock().await;
    catalog.remove_model(&model_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_api_key(state: State<'_, AppState>, provider: String, key: String) -> Result<(), String> {
    state.secrets.set(&provider, &key).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    state.secrets.delete(&provider).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "data_dir": state.repo.pool().connect_options().get_filename().unwrap_or_default(),
    }))
}

// ============================================================================
// Conversões de tipos
// ============================================================================

impl From<pgl_app::ProjectSummary> for ProjectSummary {
    fn from(p: pgl_app::ProjectSummary) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            source_file: p.source_file,
            source_format: p.source_format,
            created_at: p.created_at,
            updated_at: p.updated_at,
            overall_progress: p.overall_progress,
        }
    }
}

impl From<pgl_app::JobSummary> for JobSummary {
    fn from(j: pgl_app::JobSummary) -> Self {
        Self {
            id: j.id.to_string(),
            project_id: j.project_id.to_string(),
            target_language: j.target_language.to_string(),
            state: format!("{:?}", j.state),
            progress: j.progress.into(),
            created_at: j.created_at,
            updated_at: j.updated_at,
        }
    }
}

impl From<pgl_app::JobProgress> for JobProgressDto {
    fn from(p: pgl_app::JobProgress) -> Self {
        Self {
            total: p.total,
            completed: p.completed,
            failed: p.failed,
            flagged: p.flagged,
            pending: p.pending,
            percent: p.percent,
            elapsed_secs: p.elapsed.as_secs(),
            eta_secs: p.eta.map(|d| d.as_secs()),
            units_per_second: p.units_per_second,
        }
    }
}

impl From<pgl_app::TranslationPlan> for TranslationPlanDto {
    fn from(p: pgl_app::TranslationPlan) -> Self {
        Self {
            total_units: p.total_units,
            total_chars: p.total_chars,
            estimated_tokens: p.estimated_tokens,
            estimated_time_secs: p.estimated_time_secs,
            recommended_model: p.recommended_model.into(),
            hardware_profile: p.hardware_profile.into(),
            warnings: p.warnings,
        }
    }
}

impl From<pgl_app::ModelRecommendation> for ModelRecommendationDto {
    fn from(r: pgl_app::ModelRecommendation) -> Self {
        Self {
            model_id: r.model_id,
            reason: r.reason,
            estimated_speed: r.estimated_speed,
            estimated_ram_gb: r.estimated_ram_gb,
            quality_tier: format!("{:?}", r.quality_tier),
            download_size_gb: r.download_size_gb,
            license: r.license,
        }
    }
}

impl From<pgl_app::HardwareProfile> for HardwareProfileDto {
    fn from(h: pgl_app::HardwareProfile) -> Self {
        Self {
            cpu_cores: h.cpu_cores,
            ram_gb: h.ram_gb,
            gpu: h.gpu.map(Into::into),
            platform: h.platform,
            arch: h.arch,
        }
    }
}

impl From<pgl_app::GpuInfo> for GpuInfoDto {
    fn from(g: pgl_app::GpuInfo) -> Self {
        Self {
            name: g.name,
            vram_gb: g.vram_gb,
            cuda: g.cuda,
            metal: g.metal,
            vulkan: g.vulkan,
        }
    }
}

impl From<pgl_app::ModelInfo> for ModelInfoDto {
    fn from(m: pgl_app::ModelInfo) -> Self {
        Self {
            id: m.id,
            name: m.name,
            languages: m.languages.into_iter().map(|l| l.to_string()).collect(),
            max_context: m.max_context,
            is_local: m.is_local,
            size_gb: m.size_gb,
            license: m.license,
        }
    }
}

impl From<pgl_domain::Unit> for UnitDto {
    fn from(u: pgl_domain::Unit) -> Self {
        Self {
            id: u.id.to_string(),
            source_text: u.text.raw,
            translated_text: u.translation.map(|t| t.raw),
            kind: format!("{:?}", u.kind),
            state: format!("{:?}", u.state),
            section: u.section.to_string(),
            qa_warnings: u.qa_warnings.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pgl_domain::QaWarning> for QaWarningDto {
    fn from(w: pgl_domain::QaWarning) -> Self {
        Self {
            code: format!("{:?}", w.code),
            message: w.message,
            severity: format!("{:?}", w.severity),
        }
    }
}

// ============================================================================
// Translation Lookup para exportação
// ============================================================================

struct JobTranslationLookup {
    repo: Arc<SqliteRepository>,
    job_id: uuid::Uuid,
}

impl pgl_app::TranslationLookup for JobTranslationLookup {
    fn get(&self, unit_id: &UnitId) -> Option<pgl_domain::MarkedText> {
        // Em produção, buscar do repo
        None
    }

    fn get_section(&self, section: &pgl_domain::SectionId) -> Vec<(UnitId, pgl_domain::MarkedText)> {
        Vec::new()
    }
}

// ============================================================================
// Entry point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            let state = tauri::async_runtime::block_on(async {
                AppState::new(data_dir).await
            }).expect("Failed to initialize app state");

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_book,
            list_projects,
            get_project,
            delete_project,
            list_jobs,
            plan_translation,
            start_translation,
            pause_translation,
            resume_translation,
            cancel_translation,
            export_book,
            get_units,
            update_unit,
            list_models,
            recommend_model,
            download_model,
            remove_model,
            set_api_key,
            remove_api_key,
            get_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}