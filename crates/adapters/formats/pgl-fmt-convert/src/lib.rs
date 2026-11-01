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
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, DocumentMetadata, OutputFormat};
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

/// Tipos de formatos suportados via conversão
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConvertFormat {
    // Formatos de entrada (converter PARA EPUB)
    Mobi,
    Azw3,
    Rtf,
    Doc,
    Latex,
    Rst,
    Textile,
    Org,
    Djvu,
    // Formatos de saída (converter DE EPUB)
    Epub,
    Docx,
    Html,
    Md,
    Txt,
    Odt,
}

impl ConvertFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConvertFormat::Mobi => "mobi",
            ConvertFormat::Azw3 => "azw3",
            ConvertFormat::Rtf => "rtf",
            ConvertFormat::Doc => "doc",
            ConvertFormat::Latex => "latex",
            ConvertFormat::Rst => "rst",
            ConvertFormat::Textile => "textile",
            ConvertFormat::Org => "org",
            ConvertFormat::Djvu => "djvu",
            ConvertFormat::Epub => "epub",
            ConvertFormat::Docx => "docx",
            ConvertFormat::Html => "html",
            ConvertFormat::Md => "markdown",
            ConvertFormat::Txt => "plain",
            ConvertFormat::Odt => "odt",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            ConvertFormat::Mobi => &[".mobi"],
            ConvertFormat::Azw3 => &[".azw3"],
            ConvertFormat::Rtf => &[".rtf"],
            ConvertFormat::Doc => &[".doc"],
            ConvertFormat::Latex => &[".tex", ".latex"],
            ConvertFormat::Rst => &[".rst"],
            ConvertFormat::Textile => &[".textile"],
            ConvertFormat::Org => &[".org"],
            ConvertFormat::Djvu => &[".djvu"],
            ConvertFormat::Epub => &[".epub"],
            ConvertFormat::Docx => &[".docx"],
            ConvertFormat::Html => &[".html", ".htm"],
            ConvertFormat::Md => &[".md", ".markdown"],
            ConvertFormat::Txt => &[".txt"],
            ConvertFormat::Odt => &[".odt"],
        }
    }

    pub fn is_input_only(&self) -> bool {
        matches!(self,
            ConvertFormat::Mobi | ConvertFormat::Azw3 | ConvertFormat::Rtf |
            ConvertFormat::Doc | ConvertFormat::Latex | ConvertFormat::Rst |
            ConvertFormat::Textile | ConvertFormat::Org | ConvertFormat::Djvu
        )
    }
}

/// Tipos de conversores disponíveis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConverterType {
    Pandoc,
    Calibre,
    LibreOffice,
}

impl ConverterType {
    pub fn format_id(&self) -> FormatId {
        match self {
            ConverterType::Pandoc => FormatId("pandoc"),
            ConverterType::Calibre => FormatId("calibre"),
            ConverterType::LibreOffice => FormatId("libreoffice"),
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            ConverterType::Pandoc => pandoc::PandocConverter::extensions(),
            ConverterType::Calibre => calibre::CalibreConverter::extensions(),
            ConverterType::LibreOffice => libreoffice::LibreOfficeConverter::extensions(),
        }
    }

    pub fn sniff(&self, name: &str) -> Confidence {
        match self {
            ConverterType::Pandoc => pandoc::PandocConverter::sniff_static(name),
            ConverterType::Calibre => calibre::CalibreConverter::sniff_static(name),
            ConverterType::LibreOffice => libreoffice::LibreOfficeConverter::sniff_static(name),
        }
    }
}

/// Registry de conversores disponíveis
pub struct ConverterRegistry {
    has_pandoc: bool,
    has_calibre: bool,
    has_libreoffice: bool,
}

impl ConverterRegistry {
    pub fn new() -> Self {
        Self {
            has_pandoc: pandoc::PandocConverter::is_available(),
            has_calibre: calibre::CalibreConverter::is_available(),
            has_libreoffice: libreoffice::LibreOfficeConverter::is_available(),
        }
    }

    pub fn has_pandoc(&self) -> bool { self.has_pandoc }
    pub fn has_calibre(&self) -> bool { self.has_calibre }
    pub fn has_libreoffice(&self) -> bool { self.has_libreoffice }

    pub fn available(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.has_pandoc { v.push("pandoc"); }
        if self.has_calibre { v.push("calibre"); }
        if self.has_libreoffice { v.push("libreoffice"); }
        v
    }
}

impl Default for ConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper para usar conversores com FormatReader
pub struct ConvertReader {
    converter_type: ConverterType,
}

impl ConvertReader {
    pub fn new(converter_type: ConverterType) -> Self {
        Self { converter_type }
    }
}

#[async_trait]
impl FormatReader for ConvertReader {
    fn id(&self) -> FormatId {
        self.converter_type.format_id()
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.converter_type.extensions()
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        self.converter_type.sniff(name)
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        Err(FormatError::ParseError(format!("Converter not implemented: {:?}", self.converter_type)))
    }
}

pub struct ConvertWriter {
    converter_type: ConverterType,
}

impl ConvertWriter {
    pub fn new(converter_type: ConverterType) -> Self {
        Self { converter_type }
    }
}

#[async_trait]
impl FormatWriter for ConvertWriter {
    fn id(&self) -> FormatId {
        self.converter_type.format_id()
    }

    async fn write(
        &self,
        _source: &Path,
        _translations: &dyn TranslationLookup,
        _options: &ExportOptions,
        _out: &Path,
    ) -> Result<ExportReport, FormatError> {
        Err(FormatError::WriteError(format!("Converter not implemented: {:?}", self.converter_type)))
    }

    fn supports_bilingual(&self) -> bool { false }
    fn output_formats(&self) -> &'static [OutputFormat] { &[OutputFormat::SameAsSource] }
}

/// Registry de conversores
pub struct ConverterRegistry2 {
    pandoc: Option<pandoc::PandocConverter>,
    calibre: Option<calibre::CalibreConverter>,
    libreoffice: Option<libreoffice::LibreOfficeConverter>,
}

impl ConverterRegistry2 {
    pub fn new() -> Self {
        Self {
            pandoc: pandoc::PandocConverter::new().ok(),
            calibre: calibre::CalibreConverter::new().ok(),
            libreoffice: libreoffice::LibreOfficeConverter::new().ok(),
        }
    }

    pub fn pandoc(&self) -> Option<&pandoc::PandocConverter> {
        self.pandoc.as_ref()
    }

    pub fn calibre(&self) -> Option<&calibre::CalibreConverter> {
        self.calibre.as_ref()
    }

    pub fn libreoffice(&self) -> Option<&libreoffice::LibreOfficeConverter> {
        self.libreoffice.as_ref()
    }

    pub fn has_pandoc(&self) -> bool { self.pandoc.is_some() }
    pub fn has_calibre(&self) -> bool { self.calibre.is_some() }
    pub fn has_libreoffice(&self) -> bool { self.libreoffice.is_some() }
}

impl Default for ConverterRegistry2 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_creation() {
        let registry = ConverterRegistry::default();
        // Verifica se pelo menos um conversor foi registrado (se disponível)
    }
}