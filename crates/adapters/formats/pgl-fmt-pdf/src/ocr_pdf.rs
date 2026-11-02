//! PDF OCR - Para PDFs escaneados (imagens)
use uuid::Uuid;

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
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

/// Abre PDF com OCR (renderiza páginas como imagens e roda OCR)
pub async fn open_pdf_ocr(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    // Placeholder implementation
    let mut units = Vec::new();
    let mut assets = Vec::new();
    let section = SectionId::new();

    // Create a placeholder unit indicating OCR is needed
    let unit_id = UnitId::new();
    let source_ref = SourceRef {
        internal_path: "page_1.png".to_string(),
        selector: "full_page_ocr".to_string(),
        offset: None,
        length: None,
    };

    let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new("[OCR needed]"), section, Uuid::nil());
    unit.id = unit_id;
    unit.metadata.insert("page".into(), serde_json::Value::Number(1.into()));
    unit.metadata.insert("needs_ocr".into(), serde_json::Value::Bool(true));

    units.push(unit);

    let metadata = DocumentMetadata {
        title: Some("Scanned PDF".to_string()),
        page_count: Some(1),
        ..Default::default()
    };

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de páginas")
        .with_not_preserved("Texto (requer OCR)")
        .with_warning("PDF escaneado: requer OCR para extrair texto. Use pgl-ocr adapter.");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, metadata
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_ocr_placeholder() {
        // Teste de compilação apenas
    }
}