//! Scanner e writer para ODS
use uuid::Uuid;

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;

/// Abre ODS como SourceDocument (placeholder)
pub async fn open_ods(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let section = SectionId::new();
    let mut assets = Vec::new();

    // Placeholder implementation
    let unit_id = UnitId::new();
    let source_ref = SourceRef {
        internal_path: "content.xml".to_string(),
        selector: "//table:table-cell".to_string(),
        offset: None,
        length: None,
    };

    let mut unit = Unit::new(source_ref, UnitKind::TableCell, MarkedText::new("[ODS placeholder]"), SectionId::new(), Uuid::nil());
    unit.id = UnitId::new();
    units.push(unit);

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_not_preserved("Macros")
        .with_warning("Implementação placeholder");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap(),
        units, assets, default_zip_fidelity(), Default::default()
    )))
}

/// Writer ODS
pub async fn write_ods(
    source: &Path,
    _translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Placeholder implementation
    tokio::fs::copy(source, out).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::ODS,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["ODS writing: placeholder implementation".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ods_placeholder() {
        assert!(true);
    }
}