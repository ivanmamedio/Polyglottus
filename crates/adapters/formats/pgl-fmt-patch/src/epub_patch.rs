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
        let mut locations = Vec::new();

        // Encontra OPF (package document)
        let opf_path = self.find_opf(package)?;
        let opf_content = package.parts.get(&opf_path)
            .ok_or_else(|| FormatError::ParseError("OPF not found".into()))?;

        // Parse OPF para encontrar spine (ordem de leitura)
        let spine_items = self.parse_spine(opf_content)?;

        // Processa cada item do spine em ordem
        for (index, (href, idref)) in spine_items.iter().enumerate() {
            let full_path = self.resolve_href(&opf_path, href);
            if let Some(content) = package.content_parts.get(&full_path) {
                self.scan_xhtml(content, &full_path, &mut locations, index)?;
            }
        }

        // Também processa nav.xhtml (sumário EPUB3)
        if let Some(nav) = package.content_parts.get("nav.xhtml").or_else(|| package.content_parts.get("toc.xhtml")) {
            self.scan_nav(nav, &mut locations)?;
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption"]
    }
}

impl EpubScanner {
    fn find_opf(&self, package: &OpcPackage) -> Result<String, FormatError> {
        // Procura em META-INF/container.xml
        if let Some(container) = package.parts.get("META-INF/container.xml") {
            let content = String::from_utf8_lossy(container);
            // Parse simplificado
            if let Some(start) = content.find("full-path=\"") {
                let start = start + 11;
                if let Some(end) = content[start..].find('"') {
                    return Ok(content[start..start+end].to_string());
                }
            }
        }

        // Fallback: procura arquivo .opf
        for name in package.parts.keys() {
            if name.ends_with(".opf") {
                return Ok(name.clone());
            }
        }

        Err(FormatError::ParseError("No OPF found".into()))
    }

    fn parse_spine(&self, opf: &[u8]) -> Result<Vec<(String, String)>, FormatError> {
        let content = String::from_utf8_lossy(opf);
        let mut spine = Vec::new();

        // Parse simplificado - em produção usar XML parser
        let mut in_spine = false;
        for line in content.lines() {
            if line.contains("<spine") { in_spine = true; continue; }
            if line.contains("</spine>") { in_spine = false; continue; }
            if in_spine && line.contains("<itemref") {
                if let Some(idref) = extract_attr(line, "idref") {
                    // Encontra href correspondente no manifest
                    if let Some(href) = self.find_href_in_manifest(&content, &idref) {
                        spine.push((href, idref));
                    }
                }
            }
        }

        Ok(spine)
    }

    fn find_href_in_manifest(&self, opf: &str, idref: &str) -> Option<String> {
        let pattern = format!("id=\"{}\"", idref);
        if let Some(pos) = opf.find(&pattern) {
            // Procura href antes
            let before = &opf[..pos];
            if let Some(href_start) = before.rfind("href=\"") {
                let start = href_start + 6;
                if let Some(end) = before[start..].find('"') {
                    return Some(before[start..start+end].to_string());
                }
            }
        }
        None
    }

    fn resolve_href(&self, opf_path: &str, href: &str) -> String {
        let base = std::path::Path::new(opf_path).parent().unwrap_or_else(|| std::path::Path::new(""));
        base.join(href).to_string_lossy().to_string()
    }

    fn scan_xhtml(&self, content: &[u8], file_path: &str, locations: &mut Vec<UnitLocation>, section_index: usize) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(content)?;

        self.walk_xhtml(&doc.document, file_path, locations, section_index)
    }

    fn scan_nav(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(content)?;

        self.walk_xhtml(&doc.document, "nav.xhtml", locations, 0)
    }

    fn walk_xhtml(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
        section_index: usize,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string().to_lowercase();

                let kind = match tag_name.as_str() {
                    "p" | "div" | "span" => UnitKind::Paragraph,
                    "h1" => UnitKind::Heading(1),
                    "h2" => UnitKind::Heading(2),
                    "h3" => UnitKind::Heading(3),
                    "h4" => UnitKind::Heading(4),
                    "h5" => UnitKind::Heading(5),
                    "h6" => UnitKind::Heading(6),
                    "li" => UnitKind::ListItem,
                    "td" | "th" => UnitKind::TableCell,
                    "figcaption" => UnitKind::Caption,
                    "blockquote" => UnitKind::Paragraph,
                    "pre" | "code" => UnitKind::CodeBlock,
                    _ => UnitKind::Other,
                };

                let text = self.extract_text(node);
                if !text.trim().is_empty() && !matches!(kind, UnitKind::Other) {
                    let section = SectionId::new();
                    let unit_id = UnitId::new();
                    let selector = self.build_selector(node, &tag_name);

                    locations.push(UnitLocation {
                        unit_id,
                        file_path: file_path.to_string(),
                        xpath: selector.clone(),
                        text_node_index: 0,
                        original_text: text,
                        kind,
                        section,
                    });
                }

                for child in node.children.borrow().iter() {
                    self.walk_xhtml(child, file_path, locations, section_index)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn extract_text(&self, node: &html5ever::rcdom::Handle) -> String {
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

    fn build_selector(&self, node: &html5ever::rcdom::Handle, tag: &str) -> String {
        // Constrói XPath simplificado
        format!("//{}", tag)
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

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
            parts: parts.clone(),
            content_parts: parts,
            relationships: std::collections::HashMap::new(),
            content_types: Default::default(),
        }
    }
}