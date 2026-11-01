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

pub mod opc;
pub mod odf;
pub mod epub_patch;
pub mod html_patch;

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef, Language};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_unit_id, make_source_ref, make_section_id, default_zip_fidelity, sniff_zip_format};
use crate::opc::OpcPackage;

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
    fn scan_units(package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        let mut locations = Vec::new();

        // Para cada parte XML de conteúdo
        for (file_path, content) in &package.content_parts {
            let doc = html5ever::parse_document(
                html5ever::rcdom::RcDom::default(),
                html5ever::ParseOpts::default(),
            ).from_utf8()
            .read_from(&mut content.as_slice())?;

            // Encontra todos os nós de texto traduzíveis
            Self::find_translatable_nodes(&doc.document, file_path, &mut locations, 0)?;
        }

        Ok(locations)
    }

    fn find_translatable_nodes(
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
        depth: usize,
    ) -> Result<(), FormatError> {
        // Implementação simplificada - em produção usar seletores CSS/XPath
        // específicos por formato (w:t para DOCX, p para EPUB, etc.)

        // Por enquanto, retorna erro para forçar implementação por formato
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
                loc.section,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn patch_engine_creation() {
        // Cria EPUB mínimo para teste
        let mut f = NamedTempFile::new().unwrap();
        write_minimal_epub(&mut f).unwrap();

        let engine = PatchEngine::from_file(f.path());
        // Deve falhar pois scan_units não está implementado genericamente
        assert!(engine.is_err());
    }

    fn write_minimal_epub(f: &mut NamedTempFile) -> std::io::Result<()> {
        use zip::ZipWriter;
        use zip::write::FileOptions;

        let mut zip = ZipWriter::new(f.as_file_mut());
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("mimetype", options)?;
        zip.write_all(b"application/epub+zip")?;

        zip.start_file("META-INF/container.xml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#)?;

        zip.start_file("OEBPS/content.opf", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">test</dc:identifier>
    <dc:title>Test</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#)?;

        zip.start_file("OEBPS/ch1.xhtml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<body><p>Hello world</p></body>
</html>"#)?;

        zip.finish()?;
        Ok(())
    }
}