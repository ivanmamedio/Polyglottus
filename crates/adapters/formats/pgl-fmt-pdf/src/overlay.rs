//! PDF Overlay - Mantém PDF original, sobrepõe texto traduzido
use uuid::Uuid;

use pgl_app::{FormatError, ExportOptions, ExportReport, OutputFormat};
use std::path::Path;
use crate::reflow::create_empty_zip;
use lopdf::{Document as LoppdfDocument, Object, ObjectId, Stream};
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
    
    let doc = LoppdfDocument::load_mem(&source_bytes)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Cria novo PDF com as mesmas páginas mas adicionando anotações de tradução
    // Por enquanto, apenas copia o PDF original
    fs::write(out, source_bytes).await
        .map_err(|e| FormatError::WriteError(format!("Failed to write output: {}", e)))?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::PDF,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![
            "Overlay mode: basic implementation (translation overlay not yet implemented)".into(),
        ],
    })
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