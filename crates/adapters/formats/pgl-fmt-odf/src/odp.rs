//! Scanner e writer para ODP

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;

/// Abre ODP como SourceDocument (placeholder)
pub async fn open_odp(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let section = SectionId::new();
    let mut assets = Vec::new();

    // Placeholder implementation
    let unit_id = UnitId::new();
    let source_ref = SourceRef {
        internal_path: "content.xml".to_string(),
        selector: "//draw:page".to_string(),
        offset: None,
        length: None,
    };

    let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new("[ODP placeholder]"), SectionId::new());
    unit.id = UnitId::new();
    units.push(unit);

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de slides")
        .with_preserved("Formatação de texto")
        .with_not_preserved("Macros")
        .with_warning("Implementação placeholder");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap(),
        units, assets, default_zip_fidelity(), Default::default()
    )))
}

/// Writer ODP
pub async fn write_odp(
    source: &Path,
    _translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Placeholder implementation
    tokio::fs::copy(source, out).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::ODP,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["ODP writing: placeholder implementation".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odp_placeholder() {
        assert!(true);
    }
}