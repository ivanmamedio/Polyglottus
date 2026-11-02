//! Adaptador PDF

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod reflow;
pub mod overlay;
pub mod ocr_pdf;

use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat, SourceDocument};
use pgl_fmt_core::sniff_zip_format;

/// Leitor PDF (reflow mode - extrai texto para EPUB/DOCX/HTML)
pub struct PdfReflowReader;

impl PdfReflowReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for PdfReflowReader {
    fn id(&self) -> FormatId {
        FormatId::PDF
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pdf"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".pdf"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        reflow::open_pdf_reflow(path).await
    }
}

/// Writer PDF (overlay mode - mantém PDF original com texto traduzido sobreposto)
pub struct PdfOverlayWriter;

impl PdfOverlayWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatWriter for PdfOverlayWriter {
    fn id(&self) -> FormatId {
        FormatId::PDF
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        if options.bilingual {
            return overlay::write_pdf_bilingual(source, translations, options, out).await;
        }
        overlay::write_pdf_overlay(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::PDF]
    }
}

/// Leitor PDF com OCR (para PDFs escaneados)
pub struct PdfOcrReader;

impl PdfOcrReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for PdfOcrReader {
    fn id(&self) -> FormatId {
        FormatId::PDF
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pdf"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        // Mesmo sniff, mas o pipeline decide usar OCR se não houver texto extraível
        sniff_zip_format(Path::new(name), &[".pdf"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        ocr_pdf::open_pdf_ocr(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_creation() {
        let _ = PdfReflowReader::new();
        let _ = PdfOcrReader::new();
    }

    #[test]
    fn writer_creation() {
        let _ = PdfOverlayWriter::new();
    }
}