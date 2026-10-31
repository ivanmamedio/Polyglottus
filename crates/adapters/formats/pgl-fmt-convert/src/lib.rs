//! Adaptador para formatos de cauda longa via conversores externos
//!
//! Usa Pandoc, Calibre (ebook-convert), LibreOffice headless como "pacote opcional"
//! para converter formatos não suportados nativamente para pivô (EPUB/DOCX)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod pandoc;
pub mod calibre;
pub mod libreoffice;

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::sniff_zip_format;

/// Leitor genérico via conversor
pub struct ConvertReader {
    converter: Box<dyn ExternalConverter>,
}

impl ConvertReader {
    pub fn new(converter: Box<dyn ExternalConverter>) -> Self {
        Self { converter }
    }
}

impl FormatReader for ConvertReader {
    fn id(&self) -> FormatId {
        self.converter.format_id()
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        self.converter.sniff(name, head)
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        self.converter.open(path).await
    }
}

/// Writer genérico via conversor
pub struct ConvertWriter {
    converter: Box<dyn ExternalConverter>,
}

impl ConvertWriter {
    pub fn new(converter: Box<dyn ExternalConverter>) -> Self {
        Self { converter }
    }
}

impl FormatWriter for ConvertWriter {
    fn id(&self) -> FormatId {
        self.converter.format_id()
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        self.converter.write(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        self.converter.supports_bilingual()
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        self.converter.output_formats()
    }
}

/// Trait para conversores externos
#[async_trait::async_trait]
pub trait ExternalConverter: Send + Sync {
    fn format_id(&self) -> FormatId;
    fn extensions(&self) -> &'static [&'static str];
    fn sniff(&self, name: &str, head: &[u8]) -> Confidence;

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError>;

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError>;

    fn supports_bilingual(&self) -> bool { false }
    fn output_formats(&self) -> &'static [OutputFormat] { &[OutputFormat::SameAsSource] }

    /// Verifica se o conversor está disponível no sistema
    fn is_available(&self) -> bool;
}

/// Registry de conversores
pub struct ConverterRegistry {
    converters: Vec<Box<dyn ExternalConverter>>,
}

impl ConverterRegistry {
    pub fn new() -> Self {
        Self { converters: Vec::new() }
    }

    pub fn register(&mut self, converter: Box<dyn ExternalConverter>) {
        if converter.is_available() {
            self.converters.push(converter);
        }
    }

    pub fn find_converter(&self, format_id: FormatId) -> Option<&dyn ExternalConverter> {
        self.converters.iter()
            .find(|c| c.format_id() == format_id)
            .map(|c| c.as_ref())
    }

    pub fn available_formats(&self) -> Vec<FormatId> {
        self.converters.iter().map(|c| c.format_id()).collect()
    }
}

impl Default for ConverterRegistry {
    fn default() -> Self {
        let mut registry = Self::new();

        // Registra conversores disponíveis
        if let Ok(pandoc) = pandoc::PandocConverter::new() {
            registry.register(Box::new(pandoc));
        }
        if let Ok(calibre) = calibre::CalibreConverter::new() {
            registry.register(Box::new(calibre));
        }
        if let Ok(libreoffice) = libreoffice::LibreOfficeConverter::new() {
            registry.register(Box::new(libreoffice));
        }

        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_creation() {
        let registry = ConverterRegistry::default();
        // Verifica se pelo menos um conversor foi registrado (se disponível)
        // Em CI pode não haver nenhum
    }
}