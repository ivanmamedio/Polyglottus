//! CLI do Polyglottus

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use async_trait::async_trait;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use pgl_app::{ImportBook, SecretStore, ProjectRepository, TranslationProvider, ExportOptions, OutputFormat, ExportReport, FormatId, TranslateRequest, TranslationLookup, FormatReader, FormatError, SourceDocument};
use pgl_app::prelude::FormatRegistry;
use pgl_domain::{Language, UnitId, SectionId, MarkedText, JobState};
use pgl_fmt_epub::{EpubReader, EpubWriter};
use pgl_fmt_ooxml::{DocxReader, DocxWriter, PptxReader, PptxWriter, XlsxReader, XlsxWriter};
use pgl_fmt_odf::{OdtReader, OdtWriter, OdpReader, OdpWriter, OdsReader, OdsWriter};
use pgl_fmt_text::{TxtReader, TxtWriter, MdReader, MdWriter, SrtReader, SrtWriter, VttReader, VttWriter, PoReader, PoWriter, XliffReader, XliffWriter, TmxReader, TmxWriter, JsonI18nReader, JsonI18nWriter};
use pgl_fmt_pdf::{PdfReflowReader, PdfOverlayWriter};
use pgl_fmt_convert::{ConverterRegistry, ConvertReader, ConvertWriter, ConverterType};
use pgl_provider_openai::OpenAICompatProvider;
use pgl_provider_ollama::OllamaProvider;
use pgl_provider_embedded::EmbeddedProvider;
use pgl_testkit::FakeProvider;
use pgl_ocr::OcrEngineFactory;
use pgl_storage::SqliteRepository;
use pgl_secrets::SystemSecretStore;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// AnyFormatReader - tries multiple readers until one succeeds
struct AnyFormatReader {
    readers: Vec<Arc<dyn FormatReader>>,
}

impl AnyFormatReader {
    fn new(readers: Vec<Arc<dyn FormatReader>>) -> Self {
        Self { readers }
    }
}

#[async_trait::async_trait]
impl FormatReader for AnyFormatReader {
    fn id(&self) -> FormatId {
        FormatId("any")
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> pgl_app::Confidence {
        for reader in &self.readers {
            let conf = reader.sniff(name, head);
            if conf > pgl_app::Confidence::none() {
                return conf;
            }
        }
        pgl_app::Confidence::none()
    }

    async fn open(&self, path: &std::path::Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        for reader in &self.readers {
            if let Ok(doc) = reader.open(path).await {
                return Ok(doc);
            }
        }
        Err(FormatError::ParseError("No reader could open the file".into()))
    }
}

// Simple inline execution without EventSink trait
async fn execute_translation(
    provider: Arc<dyn TranslationProvider>,
    qa_checker: Arc<dyn pgl_domain::qa::QaChecking>,
    repo: Arc<SqliteRepository>,
    job_id: Uuid,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    let mut job = repo.load_job(&job_id).await?
        .ok_or_else(|| anyhow::anyhow!("Job not found"))?;

    if !matches!(job.state, JobState::Created | JobState::Paused) {
        return Err(anyhow::anyhow!("Job cannot be started"));
    }

    job.start();
    repo.save_job(&job, &job.project_id).await?;

    // Loop de processamento em lotes
    while job.can_resume() {
        job.update_progress();
        repo.save_job(&job, &job.project_id).await?;
        pb.set_position(job.progress.completed as u64);

        let batch = job.pending_units(job.config.batch_size);
        if batch.is_empty() { break; }

        // Collect unit IDs to avoid borrow conflicts
        let unit_ids: Vec<UnitId> = batch.iter().map(|u| u.id).collect();

        // Prepara requisição
        let req = TranslateRequest {
            units: batch.iter().map(|u| (*u).clone()).collect(),
            lang_pair: job.config.lang_pair.clone(),
            glossary: job.config.glossary.clone(),
            style_guide: job.config.style_guide.clone(),
            context: Some(String::new()),
            model_id: Some(job.config.model_id.clone()),
            temperature: Some(job.config.temperature),
            max_tokens: Some(job.config.max_tokens),
        };

        // Tradução
        let response = match provider.translate(req).await {
            Ok(r) => r,
            Err(e) => {
                for unit_id in &unit_ids {
                    if let Some(u) = job.unit_mut(unit_id) {
                        u.fail();
                    }
                }
                repo.save_job(&job, &job.project_id).await?;
                return Err(anyhow::anyhow!("Translation failed: {}", e));
            }
        };

        // Aplica traduções e QA
        let lang_pair = job.config.lang_pair.clone();
        let glossary = job.config.glossary.clone();
        let enable_qa = job.config.enable_qa;

        let job_id = job.id;
        for trans in response.translations {
            if let Some(unit) = job.unit_mut(&trans.unit_id) {
                unit.apply_translation(trans.translated_text)?;

                if enable_qa {
                    let qa_result = qa_checker.check(unit, &lang_pair, glossary.as_ref());
                    if !qa_result.passed {
                        if qa_result.has_errors {
                            unit.fail();
                        } else {
                            unit.flag(qa_result.warnings.clone());
                        }
                    }
                }

                repo.save_unit(unit, &job_id).await?;
                pb.inc(1);
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Verifica se completou
    if job.progress.completed == job.progress.total {
        job.complete(ExportReport {
            output_path: String::new(),
            format: OutputFormat::SameAsSource,
            units_processed: job.progress.completed,
            units_with_warnings: 0,
            units_failed: job.progress.failed,
            output_size: 0,
            export_time_ms: 0,
            warnings: vec![],
        });
        repo.save_job(&job, &job.project_id).await?;
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "pgl-cli")]
#[command(about = "Polyglottus - Tradutor de livros e documentos", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbosidade
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Diretório de dados
    #[arg(long, env = "POLYGLOTTUS_DATA_DIR")]
    data_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Importa livro e cria projeto
    Import {
        /// Arquivo de entrada
        #[arg(short, long)]
        input: PathBuf,

        /// Nome do projeto
        #[arg(short, long)]
        name: Option<String>,

        /// Idioma de destino
        #[arg(long, default_value = "pt-BR")]
        target_lang: String,
    },

    /// Lista projetos
    List,

    /// Mostra info do projeto
    Info {
        /// ID do projeto
        id: String,
    },

    /// Traduz projeto
    Translate {
        /// ID do projeto
        #[arg(short, long)]
        project: String,

        /// ID do job (opcional, usa o ativo)
        #[arg(long)]
        job: Option<String>,

        /// Provedor (openai, ollama, embedded)
        #[arg(long, default_value = "ollama")]
        provider: String,

        /// Modelo
        #[arg(long)]
        model: Option<String>,

        /// URL do provedor (para OpenAI-compat/Ollama)
        #[arg(long)]
        url: Option<String>,

        /// Chave de API
        #[arg(long, env = "OPENAI_API_KEY")]
        api_key: Option<String>,

        /// Caminho do modelo local (para embedded)
        #[arg(long)]
        model_path: Option<PathBuf>,

        /// Idioma de destino
        #[arg(long, default_value = "pt-BR")]
        target_lang: String,

        /// Formato de saída
        #[arg(long, default_value = "same")]
        output_format: String,

        /// Bilíngue
        #[arg(long)]
        bilingual: bool,

        /// Tamanho do lote
        #[arg(long, default_value = "10")]
        batch_size: usize,

        /// Max paralelismo
        #[arg(long, default_value = "3")]
        max_parallel: usize,
    },

    /// Exporta livro traduzido
    Export {
        /// ID do job
        #[arg(short, long)]
        job: String,

        /// Arquivo de saída
        #[arg(short, long)]
        output: PathBuf,

        /// Formato
        #[arg(long, default_value = "same")]
        format: String,

        /// Bilíngue
        #[arg(long)]
        bilingual: bool,
    },

    /// Gerencia modelos
    Models {
        #[command(subcommand)]
        action: ModelAction,
    },

    /// Configurações
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// Lista modelos instalados
    List,
    /// Recomenda modelo para hardware atual
    Recommend {
        #[arg(long, default_value = "pt-BR")]
        target_lang: String,
    },
    /// Baixa modelo
    Download {
        model_id: String,
    },
    /// Remove modelo
    Remove {
        model_id: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Define chave de API
    SetApiKey {
        provider: String,
        key: String,
    },
    /// Remove chave de API
    RemoveApiKey {
        provider: String,
    },
    /// Mostra configuração
    Show,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = match cli.verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Data dir
    let data_dir = cli.data_dir.unwrap_or_else(|| {
        PathBuf::from(".").join("polyglottus")
    });
    std::fs::create_dir_all(&data_dir)?;

    // Inicializa componentes
    let db_path = data_dir.join("polyglottus.db");
    let repo = Arc::new(SqliteRepository::new(&db_path).await?);
    let secrets = Arc::new(SystemSecretStore::new("polyglottus").await?);

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
    for converter in converter_registry.available() {
        match converter {
            "pandoc" => {
                fmt_registry.register_reader(Arc::new(ConvertReader::new(ConverterType::Pandoc)));
                fmt_registry.register_writer(Arc::new(ConvertWriter::new(ConverterType::Pandoc)));
            }
            "calibre" => {
                fmt_registry.register_reader(Arc::new(ConvertReader::new(ConverterType::Calibre)));
                fmt_registry.register_writer(Arc::new(ConvertWriter::new(ConverterType::Calibre)));
            }
            "libreoffice" => {
                fmt_registry.register_reader(Arc::new(ConvertReader::new(ConverterType::LibreOffice)));
                fmt_registry.register_writer(Arc::new(ConvertWriter::new(ConverterType::LibreOffice)));
            }
            _ => {}
        }
    }

    // OCR
    let _ocr_engine = OcrEngineFactory::create_default().ok();

    match cli.command {
        Commands::Import { input, name, target_lang: _ } => {
            let any_reader = Arc::new(AnyFormatReader::new(fmt_registry.readers().to_vec()));
            let import = ImportBook::new(vec![any_reader], repo.clone());
            let mut project = import.execute(&input, name).await?;
            // Save jobs
            for job in project.jobs.values() {
                repo.save_job(job, &project.id).await?;
            }
            info!("Project created: {} ({})", project.name, project.id);
        }

        Commands::List => {
            let projects = repo.list_projects().await?;
            for p in projects {
                println!("{} - {} ({}) [{}]", p.id, p.name, p.source_file, p.source_format);
            }
        }

        Commands::Info { id } => {
            let project_id = Uuid::parse_str(&id)?;
            if let Some(project) = repo.load_project(&project_id).await? {
                println!("Name: {}", project.name);
                println!("Source: {} ({})", project.source_file.display(), project.source_format);
                println!("Language: {:?}", project.detected_language);
                println!("Jobs: {}", project.jobs.len());
            } else {
                eprintln!("Project not found");
            }
        }

        Commands::Translate { project, job, provider, model, url, api_key, model_path, target_lang, output_format, bilingual, batch_size, max_parallel } => {
            let project_id = Uuid::parse_str(&project)?;
            let proj = repo.load_project(&project_id).await?
                .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

            let job_id = if let Some(j) = job {
                Uuid::parse_str(&j)?
            } else {
                proj.jobs.values().find(|j| matches!(j.state, JobState::Created | JobState::Running | JobState::Paused))
                    .map(|j| j.id)
                    .ok_or_else(|| anyhow::anyhow!("No active job"))?
            };

            let openai_key = if provider == "openai" {
                secrets.get("openai").await.unwrap_or(None).unwrap_or_default()
            } else {
                String::new()
            };
            let api_key_final = api_key.unwrap_or(openai_key);

            let translation_provider: Arc<dyn TranslationProvider> = match provider.as_str() {
                "fake" => {
                    // FakeProvider determinístico para testes
                    Arc::new(FakeProvider::new(pgl_testkit::FakeMode::Prefix))
                }
                "openai" => {
                    let mut config = pgl_provider_openai::OpenAICompatConfig::default();
                    config.api_key = Some(api_key_final);
                    if let Some(u) = url { config.base_url = u; }
                    let mut p = OpenAICompatProvider::new(config)?;
                    if let Some(ref m) = model { p = p.with_model(m.to_string()); }
                    Arc::new(p)
                }
                "ollama" => {
                    let mut config = pgl_provider_ollama::OllamaConfig {
                        base_url: url.unwrap_or_else(|| "http://localhost:11434".into()),
                        ..Default::default()
                    };
                    let mut p = OllamaProvider::new(config)?;
                    if let Some(ref m) = model { p = p.with_model(m.to_string()); }
                    Arc::new(p)
                }
                "embedded" => {
                    let path = model_path.ok_or_else(|| anyhow::anyhow!("model_path required for embedded provider"))?;
                    let path_str = path.to_string_lossy().to_string();
                    let mut p = EmbeddedProvider::new(pgl_provider_embedded::EmbeddedConfig::default())?
                        .with_model_path(path_str);
                    p.load_model()?;
                    Arc::new(p)
                }
                _ => anyhow::bail!("Unknown provider: {}", provider),
            };

            // Configura job
            let mut job = repo.load_job(&job_id).await?.unwrap();
            job.config.lang_pair = pgl_domain::LangPair {
                source: job.config.lang_pair.source,
                target: Language::new(target_lang),
            };
            job.config.provider = provider;
            job.config.model_id = model.unwrap_or_default();
            job.config.batch_size = batch_size;
            job.config.max_parallel = max_parallel;
            job.export_options = Some(ExportOptions {
                output_format: match output_format.as_str() {
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
            });

            // Planeja
            info!("Translation started");

            // Executa com progress bar
            let pb = ProgressBar::new(job.units.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) ETA: {eta}")
                .unwrap());

            let qa_checker: Arc<dyn pgl_domain::qa::QaChecking> = Arc::new(pgl_domain::QaChecker::new(Default::default()));
            execute_translation(translation_provider, qa_checker, repo.clone(), job_id, &pb).await?;

            pb.finish_with_message("Translation completed!");
        }

        Commands::Export { job, output, format, bilingual } => {
            let job_id = Uuid::parse_str(&job)?;
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

            // Encontra writer
            let writer = fmt_registry.writers().iter()
                .find(|w| w.id().0 == format!("{:?}", export_options.output_format) || w.id() == FormatId("any"))
                .ok_or_else(|| anyhow::anyhow!("No writer for format"))?;

            let report = writer.write(&PathBuf::new(), &CliTranslationLookup { repo: repo.clone(), job_id }, &export_options, &output).await?;
            info!("Exported to: {} ({} bytes)", report.output_path, report.output_size);
        }

        Commands::Models { action: _ } => {
            // Models commands require pgl-models crate
            eprintln!("Models commands not available (pgl-models not compiled)");
        }

        Commands::Config { action } => {
            match action {
                ConfigAction::SetApiKey { provider, key } => {
                    secrets.set(&provider, &key).await?;
                    info!("API key set for {}", provider);
                }
                ConfigAction::RemoveApiKey { provider } => {
                    secrets.delete(&provider).await?;
                    info!("API key removed for {}", provider);
                }
                ConfigAction::Show => {
                    println!("Data dir: {}", data_dir.display());
                    println!("Models dir: {}", data_dir.join("models").display());
                }
            }
        }
    }

    Ok(())
}

/// Translation lookup para CLI
struct CliTranslationLookup {
    repo: Arc<SqliteRepository>,
    job_id: Uuid,
}

impl TranslationLookup for CliTranslationLookup {
    fn get(&self, _unit_id: &UnitId) -> Option<MarkedText> {
        None
    }

    fn get_section(&self, _section: &SectionId) -> Vec<(UnitId, MarkedText)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parsing() {
    }
}
