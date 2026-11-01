//! Texto com marcação inline protegida via placeholders
//!
//! Estratégia: tags inline (bold, italic, links, etc.) são substituídas
//! por placeholders numerados ⟦1⟧...⟦/1⟧ antes de enviar para tradução.
//! Na volta, validamos contagem, ordem e aninhamento.

use super::{QaWarning, QaWarningCode, QaSeverity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Texto marcado com placeholders para proteção de marcação inline
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkedText {
    /// Texto com placeholders (ex: "Ele disse ⟦1⟧não⟦/1⟧ e saiu.")
    pub raw: String,
    /// Mapeamento placeholder → tag original
    /// Ex: "1" → "<b>", "/1" → "</b>"
    #[serde(default)]
    pub placeholders: HashMap<String, String>,
    /// Texto original sem placeholders (para referência/diff)
    #[serde(skip)]
    pub original: Option<String>,
}

impl std::hash::Hash for MarkedText {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        // Hash the placeholders in a deterministic order
        let mut keys: Vec<_> = self.placeholders.keys().collect();
        keys.sort();
        for key in keys {
            key.hash(state);
            self.placeholders[key].hash(state);
        }
        self.original.hash(state);
    }
}

impl MarkedText {
    /// Cria novo MarkedText a partir de texto simples (sem marcação)
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            raw: text.into(),
            placeholders: HashMap::new(),
            original: None,
        }
    }

    /// Cria a partir de HTML/XML, extraindo tags inline como placeholders
    pub fn from_html(html: &str) -> Result<Self, MarkError> {
        let mut placeholders = HashMap::new();
        let mut raw = String::with_capacity(html.len());
        let mut tag_stack = Vec::new();
        let mut placeholder_counter = 0usize;
        let mut chars = html.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '<' {
                // Peek ahead to see if it's a closing tag or self-closing
                let mut tag = String::new();
                tag.push('<');

                while let Some(&next_c) = chars.peek() {
                    tag.push(next_c);
                    chars.next();
                    if next_c == '>' {
                        break;
                    }
                }

                if tag.ends_with("/>") {
                    // Self-closing tag (ex: <br/>, <img/>)
                    placeholder_counter += 1;
                    let ph = format!("⟦{}⟧", placeholder_counter);
                    let close_ph = format!("⟦/{}⟧", placeholder_counter);
                    raw.push_str(&ph);
                    raw.push_str(&close_ph);
                    placeholders.insert(placeholder_counter.to_string(), tag.clone());
                    placeholders.insert(format!("/{}", placeholder_counter), String::new());
                } else if tag.starts_with("</") {
                    // Closing tag
                    if let Some(open_num) = tag_stack.pop() {
                        let ph = format!("⟦/{open_num}⟧");
                        eprintln!("DEBUG from_html: created closing placeholder '{}' for tag '{}'", ph, tag);
                        raw.push_str(&ph);
                        placeholders.insert(format!("/{}", open_num), tag);
                    } else {
                        return Err(MarkError::UnmatchedClosingTag(tag));
                    }
                } else {
                    // Opening tag
                    placeholder_counter += 1;
                    let ph = format!("⟦{}⟧", placeholder_counter);
                    eprintln!("DEBUG from_html: created placeholder '{}' (bytes: {:?}) for tag '{}'", ph, ph.as_bytes(), tag);
                    raw.push_str(&ph);
                    placeholders.insert(placeholder_counter.to_string(), tag.clone());
                    tag_stack.push(placeholder_counter);
                }
            } else {
                raw.push(c);
            }
        }

        if !tag_stack.is_empty() {
            return Err(MarkError::UnclosedTags(tag_stack.len()));
        }

        Ok(Self {
            raw,
            placeholders,
            original: Some(html.to_string()),
        })
    }

    /// Restaura a marcação original a partir dos placeholders
    pub fn restore(&self) -> Result<String, RestoreError> {
        eprintln!("DEBUG RESTORE: raw='{}'", self.raw);
        let mut result = String::with_capacity(self.raw.len() + self.placeholders.len() * 4);
        let raw_chars: Vec<char> = self.raw.chars().collect();
        eprintln!("DEBUG: raw chars: {:?}", self.raw.chars().collect::<Vec<_>>());
        
        let mut chars = self.raw.chars().peekable();
        let mut index = 0;
        
        while let Some(c) = chars.next() {
            eprintln!("DEBUG: index={}, char='{}' (U+{:04X})", index, c, c as u32);
            index += 1;
            if c == '⟦' {
                // Parse placeholder
                let mut ph_num = String::new();
                chars.next(); // consume '⟦'
                
                eprintln!("DEBUG: after consuming ⟦, peek={:?}", chars.peek());
                
                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    eprintln!("DEBUG: read char '{}' (U+{:04X}), ph_num='{}'", next_c, next_c as u32, ph_num);
                    if next_c == '⟧' {
                        break;
                    }
                    ph_num.push(next_c);
                }
                
                eprintln!("DEBUG: finished parsing placeholder, ph_num='{}'", ph_num);
                
                if let Some(replacement) = self.placeholders.get(&ph_num) {
                    eprintln!("DEBUG RESTORE: found placeholder '{}' -> '{}'", ph_num, replacement);
                    if !replacement.is_empty() {
                        result.push_str(replacement);
                    }
                } else {
                    eprintln!("DEBUG: placeholder '{}' NOT FOUND (available: {:?})", ph_num, self.placeholders.keys().collect::<Vec<_>>());
                    result.push('⟦');
                    result.push_str(&ph_num);
                    result.push('⟧');
                }
            } else {
                result.push(c);
            }
            index += 1;
        }

        Ok(result)
    }

    /// Valida se todos os placeholders originais estão presentes na tradução
    pub fn validate_placeholders_restored(&self, translated: &MarkedText) -> Result<(), PlaceholderValidationError> {
        let original_ph: Vec<_> = self.placeholders.keys()
            .filter(|k| !k.starts_with('/'))
            .cloned()
            .collect();

        let translated_ph: Vec<_> = translated.placeholders.keys()
            .filter(|k| !k.starts_with('/'))
            .cloned()
            .collect();

        // Verifica contagem
        if original_ph.len() != translated_ph.len() {
            return Err(PlaceholderValidationError::CountMismatch {
                expected: original_ph.len(),
                found: translated_ph.len(),
            });
        }

        // Verifica ordem e existência
        for (i, (orig, trans)) in original_ph.iter().zip(translated_ph.iter()).enumerate() {
            if orig != trans {
                return Err(PlaceholderValidationError::OrderMismatch {
                    position: i,
                    expected: format!("⟦{}⟧", orig),
                    found: format!("⟦{}⟧", trans),
                });
            }
        }

        // Verifica aninhamento (placeholders de fechamento na ordem inversa)
        let original_close: Vec<_> = self.placeholders.keys()
            .filter(|k| k.starts_with('/'))
            .cloned()
            .collect();

        let translated_close: Vec<_> = translated.placeholders.keys()
            .filter(|k| k.starts_with('/'))
            .cloned()
            .collect();

        if original_close != translated_close {
            return Err(PlaceholderValidationError::NestingViolation(
                "Closing placeholders order mismatch".into()
            ));
        }

        Ok(())
    }

    /// Gera avisos de QA para placeholders
    pub fn qa_placeholder_warnings(&self, translated: &MarkedText) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        let original_count = self.placeholders.values().filter(|v| !v.is_empty()).count();
        let translated_count = translated.placeholders.values().filter(|v| !v.is_empty()).count();

        if original_count != translated_count {
            warnings.push(QaWarning {
                code: QaWarningCode::PlaceholderMismatch,
                message: format!("Placeholder count mismatch: original={}, translated={}", original_count, translated_count),
                severity: QaSeverity::Error,
            });
        }

        warnings
    }

    /// Retorna texto legível (placeholders como tags visuais)
    pub fn debug_display(&self) -> String {
        let mut result = self.raw.clone();
        for (ph, tag) in &self.placeholders {
            if !tag.is_empty() {
                let placeholder = format!("⟦{}⟧", ph);
                let replacement = format!("[{}]", tag.trim_matches(|c| c == '<' || c == '>' || c == '/'));
                result = result.replace(&placeholder, &replacement);
            }
        }
        result
    }
}

/// Erros de marcação
#[derive(Debug, thiserror::Error)]
pub enum MarkError {
    #[error("Unmatched closing tag: {0}")]
    UnmatchedClosingTag(String),
    #[error("Unclosed tags: {0}")]
    UnclosedTags(usize),
}

/// Erros de validação de placeholders
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceholderValidationError {
    #[error("Placeholder count mismatch: expected {expected}, found {found}")]
    CountMismatch { expected: usize, found: usize },

    #[error("Placeholder order mismatch at position {position}: expected {expected}, found {found}")]
    OrderMismatch { position: usize, expected: String, found: String },

    #[error("Placeholder nesting violation: {0}")]
    NestingViolation(String),
}

/// Erros de restauração
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("Invalid placeholder format")]
    InvalidFormat,
}

impl std::fmt::Display for MarkedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_text_no_markup() {
        let mt = MarkedText::new("Hello world");
        assert_eq!(mt.raw, "Hello world");
        assert!(mt.placeholders.is_empty());
        assert_eq!(mt.restore().unwrap(), "Hello world");
    }

    #[test]
    fn html_with_bold() {
        let html = "Ele disse <b>não</b> e saiu.";
        let mt = MarkedText::from_html(html).unwrap();

        assert!(mt.raw.contains("⟦1⟧"));
        assert!(mt.raw.contains("⟦/1⟧"));
        assert_eq!(mt.placeholders.get("1"), Some(&"<b>".to_string()));
        assert_eq!(mt.placeholders.get("/1"), Some(&"</b>".to_string()));

        let restored = mt.restore().unwrap();
        assert_eq!(restored, html);
    }

    #[test]
    fn html_with_nested_tags() {
        let html = "<p>Texto <b>negrito <i>itálico</i> fim</b> normal</p>";
        let mt = MarkedText::from_html(html).unwrap();

        let restored = mt.restore().unwrap();
        assert_eq!(restored, html);
    }

    #[test]
    fn html_self_closing() {
        let html = "Linha 1<br/>Linha 2<img src='x'/>Fim";
        let mt = MarkedText::from_html(html).unwrap();

        let restored = mt.restore().unwrap();
        assert_eq!(restored, html);
    }

    #[test]
    fn placeholder_validation_success() {
        let original = MarkedText::from_html("<b>Olá</b> mundo").unwrap();
        let mut translated = MarkedText::from_html("<b>Hello</b> world").unwrap();
        // Simula tradução que manteve placeholders
        translated.raw = original.raw.replace("Olá", "Hello").replace("mundo", "world");

        assert!(original.validate_placeholders_restored(&translated).is_ok());
    }

    #[test]
    fn placeholder_validation_count_mismatch() {
        let original = MarkedText::from_html("<b>Um</b> <i>dois</i>").unwrap();
        let translated = MarkedText::from_html("<b>One</b> two").unwrap(); // perdeu <i>

        let err = original.validate_placeholders_restored(&translated).unwrap_err();
        assert!(matches!(err, PlaceholderValidationError::CountMismatch { .. }));
    }

    #[test]
    fn placeholder_validation_order_mismatch() {
        let original = MarkedText::from_html("<b>Primeiro</b> <i>Segundo</i>").unwrap();
        let mut translated = MarkedText::from_html("<i>Second</i> <b>First</b>").unwrap();
        // Ordem trocada
        translated.raw = "⟦2⟧Second⟦/2⟧ ⟦1⟧First⟦/1⟧".to_string();

        let err = original.validate_placeholders_restored(&translated).unwrap_err();
        assert!(matches!(err, PlaceholderValidationError::OrderMismatch { .. }));
    }

    #[test]
    fn qa_warnings_generated() {
        let original = MarkedText::from_html("<b>Test</b>").unwrap();
        let translated = MarkedText::new("Teste sem bold"); // placeholders perdidos

        let warnings = original.qa_placeholder_warnings(&translated);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, QaWarningCode::PlaceholderMismatch);
    }
}