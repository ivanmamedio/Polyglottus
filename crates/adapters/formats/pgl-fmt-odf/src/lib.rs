//! Adaptador ODF (ODT, ODP, ODS)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod odt;
pub mod odp;
pub mod ods;

use std::path::Path;
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, Language};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_source_ref, default_zip_fidelity, sniff_zip_format};
use zip::ZipArchive;

fn create_empty_zip() -> Vec<u8> {
    use zip::ZipWriter;
    use std::io::Cursor;
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}

/// Leitor ODT
pub struct OdtReader;

impl OdtReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for OdtReader {
    fn id(&self) -> FormatId {
        FormatId::ODT
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odt"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".odt"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        odt::open_odt(path).await
    }
}

/// Writer ODT
pub struct OdtWriter;

impl OdtWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatWriter for OdtWriter {
    fn id(&self) -> FormatId {
        FormatId::ODT
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        odt::write_odt(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODT]
    }
}

/// Leitor ODP
pub struct OdpReader;

impl OdpReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for OdpReader {
    fn id(&self) -> FormatId {
        FormatId::ODP
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odp"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".odp"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        odp::open_odp(path).await
    }
}

/// Writer ODP
pub struct OdpWriter;

impl OdpWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatWriter for OdpWriter {
    fn id(&self) -> FormatId {
        FormatId::ODP
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        odp::write_odp(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODP]
    }
}

/// Leitor ODS
pub struct OdsReader;

impl OdsReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for OdsReader {
    fn id(&self) -> FormatId {
        FormatId::ODS
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".ods"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".ods"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        ods::open_ods(path).await
    }
}

/// Writer ODS
pub struct OdsWriter;

impl OdsWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatWriter for OdsWriter {
    fn id(&self) -> FormatId {
        FormatId::ODS
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        ods::write_ods(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODS]
    }
}

impl ZipFormatReader for OdtReader {
    fn format_id(&self) -> FormatId {
        FormatId::ODT
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odt"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.text"]
    }
}

impl ZipFormatReader for OdpReader {
    fn format_id(&self) -> FormatId {
        FormatId::ODP
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odp"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.presentation"]
    }
}

impl ZipFormatReader for OdsReader {
    fn format_id(&self) -> FormatId {
        FormatId::ODS
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".ods"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/vnd.oasis.opendocument.spreadsheet"]
    }
}

impl ZipFormatWriter for OdtWriter {
    fn format_id(&self) -> FormatId {
        FormatId::ODT
    }
}

impl ZipFormatWriter for OdpWriter {
    fn format_id(&self) -> FormatId {
        FormatId::ODP
    }
}

impl ZipFormatWriter for OdsWriter {
    fn format_id(&self) -> FormatId {
        FormatId::ODS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_creation() {
        let _ = OdtReader::new();
        let _ = OdpReader::new();
        let _ = OdsReader::new();
    }

    #[test]
    fn writers_creation() {
        let _ = OdtWriter::new();
        let _ = OdpWriter::new();
        let _ = OdsWriter::new();
    }
}