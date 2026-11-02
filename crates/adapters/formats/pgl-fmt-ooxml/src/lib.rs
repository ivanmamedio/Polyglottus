//! Adaptador OOXML (DOCX, PPTX, XLSX)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod docx;
pub mod pptx;
pub mod xlsx;

use std::path::Path;
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, DocumentMetadata, OutputFormat};
use uuid::Uuid;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, Language};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_source_ref, default_zip_fidelity, sniff_zip_format};
use zip::ZipArchive;

pub fn create_empty_zip() -> Vec<u8> {
    use zip::ZipWriter;
    use std::io::Cursor;
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}

/// Leitor DOCX
pub struct DocxReader;

impl DocxReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for DocxReader {
    fn id(&self) -> FormatId {
        FormatId::DOCX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".docx"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), head, &[".docx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        docx::open_docx(path).await
    }
}

/// Writer DOCX
pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
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
pub struct PptxReader;

impl PptxReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for PptxReader {
    fn id(&self) -> FormatId {
        FormatId::PPTX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pptx"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), head, &[".pptx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        pptx::open_pptx(path).await
    }
}

/// Writer PPTX
pub struct PptxWriter;

impl PptxWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
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
pub struct XlsxReader;

impl XlsxReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for XlsxReader {
    fn id(&self) -> FormatId {
        FormatId::XLSX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".xlsx"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), head, &[".xlsx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        xlsx::open_xlsx(path).await
    }
}

/// Writer XLSX
pub struct XlsxWriter;

impl XlsxWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
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
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::XLSX]
    }
}

impl ZipFormatReader for DocxReader {
    fn format_id(&self) -> FormatId {
        FormatId::DOCX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".docx"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"]
    }
}

impl ZipFormatReader for PptxReader {
    fn format_id(&self) -> FormatId {
        FormatId::PPTX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pptx"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.openxmlformats-officedocument.presentationml.presentation"]
    }
}

impl ZipFormatReader for XlsxReader {
    fn format_id(&self) -> FormatId {
        FormatId::XLSX
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".xlsx"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"]
    }
}

impl ZipFormatWriter for DocxWriter {
    fn format_id(&self) -> FormatId {
        FormatId::DOCX
    }
}

impl ZipFormatWriter for PptxWriter {
    fn format_id(&self) -> FormatId {
        FormatId::PPTX
    }
}

impl ZipFormatWriter for XlsxWriter {
    fn format_id(&self) -> FormatId {
        FormatId::XLSX
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