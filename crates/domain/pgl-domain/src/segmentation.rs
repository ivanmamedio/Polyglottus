//! Segmentação de texto para tradução com contexto

use super::{MarkedText, Unit, UnitId, UnitKind, SectionId, SourceRef, unit};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Regras de segmentação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentationRule {
    /// Padrão regex para divisão (ex: fim de frase)
    pub pattern: String,
    /// Manter o delimitador no segmento anterior
    #[serde(default = "default_true")]
    pub keep_delimiter: bool,
    /// Tamanho máximo do segmento (caracteres)
    #[serde(default = "default_max_segment")]
    pub max_segment_size: usize,
    /// Tamanho mínimo do segmento
    #[serde(default = "default_min_segment")]
    pub min_segment_size: usize,
}

fn default_true() -> bool { true }
fn default_max_segment() -> usize { 2000 }
fn default_min_segment() -> usize { 10 }

impl Default for SegmentationRule {
    fn default() -> Self {
        Self {
            pattern: r"(?<=[.!?])\s+".into(), // Divide após . ! ? seguido de espaço
            keep_delimiter: true,
            max_segment_size: 2000,
            min_segment_size: 10,
        }
    }
}

/// Segmento de texto para tradução
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub id: UnitId,
    pub text: MarkedText,
    pub context_before: Option<String>, // Resumo do segmento anterior
    pub context_after: Option<String>,  // Preview do próximo
    pub section: SectionId,
    pub position: usize, // Ordem no documento
}

/// Segmentador de texto
pub struct Segmenter {
    rules: Vec<SegmentationRule>,
    /// Tamanho da janela de contexto (segmentos antes/depois)
    context_window: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Self {
            rules: vec![SegmentationRule::default()],
            context_window: 2,
        }
    }
}

impl Segmenter {
    pub fn new(rules: Vec<SegmentationRule>) -> Self {
        Self {
            rules,
            context_window: 2,
        }
    }

    pub fn with_context_window(mut self, window: usize) -> Self {
        self.context_window = window;
        self
    }

    /// Segmenta uma unidade grande em segmentos menores para tradução
    pub fn segment_unit(&self, unit: &Unit) -> Vec<Segment> {
        let text = &unit.text.raw;

        // Se já é pequeno o suficiente, retorna como um segmento
        if text.len() <= self.rules[0].max_segment_size {
            return vec![Segment {
                id: unit.id,
                text: unit.text.clone(),
                context_before: None,
                context_after: None,
                section: unit.section,
                position: 0,
            }];
        }

        // Divide usando a primeira regra
        let rule = &self.rules[0];
        let mut segments = Vec::new();
        let mut start = 0;
        let mut position = 0;

        // Regex simples para divisão (evita dependência pesada)
        let parts = self.simple_split(text, &rule.pattern);

        for part in parts {
            if part.trim().is_empty() {
                continue;
            }

            let marked = MarkedText::new(part);
            segments.push(Segment {
                id: UnitId::from_hash({
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    unit.id.hash(&mut h);
                    position.hash(&mut h);
                    h.finish()
                }),
                text: marked,
                context_before: None,
                context_after: None,
                section: unit.section,
                position,
            });
            position += 1;
        }

        // Adiciona contexto
        for i in 0..segments.len() {
            // Contexto antes: últimos N segmentos
            if i > 0 {
                let start_idx = i.saturating_sub(self.context_window);
                let context: Vec<_> = segments[start_idx..i]
                    .iter()
                    .map(|s| s.text.raw.clone())
                    .collect();
                segments[i].context_before = Some(context.join(" | "));
            }

            // Contexto depois: próximos N segmentos
            if i + 1 < segments.len() {
                let end_idx = std::cmp::min(i + 1 + self.context_window, segments.len());
                let context: Vec<_> = segments[i + 1..end_idx]
                    .iter()
                    .map(|s| s.text.raw.clone())
                    .collect();
                segments[i].context_after = Some(context.join(" | "));
            }
        }

        segments
    }

    /// Recombina segmentos traduzidos de volta em uma unidade
    pub fn recombine(&self, original: &Unit, translated_segments: &[Segment]) -> Result<Unit, RecombineError> {
        if translated_segments.is_empty() {
            return Err(RecombineError::EmptySegments);
        }

        // Valida que todos os segmentos pertencem à mesma unidade original
        for seg in translated_segments {
            if seg.section != original.section {
                return Err(RecombineError::SectionMismatch);
            }
        }

        // Concatena textos traduzidos
        let combined_text = translated_segments
            .iter()
            .map(|s| s.text.raw.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let mut result = original.clone();
        let marked = MarkedText::new(combined_text);
        result.apply_translation(marked)?;

        Ok(result)
    }

    /// Segmentação inteligente preservando parágrafos
    pub fn segment_paragraphs(&self, text: &str) -> Vec<String> {
        text.split("\n\n")
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn simple_split(&self, text: &str, pattern: &str) -> Vec<String> {
        // Implementação simples sem regex pesada
        // Divide por . ! ? seguido de espaço ou fim de linha
        let mut result = Vec::new();
        let mut current = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            current.push(c);

            if matches!(c, '.' | '!' | '?') {
                // Peek próximo
                let mut next_is_space = false;
                if let Some(&next_c) = chars.peek() {
                    if next_c.is_whitespace() {
                        next_is_space = true;
                        chars.next(); // consome o espaço
                    }
                }

                if next_is_space || chars.peek().is_none() {
                    result.push(current.trim().to_string());
                    current.clear();
                }
            }
        }

        if !current.trim().is_empty() {
            result.push(current.trim().to_string());
        }

        if result.is_empty() {
            vec![text.to_string()]
        } else {
            result
        }
    }
}

/// Erros de recombinação
#[derive(Debug, thiserror::Error)]
pub enum RecombineError {
    #[error("No segments to recombine")]
    EmptySegments,
    #[error("Segments belong to different sections")]
    SectionMismatch,
    #[error("Placeholder validation failed: {0}")]
    PlaceholderError(#[from] unit::PlaceholderError),
}

/// Rolling summary para contexto em livros longos
pub struct RollingSummary {
    window: VecDeque<String>,
    max_chars: usize,
}

impl RollingSummary {
    pub fn new(max_chars: usize) -> Self {
        Self {
            window: VecDeque::new(),
            max_chars,
        }
    }

    pub fn add(&mut self, text: &str) {
        self.window.push_back(text.to_string());
        self.trim();
    }

    pub fn summary(&self) -> String {
        self.window.iter().cloned().collect::<Vec<_>>().join(" ... ")
    }

    fn trim(&mut self) {
        let mut total: usize = self.window.iter().map(|s| s.len()).sum();
        while total > self.max_chars && !self.window.is_empty() {
            if let Some(removed) = self.window.pop_front() {
                total -= removed.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Unit, UnitKind, MarkedText, SourceRef, SectionId};

    fn make_unit(text: &str) -> Unit {
        Unit::new(
            SourceRef { internal_path: "test".into(), selector: "1".into(), offset: None, length: None },
            UnitKind::Paragraph,
            MarkedText::new(text),
            SectionId::new()
        )
    }

    #[test]
    fn segmenter_small_unit() {
        let unit = make_unit("Short text.");
        let segmenter = Segmenter::default();
        let segments = segmenter.segment_unit(&unit);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.raw, "Short text.");
    }

    #[test]
    fn segmenter_large_unit() {
        let text = "First sentence. Second sentence! Third sentence? Fourth sentence.";
        let unit = make_unit(text);
        let segmenter = Segmenter::default();
        let segments = segmenter.segment_unit(&unit);

        assert!(segments.len() >= 2);
        assert_eq!(segments[0].text.raw, "First sentence.");
    }

    #[test]
    fn segmenter_context() {
        let text = "One. Two. Three. Four. Five.";
        let unit = make_unit(text);
        let segmenter = Segmenter::default().with_context_window(1);
        let segments = segmenter.segment_unit(&unit);

        // Segmento do meio deve ter contexto
        if segments.len() > 2 {
            assert!(segments[2].context_before.is_some());
            assert!(segments[2].context_after.is_some());
        }
    }

    #[test]
    fn recombine() {
        let unit = make_unit("First. Second. Third.");
        let segmenter = Segmenter::default();
        let segments = segmenter.segment_unit(&unit);

        // Simula tradução
        let translated: Vec<_> = segments.iter().enumerate().map(|(i, s)| {
            let mut seg = s.clone();
            seg.text = MarkedText::new(&format!("Traduzido {}.", i + 1));
            seg
        }).collect();

        let result = segmenter.recombine(&unit, &translated).unwrap();
        assert!(result.translation.is_some());
        let translation = result.translation.unwrap();
        assert!(translation.raw.contains("Traduzido 1"));
        assert!(translation.raw.contains("Traduzido 2"));
    }

    #[test]
    fn rolling_summary() {
        let mut rs = RollingSummary::new(50);
        rs.add("First sentence.");
        rs.add("Second sentence.");
        rs.add("Third sentence that is quite long and should cause trimming.");

        let summary = rs.summary();
        assert!(summary.len() <= 50 + 10); // margem para separadores
        assert!(summary.contains("Third"));
    }
}