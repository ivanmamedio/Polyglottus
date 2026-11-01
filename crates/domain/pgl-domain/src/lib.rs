//! Domínio puro do Polyglottus
//!
//! Este crate contém os tipos de domínio fundamentais, regras de negócio
//! e validações. **Não tem I/O, não tem async, não conhece detalhes de
//! armazenamento, rede, formatos de arquivo ou interface.**
//!
//! Tudo aqui é testável com testes unitários puros e property-based testing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod glossary;
pub mod job;
pub mod marked_text;
pub mod project;
pub mod qa;
pub mod segmentation;
pub mod style_guide;
pub mod unit;

use std::fmt;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identificador único de unidade de tradução
///
/// Estável: hash(source_ref + texto original)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitId(pub Uuid);

impl UnitId {
    /// Cria um novo ID aleatório (v7 - timestamp + random)
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Cria um ID determinístico a partir de um hash u64
    pub fn from_hash(hash: u64) -> Self {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&hash.to_le_bytes());
        Self(Uuid::from_bytes(bytes))
    }

    /// Cria um ID determinístico a partir de um hash de 16 bytes
    pub fn from_hash_bytes(hash: &[u8; 16]) -> Self {
        Self(Uuid::from_bytes(*hash))
    }
}

impl Default for UnitId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for UnitId {
    fn from(uuid: Uuid) -> Self {
        UnitId(uuid)
    }
}

impl From<UnitId> for Uuid {
    fn from(unit_id: UnitId) -> Uuid {
        unit_id.0
    }
}

impl fmt::Display for UnitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identificador de seção/capítulo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub struct SectionId(pub Uuid);

impl SectionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for SectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Referência ao nó original no documento fonte
///
/// Permite rastrear de onde veio cada unidade para escrita "patch no lugar"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRef {
    /// Caminho interno no container (ex: "OEBPS/chapter1.xhtml")
    pub internal_path: String,
    /// XPath ou seletor CSS para o nó
    pub selector: String,
    /// Offset opcional dentro do nó (para runs fragmentados em DOCX)
    pub offset: Option<usize>,
    /// Comprimento opcional do trecho original
    pub length: Option<usize>,
}

/// Tipo de unidade de tradução
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    /// Título (com nível)
    Heading(u8),
    /// Parágrafo normal
    Paragraph,
    /// Item de lista
    ListItem,
    /// Célula de tabela
    TableCell,
    /// Legenda (figura, tabela, etc.)
    Caption,
    /// Texto alternativo de imagem
    AltText,
    /// Nota de rodapé
    Footnote,
    /// Legenda de vídeo/áudio
    Subtitle,
    /// Metadados (título do livro, autor, etc.)
    Metadata,
    /// Texto em SVG
    SvgText,
    /// Texto em MathML
    MathMl,
    /// Código/preformatado
    CodeBlock,
    /// Desconhecido/outro
    Other,
}

/// Par de idiomas (origem → destino)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LangPair {
    pub source: Language,
    pub target: Language,
}

/// Código de idioma (BCP 47)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Language(pub String);

impl Language {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into().to_lowercase())
    }

    pub fn code(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Language {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Estado de uma unidade no pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    /// Aguardando tradução
    Pending,
    /// Traduzida (pode precisar de revisão)
    Translated,
    /// Sinalizada por QA (precisa atenção)
    Flagged,
    /// Revisada e aprovada
    Reviewed,
    /// Falhou permanentemente
    Failed,
    /// Ignorada (não traduzível)
    Skipped,
}

impl Default for UnitState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Relatório de fidelidade de formato
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityReport {
    /// Nível geral (1-5)
    pub level: u8,
    /// O que é preservado
    pub preserved: Vec<String>,
    /// O que NÃO é preservado
    pub not_preserved: Vec<String>,
    /// Avisos específicos
    pub warnings: Vec<String>,
}

impl FidelityReport {
    pub fn new(level: u8) -> Self {
        Self {
            level: level.clamp(1, 5),
            preserved: Vec::new(),
            not_preserved: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_preserved(mut self, item: impl Into<String>) -> Self {
        self.preserved.push(item.into());
        self
    }

    pub fn with_not_preserved(mut self, item: impl Into<String>) -> Self {
        self.not_preserved.push(item.into());
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Configuração de exportação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Formato de saída desejado
    pub output_format: OutputFormat,
    /// Gerar versão bilíngue (original + tradução)
    pub bilingual: bool,
    /// Idioma da interface para metadados
    pub ui_language: Language,
    /// Preservar formatação original ao máximo
    pub preserve_formatting: bool,
    /// Incluir relatório de qualidade
    pub include_qa_report: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::SameAsSource,
            bilingual: false,
            ui_language: Language::new("pt-BR"),
            preserve_formatting: true,
            include_qa_report: true,
        }
    }
}

/// Formato de saída
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Mesmo formato do arquivo original
    SameAsSource,
    EPUB,
    DOCX,
    HTML,
    TXT,
    MD,
    PDF,
    SRT,
    VTT,
    XLIFF,
    TMX,
    PO,
    JSON_I18N,
}

/// Relatório de exportação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportReport {
    /// Caminho do arquivo gerado
    pub output_path: String,
    /// Formato real gerado
    pub format: OutputFormat,
    /// Número de unidades processadas
    pub units_processed: usize,
    /// Unidades com avisos
    pub units_with_warnings: usize,
    /// Unidades falhadas
    pub units_failed: usize,
    /// Tamanho do arquivo de saída (bytes)
    pub output_size: u64,
    /// Tempo de exportação (ms)
    pub export_time_ms: u64,
    /// Avisos
    pub warnings: Vec<String>,
}

/// Capacidades de um provedor de tradução
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Contexto máximo em tokens
    pub max_context_tokens: usize,
    /// Idiomas suportados (códigos BCP 47)
    pub supported_languages: Vec<Language>,
    /// É local (roda na máquina)?
    pub is_local: bool,
    /// Suporta streaming de resposta?
    pub supports_streaming: bool,
    /// Suporta glossário/guia de estilo no prompt?
    pub supports_glossary: bool,
    /// Modelo padrão recomendado
    pub default_model: Option<String>,
}

/// Requisição de tradução
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslateRequest {
    /// Unidades a traduzir
    pub units: Vec<Unit>,
    /// Par de idiomas
    pub lang_pair: LangPair,
    /// Glossário opcional
    pub glossary: Option<Glossary>,
    /// Guia de estilo opcional
    pub style_guide: Option<StyleGuide>,
    /// Contexto adicional (resumo do capítulo, etc.)
    pub context: Option<String>,
    /// ID do modelo a usar
    pub model_id: Option<String>,
    /// Temperatura (0.0 - 1.0)
    pub temperature: Option<f32>,
    /// Max tokens de resposta
    pub max_tokens: Option<usize>,
}

/// Resposta de tradução
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslateResponse {
    /// Traduções por ID de unidade
    pub translations: Vec<UnitTranslation>,
    /// Tokens usados (estimativa)
    pub tokens_used: Option<TokenUsage>,
    /// Tempo de resposta (ms)
    pub latency_ms: u64,
    /// Modelo efetivamente usado
    pub model_used: String,
}

/// Tradução de uma unidade
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitTranslation {
    pub unit_id: UnitId,
    pub translated_text: MarkedText,
    /// Confiança (0.0 - 1.0) se disponível
    pub confidence: Option<f32>,
    /// Avisos de QA automático
    pub qa_warnings: Vec<QaWarning>,
}

/// Uso de tokens
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Aviso de QA
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaWarning {
    pub code: QaWarningCode,
    pub message: String,
    pub severity: QaSeverity,
}

/// Códigos de aviso de QA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QaWarningCode {
    /// Idioma da saída não confere
    WrongLanguage,
    /// Razão de tamanho fora do esperado
    LengthRatio,
    /// Placeholders faltando ou em ordem errada
    PlaceholderMismatch,
    /// Repetição anormal (possível alucinação)
    Repetition,
    /// Termo do glossário não respeitado
    GlossaryViolation,
    /// Número/data/nome próprio alterado
    EntityChanged,
    /// Formatação perdida
    FormattingLost,
    /// Texto vazio ou muito curto
    EmptyOutput,
    /// Outro
    Other,
}

/// Severidade do aviso
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QaSeverity {
    Info,
    Warning,
    Error,
}

/// Re-export dos submódulos principais
pub use glossary::{Glossary, GlossaryEntry};
pub use job::{Job, JobConfig, JobProgress, JobState};
pub use marked_text::{MarkedText, MarkError};
pub use project::Project;
pub use qa::{QaChecker as QaCheck, QaConfig, QaResult};
pub use segmentation::{Segment, SegmentationRule};
pub use style_guide::StyleGuide;
pub use unit::Unit;

/// Prelude para imports convenientes
pub mod prelude {
    pub use crate::{
        Capabilities, ExportOptions, ExportReport, FidelityReport, LangPair, Language,
        OutputFormat, QaSeverity, QaWarning, QaWarningCode, SectionId, SourceRef,
        StyleGuide, TokenUsage, TranslateRequest, TranslateResponse, Unit, UnitId,
        UnitKind, UnitState, UnitTranslation,
    };
}