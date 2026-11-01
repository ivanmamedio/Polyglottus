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
        Err(FormatError::ParseError("HTML scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code, article, section"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption", "blockquote"]
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

#[async_trait::async_trait]
impl pgl_app::FormatReader for HtmlReader {
    fn id(&self) -> pgl_app::FormatId {
        pgl_app::FormatId::HTML
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".html", ".htm", ".xhtml"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> pgl_app::Confidence {
        let ext_match = name.ends_with(".html") || name.ends_with(".htm") || name.ends_with(".xhtml");
        let html_match = false; // Placeholder

        match (ext_match, html_match) {
            (true, true) => pgl_app::Confidence::certain(),
            (true, false) => pgl_app::Confidence::high(),
            (false, true) => pgl_app::Confidence::medium(),
            (false, false) => pgl_app::Confidence::none(),
        }
    }

    async fn open(&self, path: &std::path::Path) -> Result<Box<dyn pgl_app::SourceDocument + 'static>, pgl_app::FormatError> {
        Err(pgl_app::FormatError::ParseError("HTML reader not implemented".into()))
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

    #[test]
    fn html_placeholder() {
        assert!(true);
    }
}