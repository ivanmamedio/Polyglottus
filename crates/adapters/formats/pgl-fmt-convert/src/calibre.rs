//! Calibre Converter (ebook-convert)

use pgl_app::{FormatId, Confidence, FormatError};
use std::path::Path;
use which;

/// Calibre Converter (ebook-convert)
pub struct CalibreConverter {
    calibre_path: String,
}

impl CalibreConverter {
    pub fn new() -> Result<Self, FormatError> {
        let calibre_path = which::which("ebook-convert")
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("ebook-convert not found in PATH".into()))?;

        Ok(Self { calibre_path })
    }

    pub fn is_available() -> bool {
        which::which("ebook-convert").is_ok()
    }

    pub fn extensions() -> &'static [&'static str] {
        &[".epub", ".mobi", ".azw3", ".rtf", ".docx", ".pdf", ".txt", ".html"]
    }

    pub fn sniff_static(name: &str) -> Confidence {
        let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        let supported = &[".epub", ".mobi", ".azw3", ".rtf", ".docx", ".pdf", ".txt", ".html"];

        if supported.iter().any(|e| *e == ext_with_dot) {
            return Confidence::high();
        }
        Confidence::none()
    }

    pub fn input_formats() -> &'static [&'static str] {
        &["epub", "mobi", "azw3", "rtf", "docx", "pdf", "html"]
    }

    pub fn output_formats() -> &'static [&'static str] {
        &["epub", "mobi", "azw3", "rtf", "docx", "pdf", "txt", "html"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibre_extensions() {
        assert!(!CalibreConverter::extensions().is_empty());
    }

    #[test]
    fn calibre_sniff() {
        assert_eq!(CalibreConverter::sniff_static("test.epub"), Confidence::high());
        assert_eq!(CalibreConverter::sniff_static("test.mobi"), Confidence::high());
        assert_eq!(CalibreConverter::sniff_static("test.unknown"), Confidence::none());
    }
}