//! Scanner e leitor para EPUB (2 e 3)

use super::{FormatScanner, UnitLocation, UnitKind, SectionId};
use crate::opc::OpcPackage;
use pgl_domain::{UnitId, SourceRef};
use pgl_app::FormatError;
use std::sync::Arc;

/// Scanner EPUB
pub struct EpubScanner;

impl FormatScanner for EpubScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("EPUB scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption"]
    }
}

/// Leitor EPUB
pub struct EpubReader {
    scanner: Arc<EpubScanner>,
}

impl EpubReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(EpubScanner) }
    }
}

impl pgl_fmt_core::ZipFormatReader for EpubReader {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::EPUB
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/epub+zip"]
    }
}

/// Writer EPUB (usa patch engine)
pub struct EpubWriter;

impl EpubWriter {
    pub fn new() -> Self {
        Self
    }
}

impl pgl_fmt_core::ZipFormatWriter for EpubWriter {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::EPUB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epub_scanner_basic() {
        let scanner = EpubScanner;
        let pkg = create_test_epub();

        let locations = scanner.scan(&pkg).unwrap();
        assert!(!locations.is_empty());

        // Deve encontrar parágrafos e headings
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::Paragraph)));
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::Heading(_))));
    }

    fn create_test_epub() -> OpcPackage {
        let mut parts = std::collections::HashMap::new();

        parts.insert("META-INF/container.xml".to_string(), br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#.to_vec());

        parts.insert("OEBPS/content.opf".to_string(), br#"<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">test</dc:identifier>
    <dc:title>Test Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#.to_vec());

        parts.insert("OEBPS/ch1.xhtml".to_string(), br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
  <h1>Chapter One</h1>
  <p>First paragraph.</p>
  <p>Second paragraph with <em>emphasis</em>.</p>
  <ul>
    <li>Item 1</li>
    <li>Item 2</li>
  </ul>
</body>
</html>"#.to_vec());

        parts.insert("OEBPS/nav.xhtml".to_string(), br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="ch1.xhtml">Chapter 1</a></li>
    </ol>
  </nav>
</body>
</html>"#.to_vec());

        OpcPackage {
            parts,
            content_parts: parts.clone(),
            relationships: std::collections::HashMap::new(),
            content_types: Default::default(),
        }
    }
}