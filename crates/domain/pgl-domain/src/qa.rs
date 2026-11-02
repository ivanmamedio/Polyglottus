//! Quality Assurance automático por unidade

use super::{LangPair, MarkedText, QaSeverity, QaWarning, QaWarningCode, Unit};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Verificador de QA
pub struct QaChecker {
    config: QaConfig,
    /// Palavras comuns por idioma para detectar "wrong language"
    common_words: HashMap<Language, HashSet<String>>,
}

/// Configuração de QA
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaConfig {
    /// Habilitar verificação de idioma
    #[serde(default = "default_true")]
    pub check_language: bool,
    /// Habilitar verificação de razão de tamanho
    #[serde(default = "default_true")]
    pub check_length_ratio: bool,
    /// Habilitar verificação de placeholders
    #[serde(default = "default_true")]
    pub check_placeholders: bool,
    /// Habilitar detecção de repetição
    #[serde(default = "default_true")]
    pub check_repetition: bool,
    /// Habilitar verificação de glossário
    #[serde(default = "default_true")]
    pub check_glossary: bool,
    /// Habilitar verificação de entidades (números, datas, nomes)
    #[serde(default = "default_true")]
    pub check_entities: bool,
    /// Razão mínima esperada (target_len / source_len)
    #[serde(default = "default_min_ratio")]
    pub min_length_ratio: f32,
    /// Razão máxima esperada
    #[serde(default = "default_max_ratio")]
    pub max_length_ratio: f32,
    /// Threshold de repetição (n-grams)
    #[serde(default = "default_repetition_threshold")]
    pub repetition_threshold: f32,
}

fn default_true() -> bool { true }
fn default_min_ratio() -> f32 { 0.3 }
fn default_max_ratio() -> f32 { 3.0 }
fn default_repetition_threshold() -> f32 { 0.7 }

impl Default for QaConfig {
    fn default() -> Self {
        Self {
            check_language: true,
            check_length_ratio: true,
            check_placeholders: true,
            check_repetition: true,
            check_glossary: true,
            check_entities: true,
            min_length_ratio: 0.3,
            max_length_ratio: 3.0,
            repetition_threshold: 0.7,
        }
    }
}

impl QaChecker {
    pub fn new(config: QaConfig) -> Self {
        Self {
            config,
            common_words: default_common_words(),
        }
    }

    /// Executa todas as verificações em uma unidade traduzida
    pub fn check(&self, unit: &Unit, lang_pair: &LangPair, glossary: Option<&super::Glossary>) -> QaResult {
        let mut warnings = Vec::new();

        if let Some(translation) = &unit.translation {
            // 1. Verificação de idioma
            if self.config.check_language {
                warnings.extend(self.check_language(translation, &lang_pair.target));
            }

            // 2. Razão de tamanho
            if self.config.check_length_ratio {
                warnings.extend(self.check_length_ratio(unit, translation));
            }

            // 3. Placeholders
            if self.config.check_placeholders {
                warnings.extend(unit.text.qa_placeholder_warnings(translation));
            }

            // 4. Repetição/anomalia
            if self.config.check_repetition {
                warnings.extend(self.check_repetition(translation));
            }

            // 5. Glossário
            if self.config.check_glossary {
                if let Some(glossary) = glossary {
                    warnings.extend(self.check_glossary(unit, translation, glossary));
                }
            }

            // 6. Entidades (números, datas, nomes próprios)
            if self.config.check_entities {
                warnings.extend(self.check_entities(unit, translation));
            }
        }

        let passed = warnings.iter().all(|w| w.severity != QaSeverity::Error);
        let has_errors = warnings.iter().any(|w| w.severity == QaSeverity::Error);

        QaResult {
            passed,
            has_errors,
            warnings,
            unit_id: unit.id,
        }
    }

    fn check_language(&self, translation: &MarkedText, target_lang: &Language) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        // Heurística simples: verifica palavras comuns do idioma alvo
        if let Some(common) = self.common_words.get(target_lang) {
            let words: Vec<&str> = translation.raw.split_whitespace().collect();
            if words.len() >= 3 {
                let matches = words.iter().filter(|w| common.contains(&w.to_lowercase())).count();
                let ratio = matches as f32 / words.len() as f32;

                // Se menos de 20% das palavras são comuns no idioma alvo, suspeito
                if ratio < 0.2 && words.len() > 5 {
                    warnings.push(QaWarning {
                        code: QaWarningCode::WrongLanguage,
                        message: format!("Possible wrong language: only {:.0}% common words for {}", ratio * 100.0, target_lang),
                        severity: QaSeverity::Warning,
                    });
                }
            }
        }

        warnings
    }

    fn check_length_ratio(&self, unit: &Unit, translation: &MarkedText) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        let source_len = unit.text.raw.chars().count() as f32;
        let target_len = translation.raw.chars().count() as f32;

        if source_len > 0.0 {
            let ratio = target_len / source_len;

            if ratio < self.config.min_length_ratio {
                warnings.push(QaWarning {
                    code: QaWarningCode::LengthRatio,
                    message: format!("Translation too short: ratio {:.2} (min {:.2})", ratio, self.config.min_length_ratio),
                    severity: QaSeverity::Warning,
                });
            } else if ratio > self.config.max_length_ratio {
                warnings.push(QaWarning {
                    code: QaWarningCode::LengthRatio,
                    message: format!("Translation too long: ratio {:.2} (max {:.2})", ratio, self.config.max_length_ratio),
                    severity: QaSeverity::Warning,
                });
            }
        }

        warnings
    }

    fn check_repetition(&self, translation: &MarkedText) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        // Detecta n-grams repetidos (3-grams)
        let words: Vec<&str> = translation.raw.split_whitespace().collect();
        if words.len() >= 6 {
            let mut trigrams = HashSet::new();
            let mut repeated = 0;

            for window in words.windows(3) {
                let trigram = window.join(" ");
                if !trigrams.insert(trigram) {
                    repeated += 1;
                }
            }

            let total_trigrams = words.len().saturating_sub(2);
            let repetition_ratio = repeated as f32 / total_trigrams as f32;

            if repetition_ratio > self.config.repetition_threshold {
                warnings.push(QaWarning {
                    code: QaWarningCode::Repetition,
                    message: format!("High repetition detected: {:.0}% repeated trigrams", repetition_ratio * 100.0),
                    severity: QaSeverity::Error,
                });
            }
        }

        // Detecta loops simples (mesma palavra repetida muitas vezes)
        for window in words.windows(4) {
            if window.iter().all(|w| *w == window[0]) && window[0].len() > 1 {
                warnings.push(QaWarning {
                    code: QaWarningCode::Repetition,
                    message: format!("Word loop detected: '{}' repeated", window[0]),
                    severity: QaSeverity::Error,
                });
                break;
            }
        }

        warnings
    }

    fn check_glossary(&self, unit: &Unit, translation: &MarkedText, glossary: &super::Glossary) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        for entry in glossary.terms_sorted() {
            if entry.do_not_translate {
                // Verifica se termo "não traduzir" apareceu na tradução
                if translation.raw.contains(&entry.target_term) && entry.target_term != entry.source_term {
                    warnings.push(QaWarning {
                        code: QaWarningCode::GlossaryViolation,
                        message: format!("Do-not-translate term '{}' was translated", entry.source_term),
                        severity: QaSeverity::Error,
                    });
                }
            } else if translation.raw.contains(&entry.source_term) {
                // Termo do glossário aparece no original mas não na tradução
                if !translation.raw.contains(&entry.target_term) {
                    // Verifica se o termo está no texto original
                    if unit.text.raw.contains(&entry.source_term) {
                        warnings.push(QaWarning {
                            code: QaWarningCode::GlossaryViolation,
                            message: format!("Glossary term '{}' → '{}' not applied", entry.source_term, entry.target_term),
                            severity: QaSeverity::Warning,
                        });
                    }
                }
            }
        }

        warnings
    }

    fn check_entities(&self, unit: &Unit, translation: &MarkedText) -> Vec<QaWarning> {
        let mut warnings = Vec::new();

        // Números (incluindo com separadores)
        let number_regex = regex::Regex::new(r"\d[\d.,\s]*\d|\d+").unwrap();
        let source_numbers: Vec<_> = number_regex.find_iter(&unit.text.raw).map(|m| m.as_str()).collect();
        let target_numbers: Vec<_> = number_regex.find_iter(&translation.raw).map(|m| m.as_str()).collect();

        if source_numbers.len() != target_numbers.len() {
            warnings.push(QaWarning {
                code: QaWarningCode::EntityChanged,
                message: format!("Number count mismatch: source={}, target={}", source_numbers.len(), target_numbers.len()),
                severity: QaSeverity::Error,
            });
        } else {
            for (s, t) in source_numbers.iter().zip(target_numbers.iter()) {
                if s != t {
                    warnings.push(QaWarning {
                        code: QaWarningCode::EntityChanged,
                        message: format!("Number changed: '{}' → '{}'", s, t),
                        severity: QaSeverity::Error,
                    });
                }
            }
        }

        // URLs/emails (não devem ser traduzidos)
        let url_regex = regex::Regex::new(r"https?://\S+").unwrap();
        let source_urls: Vec<_> = url_regex.find_iter(&unit.text.raw).map(|m| m.as_str()).collect();
        let target_urls: Vec<_> = url_regex.find_iter(&translation.raw).map(|m| m.as_str()).collect();

        if source_urls != target_urls {
            warnings.push(QaWarning {
                code: QaWarningCode::EntityChanged,
                message: "URLs modified in translation".into(),
                severity: QaSeverity::Error,
            });
        }

        warnings
    }
}

/// Resultado de QA
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaResult {
    pub passed: bool,
    pub has_errors: bool,
    pub warnings: Vec<QaWarning>,
    pub unit_id: super::UnitId,
}

impl QaResult {
    pub fn error_count(&self) -> usize {
        self.warnings.iter().filter(|w| w.severity == QaSeverity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.iter().filter(|w| w.severity == QaSeverity::Warning).count()
    }
}

use std::collections::HashMap;
use super::Language;

fn default_common_words() -> HashMap<Language, HashSet<String>> {
    let mut map: HashMap<Language, HashSet<String>> = HashMap::new();

    // Português
    map.insert(Language::new("pt"), [
        "o", "a", "os", "as", "de", "do", "da", "dos", "das", "e", "é", "em", "um", "uma",
        "para", "com", "não", "se", "na", "no", "nas", "nos", "por", "mais", "como", "mas",
        "seu", "sua", "seus", "suas", "meu", "minha", "nosso", "nossa", "que", "quando",
    ].iter().map(|s| s.to_string()).collect());

    map.insert(Language::new("pt-BR"), map.get(&Language::new("pt")).unwrap().clone());
    map.insert(Language::new("pt-PT"), map.get(&Language::new("pt")).unwrap().clone());

    // Inglês
    map.insert(Language::new("en"), [
        "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for",
        "not", "on", "with", "he", "as", "you", "do", "at", "this", "but", "his", "by",
        "from", "they", "we", "say", "her", "she", "or", "an", "will", "my", "one", "all",
    ].iter().map(|s| s.to_string()).collect());

    // Espanhol
    map.insert(Language::new("es"), [
        "el", "la", "de", "que", "y", "a", "en", "un", "ser", "se", "no", "haber", "por",
        "con", "su", "para", "como", "estar", "tener", "le", "lo", "todo", "pero", "más",
        "hacer", "o", "poder", "decir", "este", "ir", "otro", "ese", "mi", "tu", "te",
    ].iter().map(|s| s.to_string()).collect());

    // Francês
    map.insert(Language::new("fr"), [
        "le", "de", "un", "à", "être", "et", "en", "avoir", "que", "pour", "dans", "ce",
        "il", "qui", "ne", "sur", "se", "pas", "plus", "pouvoir", "par", "je", "avec",
        "tout", "faire", "son", "mettre", "autre", "nous", "comme", "mais", "son", "lui",
    ].iter().map(|s| s.to_string()).collect());

    // Alemão
    map.insert(Language::new("de"), [
        "der", "die", "und", "in", "den", "von", "zu", "das", "mit", "sich", "des", "auf",
        "für", "ist", "im", "dem", "nicht", "ein", "eine", "als", "auch", "es", "an",
        "werden", "aus", "er", "hat", "dass", "sie", "nach", "wird", "bei", "einer", "um",
    ].iter().map(|s| s.to_string()).collect());

    // Italiano
    map.insert(Language::new("it"), [
        "il", "di", "che", "e", "la", "per", "un", "in", "a", "da", "non", "si", "le",
        "con", "dei", "del", "lo", "ma", "su", "al", "come", "più", "ho", "ha", "degli",
    ].iter().map(|s| s.to_string()).collect());

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Unit, UnitKind, MarkedText, SourceRef, SectionId, Language, Glossary, GlossaryEntry, QaSeverity, QaWarningCode};

    fn make_unit(text: &str) -> Unit {
        Unit::new(
            SourceRef { internal_path: "test".into(), selector: "1".into(), offset: None, length: None },
            UnitKind::Paragraph,
            MarkedText::new(text),
            SectionId::new(),
            Uuid::nil()
        )
    }

    #[test]
    fn qa_length_ratio() {
        let mut unit = make_unit("Short");
        unit.apply_translation(MarkedText::new("Esta é uma tradução muito longa para um texto curto")).unwrap();

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, None);

        assert!(!result.passed);
        assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::LengthRatio));
    }

    #[test]
    fn qa_repetition() {
        let mut unit = make_unit("Hello world");
        unit.apply_translation(MarkedText::new("Olá Olá Olá Olá Olá Olá")).unwrap();

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, None);

        assert!(!result.passed);
        assert!(result.has_errors);
        assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::Repetition));
    }

    #[test]
    fn qa_numbers_preserved() {
        let mut unit = make_unit("Price: $1,234.56");
        unit.apply_translation(MarkedText::new("Preço: $1,234.56")).unwrap();

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, None);

        assert!(result.passed);
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn qa_numbers_changed() {
        let mut unit = make_unit("Price: $1,234.56");
        unit.apply_translation(MarkedText::new("Preço: $1,234.57")).unwrap(); // centavos errados

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, None);

        assert!(!result.passed);
        assert!(result.has_errors);
        assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::EntityChanged));
    }

    #[test]
    fn qa_glossary_enforced() {
        let mut unit = make_unit("Press the Home button");
        let mut glossary = Glossary::new(Language::new("en"), Language::new("pt"));
        glossary.add_entry(GlossaryEntry {
            source_term: "Home button".into(),
            target_term: "botão Início".into(),
            context: None,
            category: Some("ui".into()),
            case_sensitive: false,
            do_not_translate: false,
            priority: 80,
        });

        // Tradução sem usar o glossário
        unit.apply_translation(MarkedText::new("Pressione o botão Home")).unwrap();

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, Some(&glossary));

        assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::GlossaryViolation));
    }

    #[test]
    fn qa_do_not_translate() {
        let mut unit = make_unit("Use the iPhone app");
        let mut glossary = Glossary::new(Language::new("en"), Language::new("pt"));
        glossary.add_entry(GlossaryEntry {
            source_term: "iPhone".into(),
            target_term: "iPhone".into(),
            context: None,
            category: Some("brand".into()),
            case_sensitive: true,
            do_not_translate: true,
            priority: 100,
        });

        // Tradução que traduziu "iPhone" (não deveria)
        unit.apply_translation(MarkedText::new("Use o aplicativo iPhone")).unwrap();

        let checker = QaChecker::new(QaConfig::default());
        let result = checker.check(&unit, &super::LangPair { source: Language::new("en"), target: Language::new("pt") }, Some(&glossary));

        assert!(result.warnings.iter().any(|w| w.code == QaWarningCode::GlossaryViolation && w.severity == QaSeverity::Error));
    }
}