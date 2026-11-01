//! Scanner e writer para XLSX

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;



/// Abre XLSX como SourceDocument (placeholder)
pub async fn open_xlsx(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let section = SectionId::new();
    let mut assets = Vec::new();

    // Placeholder implementation
    let unit_id = UnitId::new();
    let source_ref = SourceRef {
        internal_path: "xl/worksheets/sheet1.xml".to_string(),
        selector: "//sheetData/row[1]/c[1]".to_string(),
        offset: None,
        length: None,
    };

    let mut unit = Unit::new(source_ref, UnitKind::TableCell, MarkedText::new("[XLSX placeholder]"), SectionId::new());
    unit.id = UnitId::new();
    units.push(unit);

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_preserved("Formatação de células")
        .with_not_preserved("Macros/VBA")
        .with_warning("Implementação placeholder");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap(),
        units, assets, default_zip_fidelity(), Default::default()
    )))
}

/// Writer XLSX
pub async fn write_xlsx(
    source: &Path,
    _translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Placeholder implementation
    tokio::fs::copy(source, out).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::XLSX,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["XLSX writing: placeholder implementation".into()],
    })
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlsx_placeholder() {
        assert!(true);
    }
}