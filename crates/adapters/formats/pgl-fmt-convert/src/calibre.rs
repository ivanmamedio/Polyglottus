//! Calibre Converter (ebook-convert)

use super::{ExternalConverter, FormatError};
use pgl_app::{FormatId, Confidence, ExportOptions, ExportReport, TranslationLookup, OutputFormat, SourceDocument};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use zip::ZipArchive;

/// Conversor Calibre (ebook-convert)
pub struct CalibreConverter {
    ebook_convert_path: String,
    supported_formats: HashMap<FormatId, FormatInfo>,
}

struct FormatInfo {
    extensions: &'static [&'static str],
    input_only: bool,
}

impl CalibreConverter {
    pub fn new() -> Result<Self, FormatError> {
        let ebook_convert_path = which::which("ebook-convert")
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("ebook-convert not found in PATH".into()))?;

        let mut supported = HashMap::new();
        supported.insert(FormatId("mobi"), FormatInfo { extensions: &[".mobi"], input_only: true });
        supported.insert(FormatId("azw3"), FormatInfo { extensions: &[".azw3"], input_only: true });
        supported.insert(FormatId("lit"), FormatInfo { extensions: &[".lit"], input_only: true });
        supported.insert(FormatId("lrf"), FormatInfo { extensions: &[".lrf"], input_only: true });
        supported.insert(FormatId("pdb"), FormatInfo { extensions: &[".pdb"], input_only: true });
        supported.insert(FormatId("rb"), FormatInfo { extensions: &[".rb"], input_only: true });
        supported.insert(FormatId("tcr"), FormatInfo { extensions: &[".tcr"], input_only: true });
        // Saída
        supported.insert(FormatId::EPUB, FormatInfo { extensions: &[".epub"], input_only: false });
        supported.insert(FormatId::DOCX, FormatInfo { extensions: &[".docx"], input_only: false });
        supported.insert(FormatId::HTML, FormatInfo { extensions: &[".html"], input_only: false });
        supported.insert(FormatId::TXT, FormatInfo { extensions: &[".txt"], input_only: false });
        supported.insert(FormatId("rtf"), FormatInfo { extensions: &[".rtf"], input_only: false });

        Ok(Self { ebook_convert_path, supported_formats: supported })
    }
}

#[async_trait::async_trait]
impl ExternalConverter for CalibreConverter {
    fn format_id(&self) -> FormatId {
        FormatId("calibre")
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".mobi", ".azw3", ".lit", ".lrf", ".pdb", ".rb", ".tcr"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        for (_, info) in &self.supported_formats {
            if info.extensions.iter().any(|e| *e == ext_with_dot) {
                return Confidence::high();
            }
        }
        Confidence::none()
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        let temp_dir = tempfile::tempdir()?;
        let epub_path = temp_dir.path().join("converted.epub");

        let status = Command::new(&self.ebook_convert_path)
            .args([
                path.to_str().unwrap(),
                epub_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::ParseError(format!("ebook-convert failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::ParseError("ebook-convert conversion failed".into()));
        }

        crate::epub::open_epub_as_pivot(&epub_path).await
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        let temp_dir = tempfile::tempdir()?;
        let epub_path = temp_dir.path().join("input.epub");

        crate::epub::write_epub_as_pivot(source, translations, options, &epub_path).await?;

        let status = Command::new(&self.ebook_convert_path)
            .args([
                epub_path.to_str().unwrap(),
                out.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::WriteError(format!("ebook-convert failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::WriteError("ebook-convert conversion failed".into()));
        }

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: options.output_format,
            units_processed: 0,
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec!["Converted via Calibre ebook-convert - fidelity may vary".into()],
        })
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::EPUB, OutputFormat::DOCX, OutputFormat::HTML, OutputFormat::TXT, OutputFormat::RTF]
    }

    fn is_available(&self) -> bool {
        which::which(&self.ebook_convert_path).is_ok()
    }
}

pub mod epub {
    use super::*;

    pub async fn open_epub_as_pivot(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        let reader = crate::super::super::epub::EpubReader::new();
        reader.open(path).await
    }

    pub async fn write_epub_as_pivot(
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        let writer = crate::super::super::epub::EpubWriter::new();
        writer.write(source, translations, options, out).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibre_converter_creation() {
        let _ = CalibreConverter::new();
    }
}