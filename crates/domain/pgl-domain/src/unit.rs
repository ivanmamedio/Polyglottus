//! Unidade de tradução - a unidade fundamental do domínio

use super::{LangPair, MarkedText, SectionId, SourceRef, UnitId, UnitKind, UnitState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unidade de tradução
///
/// Representa um trecho de texto traduzível extraído de um documento,
/// com sua marcação inline protegida e metadados para rastreabilidade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    /// ID estável (hash do source_ref + texto original)
    pub id: UnitId,
    /// ID do job ao qual esta unidade pertence
    pub job_id: Uuid,
    /// Referência ao nó original no documento
    pub source_ref: SourceRef,
    /// Tipo de unidade (parágrafo, título, célula, etc.)
    pub kind: UnitKind,
    /// Texto com marcação inline protegida via placeholders
    pub text: MarkedText,
    /// Seção/capítulo a que pertence (para contexto e progresso)
    pub section: SectionId,
    /// Estado atual no pipeline
    #[serde(default)]
    pub state: UnitState,
    /// Tradução atual (se houver)
    pub translation: Option<MarkedText>,
    /// Avisos de QA da última tradução
    #[serde(default)]
    pub qa_warnings: Vec<super::QaWarning>,
    /// Número de tentativas de tradução
    #[serde(default)]
    pub retry_count: u32,
    /// Metadados adicionais (extensível)
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl Unit {
    /// Cria uma nova unidade pendente
    pub fn new(
        source_ref: SourceRef,
        kind: UnitKind,
        text: MarkedText,
        section: SectionId,
        job_id: Uuid,
) -> Self {
        // ID determinístico baseado no conteúdo (BLAKE3 128-bit)
        let hash = blake3::hash(&[source_ref.internal_path.as_bytes(), source_ref.selector.as_bytes(), text.raw.as_bytes()].concat());
        let hash_bytes: [u8; 16] = *<&[u8; 16]>::try_from(&hash.as_bytes()[..16]).unwrap();
        let id = UnitId::from_hash_bytes(&hash_bytes);

        Self {
            id,
            job_id,
            source_ref,
            kind,
            text,
            section,
            state: UnitState::Pending,
            translation: None,
            qa_warnings: Vec::new(),
            retry_count: 0,
            metadata: serde_json::Map::new(),
        }
    }

    /// Retorna o texto original (sem placeholders resolvidos)
    pub fn original_text(&self) -> &str {
        &self.text.raw
    }

    /// Retorna o texto traduzido se disponível, senão o original
    pub fn display_text(&self) -> &str {
        self.translation.as_ref().map(|t| t.raw.as_str()).unwrap_or(&self.text.raw)
    }

    /// Verifica se a unidade está traduzida (com ou sem revisão)
    pub fn is_translated(&self) -> bool {
        matches!(self.state, UnitState::Translated | UnitState::Reviewed)
    }

    /// Verifica se precisa de atenção humana
    pub fn needs_review(&self) -> bool {
        matches!(self.state, UnitState::Flagged | UnitState::Failed)
    }

    /// Aplica uma tradução (valida placeholders)
    pub fn apply_translation(&mut self, translated: MarkedText) -> Result<(), PlaceholderError> {
        // Valida se os placeholders originais estão presentes na tradução
        self.text.validate_placeholders_restored(&translated)?;

        self.translation = Some(translated);
        self.state = UnitState::Translated;
        self.qa_warnings.clear();
        Ok(())
    }

    /// Marca como revisada
    pub fn mark_reviewed(&mut self) {
        self.state = UnitState::Reviewed;
    }

    /// Marca como sinalizada por QA
    pub fn flag(&mut self, warnings: Vec<super::QaWarning>) {
        self.state = UnitState::Flagged;
        self.qa_warnings = warnings;
    }

    /// Marca como falhada
    pub fn fail(&mut self) {
        self.state = UnitState::Failed;
        self.retry_count += 1;
    }

    /// Reseta para pendente (para retradução)
    pub fn reset(&mut self) {
        self.state = UnitState::Pending;
        self.translation = None;
        self.qa_warnings.clear();
    }
}

/// Erro de validação de placeholders
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceholderError {
    #[error("Placeholder count mismatch: expected {expected}, found {found}")]
    CountMismatch { expected: usize, found: usize },

    #[error("Placeholder order mismatch at position {position}: expected {expected}, found {found}")]
    OrderMismatch { position: usize, expected: String, found: String },

    #[error("Placeholder tag mismatch for {placeholder}: expected {expected_tag}, found {found_tag}")]
    TagMismatch { placeholder: String, expected_tag: String, found_tag: String },

    #[error("Placeholder nesting violation: {0}")]
    NestingViolation(String),

    #[error("Unknown placeholder: {0}")]
    UnknownPlaceholder(String),
}

impl From<super::marked_text::PlaceholderValidationError> for PlaceholderError {
    fn from(e: super::marked_text::PlaceholderValidationError) -> Self {
        match e {
            super::marked_text::PlaceholderValidationError::CountMismatch { expected, found } => {
                PlaceholderError::CountMismatch { expected, found }
            }
            super::marked_text::PlaceholderValidationError::OrderMismatch { position, expected, found } => {
                PlaceholderError::OrderMismatch { position, expected, found }
            }
            super::marked_text::PlaceholderValidationError::NestingViolation(msg) => {
                PlaceholderError::NestingViolation(msg)
            }
            super::marked_text::PlaceholderValidationError::TagMismatch { placeholder, expected_tag, found_tag } => {
                PlaceholderError::TagMismatch { placeholder, expected_tag, found_tag }
            }
        }
    }
}
