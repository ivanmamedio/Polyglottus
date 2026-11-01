//! PDF Overlay - Mantém PDF original, sobrepõe texto traduzido

use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use std::path::Path;

/// Escreve PDF com overlay de tradução
pub async fn write_pdf_overlay(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Copy source to out for now (placeholder implementation)
    tokio::fs::copy(source, out).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::PDF,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![
            "Overlay mode: placeholder implementation".into(),
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
pub fn make_unit_id_for_pdf(text: &str, page: usize, bbox: (f32, f32, f32, f32)) -> pgl_domain::UnitId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    page.hash(&mut hasher);
    bbox.0.to_bits().hash(&mut hasher);
    bbox.1.to_bits().hash(&mut hasher);
    pgl_domain::UnitId::from_hash(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_unit_id_for_pdf_test() {
        let id1 = make_unit_id_for_pdf("Hello", 1, (0.0, 0.0, 100.0, 20.0));
        let id2 = make_unit_id_for_pdf("Hello", 1, (0.0, 0.0, 100.0, 20.0));
        assert_eq!(id1, id2);

        let id3 = make_unit_id_for_pdf("World", 1, (0.0, 0.0, 100.0, 20.0));
        assert_ne!(id1, id3);
    }
}