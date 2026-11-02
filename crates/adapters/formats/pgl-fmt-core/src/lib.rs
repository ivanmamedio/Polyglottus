//! Utilitários comuns para adaptadores de formato

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod sniff;
pub mod xml;
pub mod zip_safe;
pub mod source_ref;
pub mod format_builder;

use std::io::Write;
use std::io::{Read, Seek};
use std::path::Path;
use pgl_app::{FormatId, Confidence, FormatError, AssetRef, DocumentMetadata, FidelityReport};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};

/// Trait base para leitores de formato baseados em ZIP (EPUB, DOCX, ODF, etc.)
pub trait ZipFormatReader {
    fn format_id(&self) -> FormatId;
    fn extensions(&self) -> &'static [&'static str];
    fn mime_types(&self) -> &'static [&'static str];

    fn open_zip(&self, path: &Path) -> Result<zip::ZipArchive<std::fs::File>, FormatError> {
        let file = std::fs::File::open(path)?;
        zip::ZipArchive::new(file).map_err(|e| FormatError::ParseError(e.to_string()))
    }

    fn read_xml_entry(&self, archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String, FormatError> {
        let mut entry = archive.by_name(name)?;
        let mut content = String::new();
        std::io::Read::read_to_string(&mut entry, &mut content)?;
        Ok(content)
    }

    fn read_bytes_entry(&self, archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, FormatError> {
        let mut entry = archive.by_name(name)?;
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content)?;
        Ok(content)
    }

    fn list_entries(&self, archive: &zip::ZipArchive<std::fs::File>) -> Vec<String> {
        archive.file_names().map(|s| s.to_string()).collect()
    }

    fn extract_assets(&self, archive: &mut zip::ZipArchive<std::fs::File>, base_path: &str) -> Vec<AssetRef> {
        let mut assets = Vec::new();
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let name = entry.name().to_string();
                if name.starts_with(base_path) && !entry.is_dir() {
                    let mime = mime_guess::from_path(&name).first_or_octet_stream();
                    assets.push(AssetRef {
                        id: format!("asset-{}", i),
                        internal_path: name.clone(),
                        mime_type: mime.to_string(),
                        size: entry.size(),
                        hash: String::new(), // Calculado sob demanda
                    });
                }
            }
        }
        assets
    }
}

/// Trait base para escritores de formato baseados em ZIP
pub trait ZipFormatWriter {
    fn format_id(&self) -> FormatId;

    fn create_zip(&self, path: &Path) -> Result<zip::ZipWriter<std::fs::File>, FormatError> {
        let file = std::fs::File::create(path)?;
        Ok(zip::ZipWriter::new(file))
    }

    fn copy_entry(&self, src: &mut zip::ZipArchive<std::fs::File>, dst: &mut zip::ZipWriter<std::fs::File>, name: &str) -> Result<(), FormatError> {
        let mut entry = src.by_name(name)?;
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buffer)?;

        let options = zip::write::FileOptions::default()
            .compression_method(entry.compression())
            .compression_level(Some(6));

        dst.start_file(name, options)?;
        dst.write_all(&buffer)?;
        Ok(())
    }

    fn write_entry(&self, dst: &mut zip::ZipWriter<std::fs::File>, name: &str, content: &[u8]) -> Result<(), FormatError> {
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));
        dst.start_file(name, options)?;
        dst.write_all(content)?;
        Ok(())
    }

    fn write_xml_entry(&self, dst: &mut zip::ZipWriter<std::fs::File>, name: &str, content: &str) -> Result<(), FormatError> {
        self.write_entry(dst, name, content.as_bytes())
    }
}

/// Documento fonte genérico para formatos ZIP
pub struct ZipSourceDocument<R: Read + Seek + Send + Sync + 'static> {
    archive: zip::ZipArchive<R>,
    units: Vec<Unit>,
    assets: Vec<AssetRef>,
    fidelity: FidelityReport,
    metadata: DocumentMetadata,
}

impl<R: Read + Seek + Send + Sync + 'static> ZipSourceDocument<R> {
    pub fn new(
        archive: zip::ZipArchive<R>,
        units: Vec<Unit>,
        assets: Vec<AssetRef>,
        fidelity: FidelityReport,
        metadata: DocumentMetadata,
    ) -> Self {
        Self { archive, units, assets, fidelity, metadata }
    }
}

impl<R: Read + Seek + Send + Sync + 'static> pgl_app::SourceDocument for ZipSourceDocument<R> {
    fn units(&mut self) -> Box<dyn Iterator<Item = Result<Unit, FormatError>> + '_> {
        let units = std::mem::take(&mut self.units);
        Box::new(units.into_iter().map(Ok))
    }

    fn assets(&self) -> Vec<AssetRef> {
        self.assets.clone()
    }

    fn fidelity(&self) -> FidelityReport {
        self.fidelity.clone()
    }

    fn metadata(&self) -> DocumentMetadata {
        self.metadata.clone()
    }
}

/// Helper para criar UnitId determinístico
pub fn make_unit_id(source_ref: &SourceRef, text: &str) -> UnitId {
    let hash = blake3::hash(&[source_ref.internal_path.as_bytes(), source_ref.selector.as_bytes(), text.as_bytes()].concat());
    let hash_bytes: [u8; 16] = *<&[u8; 16]>::try_from(&hash.as_bytes()[..16]).unwrap();
    UnitId::from_hash_bytes(&hash_bytes)
}

/// Helper para criar SourceRef padrão
pub fn make_source_ref(internal_path: &str, selector: &str) -> SourceRef {
    SourceRef {
        internal_path: internal_path.to_string(),
        selector: selector.to_string(),
        offset: None,
        length: None,
    }
}

/// Helper para criar SectionId
pub fn make_section_id() -> SectionId {
    SectionId::new()
}

/// Fidelity report padrão para formatos ZIP-based (alta fidelidade)
pub fn default_zip_fidelity() -> FidelityReport {
    FidelityReport::new(5)
        .with_preserved("Estrutura do documento")
        .with_preserved("Imagens e assets")
        .with_preserved("Estilos e formatação")
        .with_preserved("Metadados")
        .with_preserved("Links internos e sumário")
        .with_not_preserved("Scripts/active content (não suportado)")
}

/// Função utilitária para sniffing de ZIP-based formats
pub fn sniff_zip_format(path: &Path, head: &[u8], expected_files: &[&str]) -> Confidence {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    // Verifica extensão
    let ext_match = expected_files.iter().any(|ext| name.ends_with(ext));

    // Verifica magic bytes (PK para ZIP) usando head já fornecido
    let zip_match = head.len() >= 4 && &head[0..4] == b"PK\x03\x04";

    match (ext_match, zip_match) {
        (true, true) => Confidence::certain(),
        (true, false) => Confidence::high(),
        (false, true) => Confidence::medium(),
        (false, false) => Confidence::none(),
    }
}

pub use format_builder::{
    ZipFormatReaderBuilder, ZipFormatReaderConfig, BuiltinZipFormatReader,
    ZipFormatWriterBuilder, ZipFormatWriterConfig, BuiltinZipFormatWriter,
};

/// Cria ZIP vazio
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn make_unit_id_deterministic() {
        let ref1 = make_source_ref("test.xhtml", "//p[1]");
        let ref2 = make_source_ref("test.xhtml", "//p[1]");

        let id1 = make_unit_id(&ref1, "Hello world");
        let id2 = make_unit_id(&ref2, "Hello world");
        assert_eq!(id1, id2);

        let id3 = make_unit_id(&ref1, "Different text");
        assert_ne!(id1, id3);
    }

    #[test]
    fn sniff_zip_format_epub() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"PK\x03\x04").unwrap(); // ZIP header
        let path = f.path().to_path_buf();

        // Renomeia para .epub
        let epub_path = path.with_extension("epub");
        std::fs::rename(&path, &epub_path).unwrap();

        let head = std::fs::read(&epub_path).unwrap_or_default();
        let conf = sniff_zip_format(&epub_path, &head, &[".epub"]);
        assert_eq!(conf.value(), 100); // certain

        std::fs::remove_file(epub_path).ok();
    }
}