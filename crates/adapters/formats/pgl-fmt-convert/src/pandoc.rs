//! Pandoc Converter

use pgl_app::{FormatId, Confidence, FormatError};
use std::path::Path;
use which;

/// Conversor Pandoc
pub struct PandocConverter {
    pandoc_path: String,
}

impl PandocConverter {
    pub fn new() -> Result<Self, FormatError> {
        let pandoc_path = which::which("pandoc")
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("pandoc not found in PATH".into()))?;

        Ok(Self { pandoc_path })
    }

    pub fn is_available() -> bool {
        which::which("pandoc").is_ok()
    }

    pub fn extensions() -> &'static [&'static str] {
        &[".mobi", ".azw3", ".rtf", ".doc", ".tex", ".latex", ".rst", ".textile", ".org", ".djvu", ".odt"]
    }

    pub fn sniff_static(name: &str) -> Confidence {
        let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        let supported = &[".mobi", ".azw3", ".rtf", ".doc", ".tex", ".latex", ".rst", ".textile", ".org", ".djvu", ".odt", ".epub", ".docx", ".html", ".md", ".txt", ".odt"];

        if supported.iter().any(|e| *e == ext_with_dot) {
            return Confidence::high();
        }
        Confidence::none()
    }

    pub fn input_formats() -> &'static [&'static str] {
        &["mobi", "azw3", "rtf", "doc", "latex", "rst", "textile", "org", "djvu", "odt"]
    }

    pub fn output_formats() -> &'static [&'static str] {
        &["epub", "docx", "html", "markdown", "plain", "odt"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pandoc_extensions() {
        assert!(!PandocConverter::extensions().is_empty());
    }

    #[test]
    fn pandoc_sniff() {
        assert_eq!(PandocConverter::sniff_static("test.mobi"), Confidence::high());
        assert_eq!(PandocConverter::sniff_static("test.docx"), Confidence::high());
        assert_eq!(PandocConverter::sniff_static("test.unknown"), Confidence::none());
    }
}