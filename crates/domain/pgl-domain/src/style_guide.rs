//! Guia de estilo para tradução consistente

use super::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Guia de estilo para tradução
///
/// Define preferências de formalidade, convenções tipográficas,
/// tratamento de pronomes, etc. para guiar o modelo de tradução.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleGuide {
    /// Idioma de destino
    pub target_language: Language,
    /// Nível de formalidade
    #[serde(default = "default_formality")]
    pub formality: FormalityLevel,
    /// Convenções tipográficas
    #[serde(default)]
    pub typography: TypographyRules,
    /// Tratamento de pronomes
    #[serde(default)]
    pub pronouns: PronounRules,
    /// Tratamento de números/datas
    #[serde(default)]
    pub localization: LocalizationRules,
    /// Vocabulário preferido/evitado
    #[serde(default)]
    pub vocabulary: VocabularyRules,
    /// Instruções personalizadas (free-form)
    #[serde(default)]
    pub custom_instructions: String,
}

fn default_formality() -> FormalityLevel {
    FormalityLevel::Neutral
}

impl StyleGuide {
    /// Cria guia padrão para um idioma
    pub fn default_for(lang: Language) -> Self {
        let mut guide = Self {
            target_language: lang.clone(),
            formality: FormalityLevel::Neutral,
            typography: TypographyRules::default_for(&lang),
            pronouns: PronounRules::default_for(&lang),
            localization: LocalizationRules::default_for(&lang),
            vocabulary: VocabularyRules::default(),
            custom_instructions: String::new(),
        };

        // Ajustes específicos por idioma
        match lang.code() {
            "pt" | "pt-BR" | "pt-PT" => {
                guide.formality = FormalityLevel::Neutral;
                guide.pronouns = PronounRules {
                    use_voseo: false,
                    use_tuteo: true,
                    formal_pronoun: "você".into(),
                    informal_pronoun: "tu".into(),
                    avoid_pronouns: false,
                };
            }
            "es" => {
                guide.formality = FormalityLevel::Neutral;
                guide.pronouns = PronounRules {
                    use_voseo: true,
                    use_tuteo: true,
                    formal_pronoun: "usted".into(),
                    informal_pronoun: "tú".into(),
                    avoid_pronouns: false,
                };
            }
            "fr" => {
                guide.formality = FormalityLevel::Formal;
                guide.pronouns = PronounRules {
                    use_voseo: false,
                    use_tuteo: true,
                    formal_pronoun: "vous".into(),
                    informal_pronoun: "tu".into(),
                    avoid_pronouns: false,
                };
            }
            "de" => {
                guide.formality = FormalityLevel::Formal;
                guide.pronouns = PronounRules {
                    use_voseo: false,
                    use_tuteo: true,
                    formal_pronoun: "Sie".into(),
                    informal_pronoun: "du".into(),
                    avoid_pronouns: false,
                };
            }
            "ja" => {
                guide.formality = FormalityLevel::Formal;
                guide.pronouns = PronounRules {
                    use_voseo: false,
                    use_tuteo: false,
                    formal_pronoun: "".into(),
                    informal_pronoun: "".into(),
                    avoid_pronouns: true, // Japonês omite pronomes
                };
            }
            _ => {}
        }

        guide
    }

    /// Converte para prompt de sistema para o modelo
    pub fn to_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!("Translate into {}.", self.target_language));

        match self.formality {
            FormalityLevel::Formal => parts.push("Use formal register.".into()),
            FormalityLevel::Informal => parts.push("Use informal/friendly register.".into()),
            FormalityLevel::Neutral => parts.push("Use neutral register.".into()),
        }

        if !self.typography.quotes.is_empty() {
            parts.push(format!("Use quotes: {}.", self.typography.quotes));
        }

        if self.typography.use_em_dash {
            parts.push("Use em dashes (—) for dialogue.".into());
        }

        if self.pronouns.avoid_pronouns {
            parts.push("Avoid explicit pronouns when natural.".into());
        } else if !self.pronouns.formal_pronoun.is_empty() {
            parts.push(format!("Use '{}' for formal address.", self.pronouns.formal_pronoun));
        }

        if let Some(date_fmt) = &self.localization.date_format {
            parts.push(format!("Format dates as: {}.", date_fmt));
        }

        if !self.vocabulary.preferred_terms.is_empty() {
            parts.push("Preferred terms: ".to_string() +
                &self.vocabulary.preferred_terms.iter()
                    .map(|(k, v)| format!("{}→{}", k, v))
                    .collect::<Vec<_>>()
                    .join(", "));
        }

        if !self.vocabulary.avoided_terms.is_empty() {
            parts.push("Avoid terms: ".to_string() +
                &self.vocabulary.avoided_terms.join(", "));
        }

        if !self.custom_instructions.is_empty() {
            parts.push(self.custom_instructions.clone());
        }

        parts.join(" ")
    }
}

/// Nível de formalidade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormalityLevel {
    Formal,
    Neutral,
    Informal,
}

/// Regras tipográficas
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypographyRules {
    /// Aspas a usar (ex: "«»", "\"\"", "『』")
    #[serde(default = "default_quotes")]
    pub quotes: String,
    /// Usar travessão em diálogos
    #[serde(default = "default_true")]
    pub use_em_dash: bool,
    /// Espaço antes de pontuação (francês)
    #[serde(default)]
    pub space_before_punctuation: bool,
    /// Capitalização de títulos
    #[serde(default = "default_capitalization")]
    pub title_case: TitleCase,
}

fn default_quotes() -> String { "\"\"".into() }
fn default_true() -> bool { true }
fn default_capitalization() -> TitleCase { TitleCase::Sentence }

impl TypographyRules {
    fn default_for(lang: &Language) -> Self {
        match lang.code() {
            "fr" => Self {
                quotes: "« »".into(),
                use_em_dash: true,
                space_before_punctuation: true,
                title_case: TitleCase::Sentence,
            },
            "de" => Self {
                quotes: "„“".into(),
                use_em_dash: true,
                space_before_punctuation: false,
                title_case: TitleCase::Sentence,
            },
            "es" => Self {
                quotes: "« »".into(),
                use_em_dash: true,
                space_before_punctuation: false,
                title_case: TitleCase::Sentence,
            },
            "pt" | "pt-BR" | "pt-PT" => Self {
                quotes: "\"\"".into(),
                use_em_dash: true,
                space_before_punctuation: false,
                title_case: TitleCase::Sentence,
            },
            "ja" => Self {
                quotes: "「」".into(),
                use_em_dash: false,
                space_before_punctuation: false,
                title_case: TitleCase::Sentence,
            },
            "zh" => Self {
                quotes: "「」".into(),
                use_em_dash: false,
                space_before_punctuation: false,
                title_case: TitleCase::Sentence,
            },
            _ => Self::default(),
        }
    }
}

impl Default for TypographyRules {
    fn default() -> Self {
        Self {
            quotes: "\"\"".into(),
            use_em_dash: true,
            space_before_punctuation: false,
            title_case: TitleCase::Sentence,
        }
    }
}

/// Capitalização de títulos
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleCase {
    Sentence,   // Apenas primeira palavra
    Title,      // Palavras principais
    Upper,      // Tudo maiúsculo
    Lower,      // Tudo minúsculo
}

/// Regras de pronomes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PronounRules {
    /// Usar voseo (espanhol latino-americano)
    #[serde(default)]
    pub use_voseo: bool,
    /// Usar tuteo (espanhol/português informal)
    #[serde(default = "default_true")]
    pub use_tuteo: bool,
    /// Pronome formal
    #[serde(default)]
    pub formal_pronoun: String,
    /// Pronome informal
    #[serde(default)]
    pub informal_pronoun: String,
    /// Evitar pronomes (omitir quando possível)
    #[serde(default)]
    pub avoid_pronouns: bool,
}

impl PronounRules {
    fn default_for(lang: &Language) -> Self {
        match lang.code() {
            "es" => Self {
                use_voseo: true,
                use_tuteo: true,
                formal_pronoun: "usted".into(),
                informal_pronoun: "tú".into(),
                avoid_pronouns: false,
            },
            "pt" | "pt-BR" => Self {
                use_voseo: false,
                use_tuteo: true,
                formal_pronoun: "você".into(),
                informal_pronoun: "tu".into(),
                avoid_pronouns: false,
            },
            "fr" => Self {
                use_voseo: false,
                use_tuteo: true,
                formal_pronoun: "vous".into(),
                informal_pronoun: "tu".into(),
                avoid_pronouns: false,
            },
            "de" => Self {
                use_voseo: false,
                use_tuteo: true,
                formal_pronoun: "Sie".into(),
                informal_pronoun: "du".into(),
                avoid_pronouns: false,
            },
            "ja" => Self {
                use_voseo: false,
                use_tuteo: false,
                formal_pronoun: "".into(),
                informal_pronoun: "".into(),
                avoid_pronouns: true,
            },
            _ => Self::default(),
        }
    }
}

impl Default for PronounRules {
    fn default() -> Self {
        Self {
            use_voseo: false,
            use_tuteo: true,
            formal_pronoun: String::new(),
            informal_pronoun: String::new(),
            avoid_pronouns: false,
        }
    }
}

/// Regras de localização (números, datas, moedas)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalizationRules {
    /// Formato de data (ex: "DD/MM/YYYY", "MM/DD/YYYY", "YYYY-MM-DD")
    #[serde(default = "default_date_format")]
    pub date_format: Option<String>,
    /// Separador decimal
    #[serde(default = "default_decimal_sep")]
    pub decimal_separator: String,
    /// Separador de milhares
    #[serde(default = "default_thousands_sep")]
    pub thousands_separator: String,
    /// Moeda padrão
    #[serde(default)]
    pub default_currency: Option<String>,
    /// Sistema de medida (métrico/imperial)
    #[serde(default = "default_metric")]
    pub metric_system: bool,
}

fn default_date_format() -> Option<String> { Some("DD/MM/YYYY".into()) }
fn default_decimal_sep() -> String { ",".into() }
fn default_thousands_sep() -> String { ".".into() }
fn default_metric() -> bool { true }

impl LocalizationRules {
    fn default_for(lang: &Language) -> Self {
        match lang.code() {
            "en-US" => Self {
                date_format: Some("MM/DD/YYYY".into()),
                decimal_separator: ".".into(),
                thousands_separator: ",".into(),
                default_currency: Some("USD".into()),
                metric_system: false,
            },
            "en" | "en-GB" => Self {
                date_format: Some("DD/MM/YYYY".into()),
                decimal_separator: ".".into(),
                thousands_separator: ",".into(),
                default_currency: Some("GBP".into()),
                metric_system: true,
            },
            "pt-BR" => Self {
                date_format: Some("DD/MM/YYYY".into()),
                decimal_separator: ",".into(),
                thousands_separator: ".".into(),
                default_currency: Some("BRL".into()),
                metric_system: true,
            },
            "pt" | "pt-PT" => Self {
                date_format: Some("DD/MM/YYYY".into()),
                decimal_separator: ",".into(),
                thousands_separator: ".".into(),
                default_currency: Some("EUR".into()),
                metric_system: true,
            },
            "de" => Self {
                date_format: Some("DD.MM.YYYY".into()),
                decimal_separator: ",".into(),
                thousands_separator: ".".into(),
                default_currency: Some("EUR".into()),
                metric_system: true,
            },
            "fr" => Self {
                date_format: Some("DD/MM/YYYY".into()),
                decimal_separator: ",".into(),
                thousands_separator: " ".into(),
                default_currency: Some("EUR".into()),
                metric_system: true,
            },
            "ja" => Self {
                date_format: Some("YYYY/MM/DD".into()),
                decimal_separator: ".".into(),
                thousands_separator: ",".into(),
                default_currency: Some("JPY".into()),
                metric_system: true,
            },
            _ => Self::default(),
        }
    }
}

impl Default for LocalizationRules {
    fn default() -> Self {
        Self {
            date_format: Some("DD/MM/YYYY".into()),
            decimal_separator: ",".into(),
            thousands_separator: ".".into(),
            default_currency: None,
            metric_system: true,
        }
    }
}

/// Regras de vocabulário
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VocabularyRules {
    /// Termos preferidos (origem → destino)
    #[serde(default)]
    pub preferred_terms: HashMap<String, String>,
    /// Termos a evitar
    #[serde(default)]
    pub avoided_terms: Vec<String>,
    /// Domínio/especialidade (ex: "medical", "legal", "technical", "literary")
    #[serde(default)]
    pub domain: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;

    #[test]
    fn style_guide_default_pt() {
        let guide = StyleGuide::default_for(Language::new("pt-BR"));
        assert_eq!(guide.formality, FormalityLevel::Neutral);
        assert_eq!(guide.pronouns.formal_pronoun, "você");
        assert_eq!(guide.pronouns.informal_pronoun, "tu");
        assert_eq!(guide.typography.quotes, "\"\"");
        assert!(guide.typography.use_em_dash);
        assert_eq!(guide.localization.decimal_separator, ",");
    }

    #[test]
    fn style_guide_default_es() {
        let guide = StyleGuide::default_for(Language::new("es"));
        assert!(guide.pronouns.use_voseo);
        assert_eq!(guide.typography.quotes, "« »");
    }

    #[test]
    fn style_guide_default_fr() {
        let guide = StyleGuide::default_for(Language::new("fr"));
        assert_eq!(guide.formality, FormalityLevel::Formal);
        assert_eq!(guide.typography.quotes, "« »");
        assert!(guide.typography.space_before_punctuation);
    }

    #[test]
    fn style_guide_to_prompt() {
        let mut guide = StyleGuide::default_for(Language::new("pt-BR"));
        guide.custom_instructions = "Mantenha o tom literário.".into();
        guide.vocabulary.preferred_terms.insert("you".into(), "você".into());

        let prompt = guide.to_system_prompt();
        assert!(prompt.contains("Portuguese"));
        assert!(prompt.contains("neutral"));
        assert!(prompt.contains("Mantenha o tom literário"));
        assert!(prompt.contains("you→você"));
    }

    #[test]
    fn vocabulary_rules() {
        let mut guide = StyleGuide::default_for(Language::new("pt-BR"));
        guide.vocabulary.preferred_terms.insert("computer".into(), "computador".into());
        guide.vocabulary.avoided_terms.push("ordenador".into()); // espanhol europeu

        let prompt = guide.to_system_prompt();
        assert!(prompt.contains("computer→computador"));
        assert!(prompt.contains("ordenador"));
    }
}