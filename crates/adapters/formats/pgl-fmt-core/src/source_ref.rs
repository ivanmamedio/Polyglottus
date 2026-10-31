//! Referências de origem (SourceRef) para patch no lugar

use pgl_domain::{SourceRef, UnitId};
use std::collections::HashMap;

/// Registry de referências de origem para escrita eficiente
pub struct SourceRefRegistry {
    refs: HashMap<UnitId, SourceRef>,
    /// Mapeamento path interno -> lista de UnitIds
    by_path: HashMap<String, Vec<UnitId>>,
}

impl SourceRefRegistry {
    pub fn new() -> Self {
        Self {
            refs: HashMap::new(),
            by_path: HashMap::new(),
        }
    }

    pub fn register(&mut self, unit_id: UnitId, source_ref: SourceRef) {
        self.by_path.entry(source_ref.internal_path.clone()).or_default().push(unit_id);
        self.refs.insert(unit_id, source_ref);
    }

    pub fn get(&self, unit_id: &UnitId) -> Option<&SourceRef> {
        self.refs.get(unit_id)
    }

    pub fn get_by_path(&self, path: &str) -> Vec<UnitId> {
        self.by_path.get(path).cloned().unwrap_or_default()
    }

    pub fn paths(&self) -> Vec<&String> {
        self.by_path.keys().collect()
    }
}

impl Default for SourceRefRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolvedor de SourceRef para escritores de formato
pub trait SourceRefResolver {
    /// Converte SourceRef para localização no formato alvo
    fn resolve(&self, source_ref: &SourceRef) -> Result<ResolvedLocation, ResolutionError>;

    /// Lista todas as localizações que precisam ser atualizadas
    fn all_locations(&self) -> Vec<ResolvedLocation>;
}

/// Localização resolvida no arquivo de destino
#[derive(Debug, Clone)]
pub struct ResolvedLocation {
    /// Caminho do arquivo interno (ex: "word/document.xml")
    pub file_path: String,
    /// XPath ou offset para o nó
    pub selector: String,
    /// Offset em bytes dentro do arquivo (se aplicável)
    pub byte_offset: Option<usize>,
    /// Comprimento do texto original
    pub length: Option<usize>,
}

/// Erros de resolução
#[derive(Debug, thiserror::Error)]
pub enum ResolutionError {
    #[error("Source ref not found: {0}")]
    NotFound(String),

    #[error("Invalid selector: {0}")]
    InvalidSelector(String),

    #[error("Multiple matches for selector: {0}")]
    MultipleMatches(String),

    #[error("Format mismatch: expected {expected}, found {found}")]
    FormatMismatch { expected: String, found: String },
}

/// Helper para construir SourceRef a partir de XPath
pub fn source_ref_from_xpath(internal_path: &str, xpath: &str) -> SourceRef {
    SourceRef {
        internal_path: internal_path.to_string(),
        selector: xpath.to_string(),
        offset: None,
        length: None,
    }
}

/// Helper para SourceRef com offset (ex: runs fragmentados em DOCX)
pub fn source_ref_with_offset(internal_path: &str, selector: &str, offset: usize, length: usize) -> SourceRef {
    SourceRef {
        internal_path: internal_path.to_string(),
        selector: selector.to_string(),
        offset: Some(offset),
        length: Some(length),
    }
}

/// Normaliza seletor para comparação
pub fn normalize_selector(selector: &str) -> String {
    selector
        .trim()
        .trim_start_matches('/')
        .trim_start_matches("//")
        .to_string()
}

/// Extrai parte numérica de seletor (ex: "//p[3]" -> 3)
pub fn extract_index(selector: &str) -> Option<usize> {
    let start = selector.rfind('[')?;
    let end = selector[start..].find(']')?;
    let num_str = &selector[start + 1..start + end];
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgl_domain::UnitId;

    #[test]
    fn source_ref_registry() {
        let mut registry = SourceRefRegistry::new();
        let id1 = UnitId::new();
        let id2 = UnitId::new();

        let ref1 = source_ref_from_xpath("ch1.xhtml", "//p[1]");
        let ref2 = source_ref_from_xpath("ch1.xhtml", "//p[2]");
        let ref3 = source_ref_from_xpath("ch2.xhtml", "//p[1]");

        registry.register(id1, ref1.clone());
        registry.register(id2, ref2.clone());
        registry.register(id3, ref3.clone());

        assert_eq!(registry.get(&id1), Some(&ref1));
        assert_eq!(registry.get_by_path("ch1.xhtml").len(), 2);
        assert_eq!(registry.get_by_path("ch2.xhtml").len(), 1);
        assert_eq!(registry.paths().len(), 2);
    }

    #[test]
    fn normalize_selector() {
        assert_eq!(normalize_selector("//p[1]"), "p[1]");
        assert_eq!(normalize_selector("/html/body/p"), "html/body/p");
        assert_eq!(normalize_selector("  //div  "), "div");
    }

    #[test]
    fn extract_index() {
        assert_eq!(extract_index("//p[1]"), Some(1));
        assert_eq!(extract_index("//p[42]"), Some(42));
        assert_eq!(extract_index("//p[last()]"), None);
        assert_eq!(extract_index("p"), None);
    }
}