//! Motor de "patch no lugar" para formatos baseados em ZIP+XML
//!
//! Estratégia: abre o container original (ZIP), localiza os nós de texto
//! traduzíveis via XPath/seletores, substitui pelo texto traduzido e
//! regrava tudo o mais **sem alterar**.
//!
//! Um único motor atende uma família inteira de formatos:
//! - Pacotes OOXML (OPC): DOCX, PPTX, XLSX
//! - ODF: ODT, ODP, ODS
//! - E-books: EPUB (2 e 3), FB2
//! - HTML/XHTML

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use uuid::Uuid;

pub mod opc;

use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef, Language};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_unit_id, make_source_ref, make_section_id, default_zip_fidelity, sniff_zip_format};
use crate::opc::OpcPackage;

/// Converte FormatId para OutputFormat
fn format_id_to_output_format(id: FormatId) -> OutputFormat {
    match id.0 {
        "epub" => OutputFormat::EPUB,
        "docx" => OutputFormat::DOCX,
        "pptx" => OutputFormat::PPTX,
        "xlsx" => OutputFormat::XLSX,
        "odt" => OutputFormat::ODT,
        "odp" => OutputFormat::ODP,
        "ods" => OutputFormat::ODS,
        "html" => OutputFormat::HTML,
        "txt" => OutputFormat::TXT,
        "md" => OutputFormat::MD,
        "pdf" => OutputFormat::PDF,
        "srt" => OutputFormat::SRT,
        "vtt" => OutputFormat::VTT,
        "xliff" => OutputFormat::XLIFF,
        "tmx" => OutputFormat::TMX,
        "po" => OutputFormat::PO,
        "json-i18n" => OutputFormat::JSON_I18N,
        _ => OutputFormat::SameAsSource,
    }
}

/// Motor de patch genérico para formatos ZIP+XML
pub struct PatchEngine {
    /// Package OPC/ODF/EPUB
    package: OpcPackage,
    /// Mapeamento UnitId -> localização no XML
    unit_locations: Vec<UnitLocation>,
}

/// Localização de uma unidade no XML
#[derive(Debug, Clone)]
pub struct UnitLocation {
    pub unit_id: UnitId,
    pub file_path: String,      // Ex: "word/document.xml"
    pub xpath: String,          // XPath para o nó de texto
    pub text_node_index: usize, // Índice do nó de texto dentro do elemento
    pub original_text: String,
    pub kind: UnitKind,
    pub section: SectionId,
}

impl PatchEngine {
    /// Cria engine a partir de arquivo
    pub fn from_file(path: &Path) -> Result<Self, FormatError> {
        let package = OpcPackage::open(path)?;
        let unit_locations = Self::scan_units(&package)?;
        Ok(Self { package, unit_locations })
    }

    /// Escaneia todas as unidades traduzíveis no package
    fn scan_units(_package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        // Implementação simplificada - placeholder
        Err(FormatError::ParseError("Use format-specific scanner".into()))
    }

    /// Aplica traduções e gera novo arquivo
    pub fn apply_and_save<L: TranslationLookup>(
        &mut self,
        translations: &L,
        output_path: &Path,
    ) -> Result<ExportReport, FormatError> {
        // Para cada unidade traduzida, atualiza o XML correspondente
        for loc in &self.unit_locations {
            if let Some(translated) = translations.get(&loc.unit_id) {
                self.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &translated.raw)?;
            }
        }

        // Salva package
        self.package.save(output_path)?;

        Ok(ExportReport {
            output_path: output_path.display().to_string(),
            format: OutputFormat::SameAsSource,
            units_processed: self.unit_locations.len(),
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(output_path)?.len(),
            export_time_ms: 0,
            warnings: vec![],
        })
    }

    /// Converte para unidades do domínio
    pub fn to_domain_units(&self) -> Vec<Unit> {
        self.unit_locations.iter().map(|loc| {
            let source_ref = make_source_ref(&loc.file_path, &loc.xpath);
            let mut unit = Unit::new(
                source_ref,
                loc.kind,
                MarkedText::new(&loc.original_text),
                loc.section, Uuid::nil(),
            );
            unit.id = loc.unit_id;
            unit
        }).collect()
    }
}

/// Trait para scanners específicos de formato
pub trait FormatScanner: Send + Sync {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError>;
    fn get_text_selector(&self) -> &'static str; // Ex: "w:t", "p", "text()"
    fn get_context_selectors(&self) -> &'static [&'static str]; // Para heading, list, table, etc.
}

/// Registry de scanners por formato
pub struct ScannerRegistry {
    scanners: Vec<Box<dyn FormatScanner>>,
}

impl ScannerRegistry {
    pub fn new() -> Self {
        Self { scanners: Vec::new() }
    }

    pub fn register(&mut self, scanner: Box<dyn FormatScanner>) {
        self.scanners.push(scanner);
    }

    pub fn find_scanner(&self, format_id: FormatId) -> Option<&dyn FormatScanner> {
        self.scanners.iter()
            .find(|s| s.get_text_selector().contains(format_id.0)) // simplificado
            .map(|s| s.as_ref())
    }
}

impl Default for ScannerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Leitor genérico usando PatchEngine
pub struct PatchFormatReader {
    scanner: Arc<dyn FormatScanner>,
    format_id: FormatId,
    extensions: &'static [&'static str],
}

impl PatchFormatReader {
    pub fn new(scanner: Box<dyn FormatScanner>, format_id: FormatId, extensions: &'static [&'static str]) -> Self {
        Self {
            scanner: scanner.into(),
            format_id,
            extensions,
        }
    }
}

#[async_trait]
impl FormatReader for PatchFormatReader {
    fn id(&self) -> FormatId {
        self.format_id
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), self.extensions)
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        let package = OpcPackage::open(path)?;
        let locations = self.scanner.scan(&package)?;

        let units: Vec<Unit> = locations.into_iter().map(|loc| {
            let source_ref = make_source_ref(&loc.file_path, &loc.xpath);
            let mut unit = Unit::new(
                source_ref,
                loc.kind,
                MarkedText::new(&loc.original_text),
                loc.section, Uuid::nil(),
            );
            unit.id = loc.unit_id;
            unit
        }).collect();

        let fidelity = default_zip_fidelity()
            .with_preserved("Estrutura ZIP+XML original")
            .with_preserved("Formatação e estilos")
            .with_preserved("Imagens e assets")
            .with_not_preserved("Macros/VBA")
            .with_warning("Patch engine: alterações em layout complexo podem quebrar");

        let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new())).unwrap();

        Ok(Box::new(ZipSourceDocument::new(
            empty_archive,
            units,
            vec![],
            fidelity,
            Default::default(),
        )))
    }
}

/// Writer genérico usando PatchEngine
pub struct PatchFormatWriter {
    format_id: FormatId,
}

impl PatchFormatWriter {
    pub fn new(format_id: FormatId) -> Self {
        Self { format_id }
    }
}

#[async_trait]
impl FormatWriter for PatchFormatWriter {
    fn id(&self) -> FormatId {
        self.format_id
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        let mut engine = PatchEngine::from_file(source)?;

        if options.bilingual {
            // Bilíngue: adiciona tradução após original
            for loc in &engine.unit_locations {
                if let Some(translated) = translations.get(&loc.unit_id) {
                    let bilingual = format!("{} ⟦sep⟧ {}", loc.original_text, translated.raw);
                    engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &bilingual)?;
                }
            }
        } else {
            // Aplica traduções
            for loc in &engine.unit_locations {
                if let Some(translated) = translations.get(&loc.unit_id) {
                    engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &translated.raw)?;
                }
            }
        }

        engine.package.save(out)?;

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: format_id_to_output_format(self.id()),
            units_processed: engine.unit_locations.len(),
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec!["Patch engine: alguns layouts complexos podem não ser preservados".into()],
        })
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::SameAsSource]
    }
}

/// ODT Reader usando patch engine
pub struct OdtReader {
    reader: PatchFormatReader,
}

impl OdtReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(OdfScanner),
                FormatId::ODT,
                &[".odt"],
            ),
        }
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
        self.reader.open(path).await
    }
}

/// ODT Writer
pub struct OdtWriter {
    writer: PatchFormatWriter,
}

impl OdtWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::ODT),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODT]
    }
}

/// ODP Reader
pub struct OdpReader {
    reader: PatchFormatReader,
}

impl OdpReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(OdfScanner),
                FormatId::ODP,
                &[".odp"],
            ),
        }
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
        self.reader.open(path).await
    }
}

/// ODP Writer
pub struct OdpWriter {
    writer: PatchFormatWriter,
}

impl OdpWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::ODP),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODP]
    }
}

/// ODS Reader
pub struct OdsReader {
    reader: PatchFormatReader,
}

impl OdsReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(OdfScanner),
                FormatId::ODS,
                &[".ods"],
            ),
        }
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
        self.reader.open(path).await
    }
}

/// ODS Writer
pub struct OdsWriter {
    writer: PatchFormatWriter,
}

impl OdsWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::ODS),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::ODS]
    }
}

/// DOCX Reader
pub struct DocxReader {
    reader: PatchFormatReader,
}

impl DocxReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(DocxScanner),
                FormatId::DOCX,
                &[".docx"],
            ),
        }
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

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".docx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        self.reader.open(path).await
    }
}

/// DOCX Writer
pub struct DocxWriter {
    writer: PatchFormatWriter,
}

impl DocxWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::DOCX),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::DOCX]
    }
}

/// PPTX Reader
pub struct PptxReader {
    reader: PatchFormatReader,
}

impl PptxReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(PptxScanner),
                FormatId::PPTX,
                &[".pptx"],
            ),
        }
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

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".pptx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        self.reader.open(path).await
    }
}

/// PPTX Writer
pub struct PptxWriter {
    writer: PatchFormatWriter,
}

impl PptxWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::PPTX),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::PPTX]
    }
}

/// XLSX Reader
pub struct XlsxReader {
    reader: PatchFormatReader,
}

impl XlsxReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(XlsxScanner),
                FormatId::XLSX,
                &[".xlsx"],
            ),
        }
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

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".xlsx"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        self.reader.open(path).await
    }
}

/// XLSX Writer
pub struct XlsxWriter {
    writer: PatchFormatWriter,
}

impl XlsxWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::XLSX),
        }
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
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::XLSX]
    }
}

/// EPUB Reader
pub struct EpubReader {
    reader: PatchFormatReader,
}

impl EpubReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(EpubScanner),
                FormatId::EPUB,
                &[".epub"],
            ),
        }
    }
}

#[async_trait]
impl FormatReader for EpubReader {
    fn id(&self) -> FormatId {
        FormatId::EPUB
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".epub"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        self.reader.open(path).await
    }
}

/// EPUB Writer
pub struct EpubWriter {
    writer: PatchFormatWriter,
}

impl EpubWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::EPUB),
        }
    }
}

#[async_trait]
impl FormatWriter for EpubWriter {
    fn id(&self) -> FormatId {
        FormatId::EPUB
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::EPUB]
    }
}

/// HTML Reader
pub struct HtmlReader {
    reader: PatchFormatReader,
}

impl HtmlReader {
    pub fn new() -> Self {
        Self {
            reader: PatchFormatReader::new(
                Box::new(HtmlScanner),
                FormatId::HTML,
                &[".html", ".htm", ".xhtml"],
            ),
        }
    }
}

#[async_trait]
impl FormatReader for HtmlReader {
    fn id(&self) -> FormatId {
        FormatId::HTML
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".html", ".htm", ".xhtml"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".html", ".htm", ".xhtml"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        self.reader.open(path).await
    }
}

/// HTML Writer
pub struct HtmlWriter {
    writer: PatchFormatWriter,
}

impl HtmlWriter {
    pub fn new() -> Self {
        Self {
            writer: PatchFormatWriter::new(FormatId::HTML),
        }
    }
}

#[async_trait]
impl FormatWriter for HtmlWriter {
    fn id(&self) -> FormatId {
        FormatId::HTML
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        self.writer.write(source, translations, &ExportOptions::default(), out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::HTML]
    }
}

/// Scanner ODF genérico (placeholder)
pub struct OdfScanner;

impl FormatScanner for OdfScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("ODF scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "text:p, text:h, text:span, text:a"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["text:h", "text:list-item", "table:table-cell", "draw:text-box"]
    }
}

/// Scanner DOCX genérico
pub struct DocxScanner;

impl FormatScanner for DocxScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("DOCX scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "w:t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["w:pStyle", "w:tbl", "w:drawing"]
    }
}

/// Scanner PPTX genérico
pub struct PptxScanner;

impl FormatScanner for PptxScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("PPTX scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "a:t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["p:sp", "p:txBody"]
    }
}

/// Scanner XLSX genérico
pub struct XlsxScanner;

impl FormatScanner for XlsxScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("XLSX scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "si > t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["sheetData > row > c"]
    }
}

/// Scanner EPUB
pub struct EpubScanner;

impl FormatScanner for EpubScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("EPUB scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption"]
    }
}

/// Scanner HTML
pub struct HtmlScanner;

impl FormatScanner for HtmlScanner {
    fn scan(&self, _package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        Err(FormatError::ParseError("HTML scanner not implemented".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "p, h1, h2, h3, h4, h5, h6, li, td, th, caption, figcaption, blockquote, pre, code, article, section"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "figcaption", "blockquote"]
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
        let _ = DocxReader::new();
        let _ = PptxReader::new();
        let _ = XlsxReader::new();
        let _ = EpubReader::new();
        let _ = HtmlReader::new();
    }

    #[test]
    fn writers_creation() {
        let _ = OdtWriter::new();
        let _ = OdpWriter::new();
        let _ = OdsWriter::new();
        let _ = DocxWriter::new();
        let _ = PptxWriter::new();
        let _ = XlsxWriter::new();
        let _ = EpubWriter::new();
        let _ = HtmlWriter::new();
    }
}