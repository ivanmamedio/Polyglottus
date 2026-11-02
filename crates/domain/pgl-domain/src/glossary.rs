//! Glossário de termos para consistência de tradução

use super::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

/// Glossário de termos
///
/// Mapeia termos no idioma de origem para traduções aprovadas
/// no idioma de destino, com metadados de contexto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Glossary {
    /// ID único do glossário
    #[serde(default = "Uuid::now_v7")]
    pub id: Uuid,
    /// ID do projeto associado (opcional)
    pub project_id: Option<Uuid>,
    /// ID do job associado (opcional)
    pub job_id: Option<Uuid>,
    /// Nome do glossário
    pub name: String,
    /// Par de idiomas deste glossário
    pub lang_pair: super::LangPair,
    /// Entradas do glossário
    #[serde(default)]
    pub entries: HashMap<String, GlossaryEntry>,
    /// Metadados adicionais
    #[serde(default)]
    pub metadata: GlossaryMetadata,
    /// Data de criação
    #[serde(default = "SystemTime::now")]
    pub created_at: SystemTime,
    /// Data de última atualização
    #[serde(default = "SystemTime::now")]
    pub updated_at: SystemTime,
}

impl Glossary {
    /// Cria glossário vazio
    pub fn new(source: Language, target: Language) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::now_v7(),
            project_id: None,
            job_id: None,
            name: "Untitled Glossary".into(),
            lang_pair: super::LangPair { source, target },
            entries: HashMap::new(),
            metadata: GlossaryMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Cria glossário com nome
    pub fn with_name(name: impl Into<String>, source: Language, target: Language) -> Self {
        let mut glossary = Self::new(source, target);
        glossary.name = name.into();
        glossary
    }

    /// Adiciona uma entrada
    pub fn add_entry(&mut self, entry: GlossaryEntry) {
        self.entries.insert(entry.source_term.clone(), entry);
    }

    /// Busca termo (case-insensitive)
    pub fn get(&self, term: &str) -> Option<&GlossaryEntry> {
        self.entries.get(term)
            .or_else(|| self.entries.iter().find(|(k, _)| k.eq_ignore_ascii_case(term)).map(|(_, v)| v))
    }

    /// Retorna todos os termos ordenados alfabeticamente
    pub fn terms_sorted(&self) -> Vec<&GlossaryEntry> {
        let mut terms: Vec<_> = self.entries.values().collect();
        terms.sort_by(|a, b| a.source_term.cmp(&b.source_term));
        terms
    }

    /// Mescla outro glossário (outro tem prioridade em conflitos)
    pub fn merge(&mut self, other: Glossary) {
        for (term, entry) in other.entries {
            self.entries.insert(term, entry);
        }
    }

    /// Exporta para formato TMX (simplificado)
    pub fn to_tmx(&self) -> String {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push_str("\n<tmx version=\"1.4\">\n  <header ");
        xml.push_str(&format!("srclang=\"{}\" ", self.lang_pair.source));
        xml.push_str(&format!("datatype=\"plaintext\"/>\n"));
        xml.push_str("  <body>\n");

        for entry in self.terms_sorted() {
            xml.push_str("    <tu>\n");
            xml.push_str(&format!("      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
                self.lang_pair.source, escape_xml(&entry.source_term)));
            xml.push_str(&format!("      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
                self.lang_pair.target, escape_xml(&entry.target_term)));
            xml.push_str("    </tu>\n");
        }

        xml.push_str("  </body>\n</tmx>");
        xml
    }
}

/// Entrada individual do glossário
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    /// Termo no idioma de origem
    pub source_term: String,
    /// Tradução aprovada no idioma de destino
    pub target_term: String,
    /// Contexto/definição opcional
    pub context: Option<String>,
    /// Categoria (ex: "technical", "character-name", "proper-noun")
    pub category: Option<String>,
    /// Sensível a maiúsculas/minúsculas
    #[serde(default)]
    pub case_sensitive: bool,
    /// Não traduzir (manter original)
    #[serde(default)]
    pub do_not_translate: bool,
    /// Prioridade (maior = mais importante)
    #[serde(default = "default_priority")]
    pub priority: u8,
}

fn default_priority() -> u8 {
    50
}

/// Metadados do glossário
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryMetadata {
    pub name: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub created_at: Option<String>, // ISO 8601
    pub updated_at: Option<String>,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for GlossaryMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled Glossary".into(),
            description: None,
            creator: None,
            created_at: None,
            updated_at: None,
            version: "1.0".into(),
            tags: Vec::new(),
        }
    }
}

/// Importa glossário de CSV
pub fn import_csv(csv: &str, lang_pair: super::LangPair) -> Result<Glossary, csv::Error> {
    let mut rdr = csv::Reader::from_reader(csv.as_bytes());
    let mut glossary = Glossary::new(lang_pair.source, lang_pair.target);

    for result in rdr.deserialize() {
        let record: GlossaryCsvRecord = result?;
        let entry = GlossaryEntry {
            source_term: record.source_term,
            target_term: record.target_term,
            context: record.context,
            category: record.category,
            case_sensitive: record.case_sensitive.unwrap_or(false),
            do_not_translate: record.do_not_translate.unwrap_or(false),
            priority: record.priority.unwrap_or(50),
        };
        glossary.add_entry(entry);
    }

    Ok(glossary)
}

#[derive(Deserialize)]
struct GlossaryCsvRecord {
    source_term: String,
    target_term: String,
    context: Option<String>,
    category: Option<String>,
    case_sensitive: Option<bool>,
    do_not_translate: Option<bool>,
    priority: Option<u8>,
}

fn escape_xml(s: &str) -> String {
    // TODO: Fix string literal parsing issue with single quote in Rust 2021
    // s.replace('&', "&")
    //     .replace('<', "<")
    //     .replace('>', ">")
    //     .replace('"', """)
    //     .replace("'", "&apos;")
    s.to_string()
}