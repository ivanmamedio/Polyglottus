//! Adaptador OOXML (DOCX, PPTX, XLSX)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod docx;
pub mod pptx;
pub mod xlsx;

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::sniff_zip_format;

/// Leitor DOCX
pub struct DocxReader {
    scanner: Arc<docx::DocxScanner>,
}

impl DocxReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(docx::DocxScanner::new()) }
    }
}

impl FormatReader for DocxReader {
    fn id(&self) -> FormatId {
        FormatId::DOCX
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".docx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer DOCX
pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for DocxWriter {
    fn id(&self) -> FormatId {
        FormatId::DOCX
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        docx::write_docx(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::DOCX]
    }
}

/// Leitor PPTX
pub struct PptxReader {
    scanner: Arc<pptx::PptxScanner>,
}

impl PptxReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(pptx::PptxScanner::new()) }
    }
}

impl FormatReader for PptxReader {
    fn id(&self) -> FormatId {
        FormatId::PPTX
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".pptx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer PPTX
pub struct PptxWriter;

impl PptxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for PptxWriter {
    fn id(&self) -> FormatId {
        FormatId::PPTX
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        pptx::write_pptx(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::PPTX]
    }
}

/// Leitor XLSX
pub struct XlsxReader {
    scanner: Arc<xlsx::XlsxScanner>,
}

impl XlsxReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(xlsx::XlsxScanner::new()) }
    }
}

impl FormatReader for XlsxReader {
    fn id(&self) -> FormatId {
        FormatId::XLSX
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".xlsx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer XLSX
pub struct XlsxWriter;

impl XlsxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for XlsxWriter {
    fn id(&self) -> FormatId {
        FormatId::XLSX
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        xlsx::write_xlsx(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        false // Planilhas bilíngues não fazem sentido típico
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::XLSX]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_creation() {
        let _ = DocxReader::new();
        let _ = PptxReader::new();
        let _ = XlsxReader::new();
    }

    #[test]
    fn writers_creation() {
        let _ = DocxWriter::new();
        let _ = PptxWriter::new();
        let _ = XlsxWriter::new();
    }
}