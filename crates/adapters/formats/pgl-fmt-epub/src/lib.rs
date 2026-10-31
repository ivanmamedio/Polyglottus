//! Adaptador EPUB completo (leitura e escrita)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef, Language};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_unit_id, make_source_ref, make_section_id, default_zip_fidelity, sniff_zip_format};
use pgl_fmt_patch::{PatchEngine, EpubScanner, EpubReader as PatchEpubReader, EpubWriter as PatchEpubWriter, OpcPackage};
use epub::doc::EpubDoc;

/// Leitor EPUB (usa epub crate para parsing robusto + patch engine para unidades)
pub struct EpubReader {
    patch_reader: PatchEpubReader,
}

impl EpubReader {
    pub fn new() -> Self {
        Self { patch_reader: PatchEpubReader::new() }
    }
}

impl FormatReader for EpubReader {
    fn id(&self) -> FormatId {
        FormatId::EPUB
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".epub"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        // Usa epub crate para extrair metadados e estrutura
        let mut doc = EpubDoc::new(path)
            .map_err(|e| FormatError::ParseError(e.to_string()))?;

        let mut package = OpcPackage::open(path)?;

        // Scanner EPUB
        let scanner = EpubScanner;
        let locations = scanner.scan(&package)?;

        // Converte para unidades do domínio
        let mut units = Vec::new();
        for loc in locations {
            let source_ref = make_source_ref(&loc.file_path, &loc.xpath);
            let mut unit = Unit::new(
                source_ref,
                loc.kind,
                MarkedText::new(&loc.original_text),
                loc.section,
            );
            unit.id = loc.unit_id;
            units.push(unit);
        }

        // Assets (imagens, CSS, fontes)
        let assets = self.extract_assets(&mut package)?;

        // Fidelidade
        let fidelity = default_zip_fidelity()
            .with_preserved("Estrutura de capítulos (spine)")
            .with_preserved("Sumário (NCX/NAV)")
            .with_preserved("Metadados (OPF)")
            .with_preserved("Imagens, CSS, fontes")
            .with_preserved("Links internos")
            .with_not_preserved("JavaScript (não suportado em EPUB)")
            .with_warning("Scripts são removidos na exportação");

        // Metadados
        let metadata = DocumentMetadata {
            title: doc.mdata("title").map(|s| s.to_string()),
            author: doc.mdata("creator").map(|s| s.to_string()),
            language: doc.mdata("language").map(|s| Language::new(s)),
            subject: doc.mdata("subject").map(|s| s.to_string()),
            description: doc.mdata("description").map(|s| s.to_string()),
            ..Default::default()
        };

        // Archive vazio (não usado para leitura streaming)
        let archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

        Ok(Box::new(ZipSourceDocument::new(
            archive,
            units,
            assets,
            fidelity,
            metadata,
        )))
    }
}

impl EpubReader {
    fn extract_assets(&self, package: &mut OpcPackage) -> Result<Vec<AssetRef>, FormatError> {
        let mut assets = Vec::new();

        for (name, content) in &package.parts {
            // Skip XML content files
            if name.ends_with(".xml") || name.ends_with(".xhtml") || name.ends_with(".opf") || name.ends_with(".ncx") {
                continue;
            }

            // Assets: imagens, CSS, fontes
            let mime = mime_guess::from_path(name).first_or_octet_stream();
            let hash = format!("{:x}", md5::compute(content));

            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: name.clone(),
                mime_type: mime.to_string(),
                size: content.len() as u64,
                hash,
            });
        }

        Ok(assets)
    }
}

impl ZipFormatReader for EpubReader {
    fn format_id(&self) -> FormatId {
        FormatId::EPUB
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/epub+zip"]
    }
}

/// Writer EPUB (patch no lugar)
pub struct EpubWriter {
    patch_writer: PatchEpubWriter,
}

impl EpubWriter {
    pub fn new() -> Self {
        Self { patch_writer: PatchEpubWriter::new() }
    }
}

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
        // Abre source com patch engine
        let mut engine = PatchEngine::from_file(source)?;

        // Se bilíngue, precisa de lógica especial
        if options.bilingual {
            return self.write_bilingual(source, translations, options, out).await;
        }

        // Aplica traduções
        let report = engine.apply_and_save(translations, out)?;

        // Valida EPUB (opcional)
        if let Err(e) = validate_epub(out) {
            eprintln!("EPUB validation warning: {}", e);
        }

        Ok(report)
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::EPUB]
    }
}

impl EpubWriter {
    async fn write_bilingual(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        // Para bilíngue: cria novo EPUB com original + tradução lado a lado
        // Implementação simplificada - em produção criar estrutura bilíngue
        let mut engine = PatchEngine::from_file(source)?;

        // Duplica conteúdo com tradução
        for loc in &engine.unit_locations {
            if let Some(translated) = translations.get(&loc.unit_id) {
                // Formato bilíngue: "Original | Tradução"
                let bilingual = format!("{} ⟦separator⟧ {}", loc.original_text, translated.raw);
                engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &bilingual)?;
            }
        }

        engine.package.save(out)?;

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: OutputFormat::EPUB,
            units_processed: engine.unit_locations.len(),
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec!["Bilingual format: original and translation separated by marker".into()],
        })
    }
}

impl ZipFormatWriter for EpubWriter {
    fn format_id(&self) -> FormatId {
        FormatId::EPUB
    }
}

/// Valida EPUB com epubcheck (se disponível)
fn validate_epub(path: &Path) -> Result<(), String> {
    // Tenta usar epubcheck se instalado
    if let Ok(output) = std::process::Command::new("epubcheck")
        .arg(path)
        .output()
    {
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }
    Ok(())
}

/// Verifica EPUB válido
pub fn is_valid_epub(path: &Path) -> bool {
    EpubDoc::new(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn epub_reader_creation() {
        let reader = EpubReader::new();
        assert_eq!(reader.id(), FormatId::EPUB);
    }

    #[test]
    fn epub_writer_creation() {
        let writer = EpubWriter::new();
        assert_eq!(writer.id(), FormatId::EPUB);
    }

    #[test]
    fn create_minimal_epub() {
        let mut f = NamedTempFile::new().unwrap();
        write_test_epub(&mut f).unwrap();

        assert!(is_valid_epub(f.path()));
    }

    fn write_test_epub(f: &mut NamedTempFile) -> std::io::Result<()> {
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
    <dc:title>Test Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#)?;

        zip.start_file("OEBPS/ch1.xhtml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body><h1>Chapter One</h1><p>Hello world</p></body>
</html>"#)?;

        zip.start_file("OEBPS/nav.xhtml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body><nav epub:type="toc"><ol><li><a href="ch1.xhtml">Chapter 1</a></li></ol></nav></body>
</html>"#)?;

        zip.finish()?;
        Ok(())
    }
}