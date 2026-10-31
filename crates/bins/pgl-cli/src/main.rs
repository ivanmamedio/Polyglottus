//! CLI do Polyglottus

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{ImportBook, PlanTranslation, ExecuteJob, ExportBook, FormatRegistry};
use pgl_domain::{Language, ExportOptions, OutputFormat, JobConfig};
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
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polyglottus")
    });
    std::fs::create_dir_all(&data_dir)?;

    // Inicializa componentes
    let db_path = data_dir.join("polyglottus.db");
    let repo = Arc::new(SqliteRepository::new(&db_path).await?);
    let secrets = Arc::new(SystemSecretStore::new("polyglottus"));
    let mut catalog = ModelCatalogImpl::new(pgl_models::CatalogConfig {
        models_dir: data_dir.join("models"),
        ..Default::default()
    })?;
    catalog.initialize().await?;

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

    match cli.command {
        Commands::Import { input, name, target_lang } => {
            let import = ImportBook::new(fmt_registry.readers().to_vec(), repo.clone());
            let project = import.execute(&input, name).await?;
            info!("Project created: {} ({})", project.name, project.id);
        }

        Commands::List => {
            let projects = repo.list_projects().await?;
            for p in projects {
                println!("{} - {} ({}) [{}]", p.id, p.name, p.source_file, p.source_format);
            }
        }

        Commands::Info { id } => {
            let project_id = uuid::Uuid::parse_str(&id)?;
            if let Some(project) = repo.load_project(&project_id).await? {
                println!("Name: {}", project.name);
                println!("Source: {} ({})", project.source_file.display(), project.source_format);
                println!("Language: {:?}", project.detected_language);
                println!("Units: {}", project.total_units());
                println!("Progress: {}%", project.overall_progress());
            } else {
                eprintln!("Project not found");
            }
        }

        Commands::Translate { project, job, provider, model, url, api_key, model_path, target_lang, output_format, bilingual, batch_size, max_parallel } => {
            let project_id = uuid::Uuid::parse_str(&project)?;
            let proj = repo.load_project(&project_id).await?
                .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

            let job_id = if let Some(j) = job {
                uuid::Uuid::parse_str(&j)?
            } else {
                proj.active_job().map(|j| j.id).ok_or_else(|| anyhow::anyhow!("No active job"))?
            };

            // Cria provedor
            let translation_provider: Arc<dyn pgl_app::TranslationProvider> = match provider.as_str() {
                "openai" => {
                    let key = api_key.unwrap_or_else(|| secrets.get("openai").await.unwrap_or(None).unwrap_or_default());
                    let mut p = OpenAICompatProvider::openai(key)?;
                    if let Some(u) = url { p.config.base_url = u; }
                    if let Some(m) = model { p.default_model = m; }
                    Arc::new(p)
                }
                "ollama" => {
                    let mut p = OllamaProvider::new(pgl_providers_ollama::OllamaConfig {
                        base_url: url.unwrap_or_else(|| "http://localhost:11434".into()),
                        ..Default::default()
                    })?;
                    if let Some(m) = model { p.default_model = m; }
                    Arc::new(p)
                }
                "embedded" => {
                    let path = model_path.ok_or_else(|| anyhow::anyhow!("model_path required for embedded provider"))?;
                    let mut p = EmbeddedProvider::new(pgl_providers_embedded::EmbeddedConfig::default())
                        .with_model_path(path);
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
            let planner = PlanTranslation::new(Arc::new(catalog));
            let plan = planner.execute(&job).await?;
            info!("Plan: {} units, ~{}s, model: {}", plan.total_units, plan.estimated_time_secs, plan.recommended_model.model_id);

            // Executa com progress bar
            let pb = ProgressBar::new(job.units.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) ETA: {eta}")
                .unwrap());

            let events = Arc::new(CliEventSink { pb: pb.clone() });
            let executor = ExecuteJob::new(translation_provider, Arc::new(pgl_domain::QaChecker::new(Default::default())), repo.clone(), events);
            executor.execute(job_id).await?;

            pb.finish_with_message("Translation completed!");
        }

        Commands::Export { job, output, format, bilingual } => {
            let job_id = uuid::Uuid::parse_str(&job)?;
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
                .find(|w| w.id() == export_options.output_format.into() || w.id() == pgl_app::FormatId("any"))
                .ok_or_else(|| anyhow::anyhow!("No writer for format"))?;

            let report = writer.write(&PathBuf::new(), &CliTranslationLookup { repo: repo.clone(), job_id }, &export_options, &output).await?;
            info!("Exported to: {} ({} bytes)", report.output_path, report.output_size);
        }

        Commands::Models { action } => {
            match action {
                ModelAction::List => {
                    let models = catalog.list_installed().await?;
                    for m in models {
                        println!("{} - {:.1} GB", m.id, m.size_gb.unwrap_or(0.0));
                    }
                }
                ModelAction::Recommend { target_lang } => {
                    let hw = catalog.detect_hardware().await?;
                    let rec = catalog.recommend_model(&hw, &pgl_domain::LangPair {
                        source: Language::new("auto"),
                        target: Language::new(target_lang),
                    }).await?;
                    println!("Recommended: {}", rec.model_id);
                    println!("Reason: {}", rec.reason);
                    println!("Speed: {}", rec.estimated_speed);
                    println!("RAM: {:.1} GB", rec.estimated_ram_gb);
                    println!("Quality: {:?}", rec.quality_tier);
                    println!("Size: {:.1} GB", rec.download_size_gb);
                    println!("License: {}", rec.license);
                }
                ModelAction::Download { model_id } => {
                    let pb = ProgressBar::new(100);
                    pb.set_style(ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
                        .unwrap());
                    catalog.download_model(&model_id, |d, t| {
                        pb.set_length(t);
                        pb.set_position(d);
                    }).await?;
                    pb.finish_with_message("Downloaded!");
                }
                ModelAction::Remove { model_id } => {
                    catalog.remove_model(&model_id).await?;
                    info!("Model removed: {}", model_id);
                }
            }
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

/// Event sink para CLI
struct CliEventSink {
    pb: ProgressBar,
}

#[async_trait::async_trait]
impl pgl_app::EventSink for CliEventSink {
    async fn emit(&self, event: pgl_app::AppEvent) {
        match event {
            pgl_app::AppEvent::JobProgress { progress, .. } => {
                self.pb.set_position(progress.completed as u64);
            }
            pgl_app::AppEvent::UnitTranslated { .. } => {
                self.pb.inc(1);
            }
            _ => {}
        }
    }
}

/// Translation lookup para CLI
struct CliTranslationLookup {
    repo: Arc<SqliteRepository>,
    job_id: uuid::Uuid,
}

impl pgl_app::TranslationLookup for CliTranslationLookup {
    fn get(&self, unit_id: &pgl_domain::UnitId) -> Option<pgl_domain::MarkedText> {
        // Simplificado - em produção buscar do repo
        None
    }

    fn get_section(&self, section: &pgl_domain::SectionId) -> Vec<(pgl_domain::UnitId, pgl_domain::MarkedText)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parsing() {
        // Teste de compilação
    }
}