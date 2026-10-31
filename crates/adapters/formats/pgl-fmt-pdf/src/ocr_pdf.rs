//! PDF OCR - Para PDFs escaneados (imagens)

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use pdfium_render::prelude::*;

/// Abre PDF com OCR (renderiza páginas como imagens e roda OCR)
pub async fn open_pdf_ocr(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let pdfium = Pdfium::default();
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let mut units = Vec::new();
    let mut assets = Vec::new();
    let section = SectionId::new();

    let pages_count = document.pages().len();

    for page_idx in 0..pages_count {
        let page = document.pages().get(page_idx).unwrap();

        // Renderiza página como imagem
        let bitmap = page.render_with_config(
            PdfRenderConfig::new()
                .set_target_width(2000)
                .set_render_flags(PdfRenderFlags::ANNOTATIONS),
        ).map_err(|e| FormatError::ParseError(e.to_string()))?;

        let mut png_data = Vec::new();
        bitmap.write_png(&mut png_data).map_err(|e| FormatError::ParseError(e.to_string()))?;

        // Salva imagem como asset
        let hash = format!("{:x}", md5::compute(&png_data));
        assets.push(AssetRef {
            id: hash.clone(),
            internal_path: format!("page_{}.png", page_idx + 1),
            mime_type: "image/png".to_string(),
            size: png_data.len() as u64,
            hash: hash.clone(),
        });

        // TODO: Chamar OCR engine (pgl-ocr) aqui
        // Por enquanto, cria unidade placeholder
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: format!("page_{}.png", page_idx + 1),
            selector: "full_page_ocr".to_string(),
            offset: None,
            length: None,
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new("[OCR needed]"), section);
        unit.id = unit_id;
        unit.metadata.insert("page".into(), serde_json::Value::Number((page_idx + 1).into()));
        unit.metadata.insert("needs_ocr".into(), serde_json::Value::Bool(true));
        unit.metadata.insert("image_hash".into(), serde_json::Value::String(hash));

        units.push(unit);
    }

    let metadata = DocumentMetadata {
        title: document.metadata().title().map(|s| s.to_string()),
        author: document.metadata().author().map(|s| s.to_string()),
        subject: document.metadata().subject().map(|s| s.to_string()),
        page_count: Some(pages_count),
        ..Default::default()
    };

    let fidelity = default_zip_fidelity()
        .with_preserved("Imagens originais das páginas")
        .with_preserved("Estrutura de páginas")
        .with_not_preserved("Texto (requer OCR)")
        .with_not_preserved("Layout vetorial")
        .with_warning("PDF escaneado: requer OCR para extrair texto. Use pgl-ocr adapter.");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

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