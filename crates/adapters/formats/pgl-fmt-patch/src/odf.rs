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
        Err(FormatError::ParseError("ODF scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "text:p, text:h, text:span, text:a"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["text:h", "text:list-item", "table:table-cell", "draw:text-box"]
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