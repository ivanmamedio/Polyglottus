//! Adaptador ODF (ODT, ODP, ODS)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod odt;
pub mod odp;
pub mod ods;

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::sniff_zip_format;

/// Leitor ODT
pub struct OdtReader {
    scanner: Arc<odt::OdtScanner>,
}

impl OdtReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(odt::OdtScanner::new()) }
    }
}

impl FormatReader for OdtReader {
    fn id(&self) -> FormatId {
        FormatId::ODT
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".odt"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer ODT
pub struct OdtWriter;

impl OdtWriter {
    pub fn new() -> Self {
        Self
    }
}

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
pub struct OdpReader {
    scanner: Arc<odp::OdpScanner>,
}

impl OdpReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(odp::OdpScanner::new()) }
    }
}

impl FormatReader for OdpReader {
    fn id(&self) -> FormatId {
        FormatId::ODP
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".odp"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer ODP
pub struct OdpWriter;

impl OdpWriter {
    pub fn new() -> Self {
        Self
    }
}

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
pub struct OdsReader {
    scanner: Arc<ods::OdsScanner>,
}

impl OdsReader {
    pub fn new() -> Self {
        Self { scanner: Arc::new(ods::OdsScanner::new()) }
    }
}

impl FormatReader for OdsReader {
    fn id(&self) -> FormatId {
        FormatId::ODS
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".ods"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.scanner.open(path).await
    }
}

/// Writer ODS
pub struct OdsWriter;

impl OdsWriter {
    pub fn new() -> Self {
        Self
    }
}

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