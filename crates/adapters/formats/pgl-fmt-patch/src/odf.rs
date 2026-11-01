//! Scanner e leitor para ODF (ODT, ODP, ODS)

use super::{FormatScanner, UnitLocation, UnitKind, SectionId};
use crate::opc::OpcPackage;
use pgl_domain::{UnitId, SourceRef};
use pgl_app::FormatError;
use std::sync::Arc;

/// Scanner ODF
pub struct OdfScanner;

impl FormatScanner for OdfScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        let mut locations = Vec::new();

        // ODF content.xml contém todo o conteúdo
        if let Some(content) = package.content_parts.get("content.xml") {
            self.scan_content_xml(content, &mut locations)?;
        }

        // Também verifica styles.xml para texto em estilos
        if let Some(styles) = package.content_parts.get("styles.xml") {
            self.scan_styles_xml(styles, &mut locations)?;
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "text:p, text:h, text:span, text:a"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["text:h", "text:list-item", "table:table-cell", "draw:text-box"]
    }
}

impl OdfScanner {
    fn scan_content_xml(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(content)?;

        self.walk_odf(&doc.document, "content.xml", locations, 0)
    }

    fn scan_styles_xml(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        // Textos em estilos (ex: cabeçalhos de página)
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(content)?;

        self.walk_odf(&doc.document, "styles.xml", locations, 0)
    }

    fn walk_odf(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
        depth: usize,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string();

                // Identifica elementos traduzíveis
                let (kind, text_content) = match tag_name.as_str() {
                    "p" | "span" | "a" => (UnitKind::Paragraph, self.extract_text_content(node)),
                    "h" => {
                        let level = attrs.borrow().iter()
                            .find(|a| a.name.local.as_ref() == b"outline-level")
                            .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1);
                        (UnitKind::Heading(level), self.extract_text_content(node))
                    }
                    "list-item" => (UnitKind::ListItem, self.extract_text_content(node)),
                    "table-cell" => (UnitKind::TableCell, self.extract_text_content(node)),
                    "text-box" => (UnitKind::Other, self.extract_text_content(node)),
                    _ => (UnitKind::Other, String::new()),
                };

                if !text_content.trim().is_empty() && matches!(kind, UnitKind::Paragraph | UnitKind::Heading(_) | UnitKind::ListItem | UnitKind::TableCell) {
                    let section = SectionId::new();
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_path.to_string(),
                        selector: format!("//{}", tag_name),
                        offset: None,
                        length: None,
                    };

                    locations.push(UnitLocation {
                        unit_id,
                        file_path: file_path.to_string(),
                        xpath: format!("//{}", tag_name),
                        text_node_index: 0,
                        original_text: text_content,
                        kind,
                        section,
                    });
                }

                // Recursão nos filhos
                for child in node.children.borrow().iter() {
                    self.walk_odf(child, file_path, locations, depth + 1)?;
                }
            }
            html5ever::rcdom::NodeData::Text { contents } => {
                // Textos soltos podem ser relevantes
            }
            _ => {}
        }

        Ok(())
    }

    fn extract_text_content(&self, node: &html5ever::rcdom::Handle) -> String {
        let mut text = String::new();
        self.collect_text(node, &mut text);
        text.trim().to_string()
    }

    fn collect_text(&self, node: &html5ever::rcdom::Handle, text: &mut String) {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Text { contents } => {
                text.push_str(&contents.borrow());
            }
            html5ever::rcdom::NodeData::Element { .. } => {
                for child in node.children.borrow().iter() {
                    self.collect_text(child, text);
                }
            }
            _ => {}
        }
    }
}

/// Leitor ODT
pub struct OdtReader {
    scanner: Arc<OdfScanner>,
}

impl OdtReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(OdfScanner) }
    }
}

impl pgl_fmt_core::ZipFormatReader for OdtReader {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::ODT
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odt"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.text"]
    }
}

/// Leitor ODP
pub struct OdpReader {
    scanner: Arc<OdfScanner>,
}

impl OdpReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(OdfScanner) }
    }
}

impl pgl_fmt_core::ZipFormatReader for OdpReader {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::ODP
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odp"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.presentation"]
    }
}

/// Leitor ODS
pub struct OdsReader {
    scanner: Arc<OdfScanner>,
}

impl OdsReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(OdfScanner) }
    }
}

impl pgl_fmt_core::ZipFormatReader for OdsReader {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::ODS
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".ods"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.spreadsheet"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn odf_scanner_basic() {
        let scanner = OdfScanner;
        let pkg = create_test_odf();

        let locations = scanner.scan(&pkg).unwrap();
        assert!(!locations.is_empty());

        // Deve encontrar parágrafos
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::Paragraph)));
    }

    fn create_test_odf() -> OpcPackage {
        // Cria ODF mínimo em memória para teste
        let mut parts = std::collections::HashMap::new();
        parts.insert("content.xml".to_string(), br#"<?xml version="1.0"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
      <text:p>First paragraph</text:p>
      <text:h text:outline-level="1">Heading</text:h>
      <text:p>Second paragraph</text:p>
      <text:list>
        <text:list-item><text:p>Item 1</text:p></text:list-item>
      </text:list>
      <table:table xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0">
        <table:table-row>
          <table:table-cell><text:p>Cell 1</text:p></table:table-cell>
        </table:table-row>
      </table:table>
    </office:text>
  </office:body>
</office:document-content>"#.to_vec());

        parts.insert("styles.xml".to_string(), b"<styles/>".to_vec());

        OpcPackage {
            parts,
            content_parts: parts.clone(),
            relationships: std::collections::HashMap::new(),
            content_types: Default::default(),
        }
    }
}