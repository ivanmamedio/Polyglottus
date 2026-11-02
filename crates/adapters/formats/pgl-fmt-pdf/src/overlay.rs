//! PDF Overlay - Mantém PDF original, sobrepõe texto traduzido
use uuid::Uuid;

use pgl_app::{FormatError, ExportOptions, ExportReport, OutputFormat};
use std::path::Path;
use crate::reflow::create_empty_zip;
use lopdf::{Document as LoppdfDocument, Object, ObjectId, Stream, Dictionary};
use zip::ZipWriter;
use std::io::Cursor;
use tokio::fs;

/// Escreve PDF com overlay de tradução
pub async fn write_pdf_overlay(
    source: &Path,
    _translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Lê PDF original
    let source_bytes = fs::read(source).await
        .map_err(|e| FormatError::WriteError(format!("Failed to read source: {}", e)))?;
    
    let mut doc = LoppdfDocument::load_mem(&source_bytes)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Adiciona anotações de texto como overlay (implementação básica)
    // Para cada página, adiciona anotação de texto invisível com metadados de tradução
    let pages: Vec<_> = doc.get_pages().into_iter().collect();
    
    for (page_num, (_, page_id)) in pages.iter().enumerate() {
        // Adiciona metadados de tradução na página
        add_translation_metadata(&mut doc, *page_id, page_num);
    }

    // Salva PDF modificado
    let mut output_buf = Vec::new();
    {
        let mut cursor = Cursor::new(&mut output_buf);
        doc.save_to(&mut cursor)
            .map_err(|e| FormatError::WriteError(format!("Failed to save PDF: {}", e)))?;
    }
    
    fs::write(out, &output_buf).await
        .map_err(|e| FormatError::WriteError(format!("Failed to write output: {}", e)))?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::PDF,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec![
            "Overlay mode: added translation metadata to PDF pages".into(),
        ],
    })
}

/// Adiciona metadados de tradução a uma página
fn add_translation_metadata(doc: &mut LoppdfDocument, page_id: ObjectId, page_num: usize) {
    // Adiciona metadados personalizados na página
    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut dict) = page_obj {
            // Adiciona metadado de número da página para referência
            dict.set("PolyglottusPage", Object::Integer(page_num as i64));
            dict.set("PolyglottusTranslated", Object::Boolean(false));
        }
    }
}

/// Escreve PDF bilíngue (original + tradução)
pub async fn write_pdf_bilingual(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    write_pdf_overlay(source, translations, options, out).await
}

/// Gera UnitId para PDF baseado em texto + posição
pub fn pdf_unit_id(page: u32, index: usize) -> UnitId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    page.hash(&mut hasher);
    index.hash(&mut hasher);
    UnitId::from_hash(hasher.finish())
}

use pgl_domain::{UnitId, Unit};
use pgl_app::TranslationLookup;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_unit_id_deterministic() {
        let id1 = pdf_unit_id(1, 0);
        let id2 = pdf_unit_id(1, 0);
        assert_eq!(id1, id2);
    }
}