//! Scanner e leitor para HTML/XHTML

use super::{FormatScanner, UnitLocation, UnitKind, SectionId};
use crate::opc::OpcPackage;
use pgl_domain::{UnitId, SourceRef};
use pgl_app::FormatError;
use std::sync::Arc;

/// Scanner HTML genérico
pub struct HtmlScanner;

impl FormatScanner for HtmlScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        let mut locations = Vec::new();

        // Processa todos os arquivos HTML/XHTML
        for (file_path, content) in &package.content_parts {
            if self.is_html_file(file_path) {
                self.scan_html(content, file_path, &mut locations)?;
            }
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code, article, section"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption", "blockquote"]
    }
}

impl HtmlScanner {
    fn is_html_file(&self, path: &str) -> bool {
        path.ends_with(".html") || path.ends_with(".htm") || path.ends_with(".xhtml")
    }

    fn scan_html(&self, content: &[u8], file_path: &str, locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        self.walk_html(&doc.document, file_path, locations)
    }

    fn walk_html(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string().to_lowercase();

                // Skip script, style, noscript, template
                if matches!(tag_name.as_str(), "script" | "style" | "noscript" | "template" | "head" | "meta" | "link" | "title") {
                    return Ok(());
                }

                let kind = match tag_name.as_str() {
                    "p" | "div" | "article" | "section" | "blockquote" => UnitKind::Paragraph,
                    "h1" => UnitKind::Heading(1),
                    "h2" => UnitKind::Heading(2),
                    "h3" => UnitKind::Heading(3),
                    "h4" => UnitKind::Heading(4),
                    "h5" => UnitKind::Heading(5),
                    "h6" => UnitKind::Heading(6),
                    "li" => UnitKind::ListItem,
                    "td" | "th" => UnitKind::TableCell,
                    "figcaption" => UnitKind::Caption,
                    "pre" | "code" => UnitKind::CodeBlock,
                    "dt" | "dd" => UnitKind::Paragraph,
                    _ => UnitKind::Other,
                };

                let text = self.extract_text(node);
                if !text.trim().is_empty() && !matches!(kind, UnitKind::Other) {
                    let section = SectionId::new();
                    let unit_id = UnitId::new();

                    locations.push(UnitLocation {
                        unit_id,
                        file_path: file_path.to_string(),
                        xpath: format!("//{}", tag_name),
                        text_node_index: 0,
                        original_text: text,
                        kind,
                        section,
                    });
                }

                for child in node.children.borrow().iter() {
                    self.walk_html(child, file_path, locations)?;
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
}

/// Leitor HTML
pub struct HtmlReader {
    scanner: Arc<HtmlScanner>,
}

impl HtmlReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(HtmlScanner) }
    }
}

impl pgl_fmt_core::ZipFormatReader for HtmlReader {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::HTML
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".html", ".htm", ".xhtml"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["text/html", "application/xhtml+xml"]
    }
}

impl pgl_app::FormatReader for HtmlReader {
    fn id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::HTML
    }

    fn sniff(&self, name: &str, head: &[u8]) -> pgl_app::Confidence {
        let ext_match = name.ends_with(".html") || name.ends_with(".htm") || name.ends_with(".xhtml");
        let html_match = head.windows(4).any(|w| w == b"<htm") || head.windows(5).any(|w| w == b"<!DOCT") || head.windows(7).any(|w| w == b"<html");

        match (ext_match, html_match) {
            (true, true) => pgl_app::Confidence::certain(),
            (true, false) => pgl_app::Confidence::high(),
            (false, true) => pgl_app::Confidence::medium(),
            (false, false) => pgl_app::Confidence::none(),
        }
    }

    async fn open(&self, path: &std::path::Path) -> Result<Box<dyn pgl_app::SourceDocument>, pgl_app::FormatError> {
        let content = std::fs::read(path)?;

        // Cria package OPC simulado para HTML standalone
        let mut package = OpcPackage {
            parts: std::collections::HashMap::new(),
            content_parts: std::collections::HashMap::new(),
            relationships: std::collections::HashMap::new(),
            content_types: Default::default(),
        };

        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("index.html");
        package.content_parts.insert(file_name.to_string(), content);

        let scanner = HtmlScanner;
        let locations = scanner.scan(&package)?;

        let units: Vec<pgl_domain::Unit> = locations.into_iter().map(|loc| {
            let source_ref = SourceRef {
                internal_path: loc.file_path.clone(),
                selector: loc.xpath,
                offset: None,
                length: None,
            };
            let mut unit = pgl_domain::Unit::new(
                source_ref,
                loc.kind,
                pgl_domain::MarkedText::new(&loc.original_text),
                loc.section,
            );
            unit.id = loc.unit_id;
            unit
        }).collect();

        let fidelity = pgl_fmt_core::default_zip_fidelity()
            .with_preserved("Estrutura HTML")
            .with_preserved("CSS e scripts (não modificados)")
            .with_preserved("Imagens e assets")
            .with_not_preserved("JavaScript executável");

        let doc = pgl_fmt_core::ZipSourceDocument::new(
            // Usa archive vazio para HTML standalone
            zip::ZipArchive::new(std::io::Cursor::new(Vec::new())).unwrap(),
            units,
            vec![],
            fidelity,
            Default::default(),
        );

        Ok(Box::new(doc))
    }
}

/// Writer HTML
pub struct HtmlWriter;

impl HtmlWriter {
    pub fn new() -> Self {
        Self
    }
}

impl pgl_fmt_core::ZipFormatWriter for HtmlWriter {
    fn format_id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::HTML
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn html_scanner_basic() {
        let scanner = HtmlScanner;
        let mut package = OpcPackage {
            parts: std::collections::HashMap::new(),
            content_parts: std::collections::HashMap::new(),
            relationships: std::collections::HashMap::new(),
            content_types: Default::default(),
        };

        package.content_parts.insert("index.html".to_string(), br#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
  <h1>Main Title</h1>
  <p>First paragraph.</p>
  <div>Second paragraph in div.</div>
  <ul>
    <li>Item 1</li>
    <li>Item 2</li>
  </ul>
  <table>
    <tr><td>Cell 1</td><td>Cell 2</td></tr>
  </table>
  <script>console.log("ignored")</script>
  <style>body { color: red; }</style>
</body>
</html>"#.to_vec());

        let locations = scanner.scan(&package).unwrap();
        assert!(!locations.is_empty());

        // Deve encontrar elementos traduzíveis
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::Heading(1))));
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::Paragraph)));
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::ListItem)));
        assert!(locations.iter().any(|l| matches!(l.kind, UnitKind::TableCell)));

        // NÃO deve encontrar script/style
        assert!(!locations.iter().any(|l| l.original_text.contains("console.log")));
        assert!(!locations.iter().any(|l| l.original_text.contains("color: red")));
    }
}