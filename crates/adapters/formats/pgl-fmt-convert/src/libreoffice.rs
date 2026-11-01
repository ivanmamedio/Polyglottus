//! LibreOffice Converter (headless)

use pgl_app::{FormatId, Confidence, FormatError};
use which;

/// LibreOffice Converter (soffice headless)
pub struct LibreOfficeConverter {
    libreoffice_path: String,
}

impl LibreOfficeConverter {
    pub fn new() -> Result<Self, FormatError> {
        let libreoffice_path = which::which("soffice")
            .or_else(|_| which::which("libreoffice"))
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("libreoffice/soffice not found in PATH".into()))?;

        Ok(Self { libreoffice_path })
    }

    pub fn is_available() -> bool {
        which::which("soffice").is_ok() || which::which("libreoffice").is_ok()
    }

    pub fn extensions() -> &'static [&'static str] {
        &[".doc", ".docx", ".rtf", ".odt", ".ods", ".odp", ".xls", ".xlsx", ".ppt", ".pptx", ".pdf", ".html"]
    }

    pub fn sniff_static(name: &str) -> Confidence {
        let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        let supported = &[".doc", ".docx", ".rtf", ".odt", ".ods", ".odp", ".xls", ".xlsx", ".ppt", ".pptx", ".pdf", ".html"];

        if supported.iter().any(|e| *e == ext_with_dot) {
            return Confidence::high();
        }
        Confidence::none()
    }

    pub fn input_formats() -> &'static [&'static str] {
        &["doc", "docx", "rtf", "odt", "ods", "odp", "xls", "xlsx", "ppt", "pptx", "pdf", "html"]
    }

    pub fn output_formats() -> &'static [&'static str] {
        &["doc", "docx", "rtf", "odt", "ods", "odp", "xls", "xlsx", "ppt", "pptx", "pdf", "html"]
    }
}

use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libreoffice_extensions() {
        assert!(!LibreOfficeConverter::extensions().is_empty());
    }

    #[test]
    fn libreoffice_sniff() {
        assert_eq!(LibreOfficeConverter::sniff_static("test.docx"), Confidence::high());
        assert_eq!(LibreOfficeConverter::sniff_static("test.odt"), Confidence::high());
        assert_eq!(LibreOfficeConverter::sniff_static("test.unknown"), Confidence::none());
    }
}