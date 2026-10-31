//! Scanner e writer para XLSX

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{default_zip_fidelity};
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use calamine::{Reader, Xlsx, open_workbook};

/// Scanner XLSX
pub struct XlsxScanner;

impl XlsxScanner {
    pub fn new() -> Self {
        Self
    }
}

impl FormatScanner for XlsxScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        // XLSX usa sharedStrings.xml para texto
        // Em produção, parsear sharedStrings e sheets
        Err(FormatError::ParseError("Use extract_units with file path".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "si > t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["sheetData > row > c"]
    }
}

/// Extrai unidades de XLSX usando calamine
pub fn extract_xlsx_units(path: &Path) -> Result<Vec<Unit>, FormatError> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let mut units = Vec::new();
    let section = DomainSectionId::new();

    for sheet_name in workbook.sheet_names() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            for (row_idx, row) in range.rows().enumerate() {
                for (col_idx, cell) in row.iter().enumerate() {
                    let text = cell.to_string();
                    if text.trim().is_empty() {
                        continue;
                    }

                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: format!("xl/worksheets/{}.xml", sheet_name),
                        selector: format!("//row[@r='{}']/c[@r='{}{}]", row_idx + 1, col_idx_to_letter(col_idx), row_idx + 1),
                        offset: None,
                        length: None,
                    };

                    let mut unit = Unit::new(source_ref, DomainUnitKind::TableCell, MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                }
            }
        }
    }

    Ok(units)
}

fn col_idx_to_letter(mut idx: usize) -> String {
    let mut result = String::new();
    loop {
        let rem = idx % 26;
        result.push((b'A' + rem as u8) as char);
        if idx < 26 { break; }
        idx = idx / 26 - 1;
    }
    result.chars().rev().collect()
}

/// Writer XLSX
pub async fn write_xlsx(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Para XLSX, reescreve sharedStrings.xml e sheets
    let mut engine = super::super::pgl_fmt_patch::PatchEngine::from_file(source)?;

    for loc in &engine.unit_locations {
        if let Some(translated) = translations.get(&loc.unit_id) {
            engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &translated.raw)?;
        }
    }

    engine.package.save(out)?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::XLSX,
        units_processed: engine.unit_locations.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

/// Abre XLSX como SourceDocument
pub async fn open_xlsx(path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
    let units = extract_xlsx_units(path)?;

    // Assets (imagens em xl/media/)
    let mut archive = pgl_fmt_core::zip_safe::open_zip_safe(path)?;
    let mut assets = Vec::new();

    for entry_info in pgl_fmt_core::zip_safe::list_entries_safe(&archive) {
        if entry_info.name.starts_with("xl/media/") && !entry_info.is_dir {
            let content = pgl_fmt_core::zip_safe::extract_entry_bytes(&mut archive, entry_info.name)?;
            let mime = mime_guess::from_path(&entry_info.name).first_or_octet_stream();
            let hash = format!("{:x}", md5::compute(&content));
            assets.push(pgl_app::AssetRef {
                id: hash.clone(),
                internal_path: entry_info.name,
                mime_type: mime.to_string(),
                size: content.len() as u64,
                hash,
            });
        }
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_preserved("Formatação de células")
        .with_preserved("Gráficos e tabelas dinâmicas")
        .with_not_preserved("Macros/VBA")
        .with_warning("Fórmulas com referências a texto traduzido podem quebrar");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(pgl_fmt_core::ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, Default::default()
    )))
}

use pgl_domain::{Unit, UnitKind, SectionId, MarkedText, SourceRef, UnitId};
use pgl_fmt_core::ZipSourceDocument;
use pgl_app::{AssetRef, FidelityReport, DocumentMetadata};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlsx_scanner_creation() {
        let _ = XlsxScanner::new();
    }

    #[test]
    fn col_idx_to_letter_test() {
        assert_eq!(col_idx_to_letter(0), "A");
        assert_eq!(col_idx_to_letter(25), "Z");
        assert_eq!(col_idx_to_letter(26), "AA");
        assert_eq!(col_idx_to_letter(27), "AB");
    }
}