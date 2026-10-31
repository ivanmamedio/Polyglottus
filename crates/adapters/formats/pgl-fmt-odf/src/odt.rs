//! Scanner e writer para ODT

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat, AssetRef, FidelityReport, DocumentMetadata};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use odf::OdfDocument;

/// Scanner ODT (usa odf crate + scanner do pgl-fmt-patch)
pub struct OdtScanner;

impl OdtScanner {
    pub fn new() -> Self {
        Self
    }
}

impl FormatScanner for OdtScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        // Usa scanner ODF do pgl-fmt-patch
        super::super::pgl_fmt_patch::odf::OdfScanner.scan(package)
    }

    fn get_text_selector(&self) -> &'static str {
        "text:p, text:h, text:span"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["text:h", "text:list-item", "table:table-cell"]
    }
}

/// Abre ODT como SourceDocument
pub async fn open_odt(path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
    // Usa odf crate para parsing robusto
    let doc = OdfDocument::load(path)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Também carrega via OpcPackage para patch engine
    let mut archive = pgl_fmt_core::zip_safe::open_zip_safe(path)?;
    let mut package = OpcPackage {
        parts: HashMap::new(),
        content_parts: HashMap::new(),
        relationships: HashMap::new(),
        content_types: Default::default(),
    };

    for entry_info in pgl_fmt_core::zip_safe::list_entries_safe(&archive) {
        if entry_info.is_dir { continue; }
        let content = pgl_fmt_core::zip_safe::extract_entry_bytes(&mut archive, entry_info.name)?;
        let is_xml = entry_info.name.ends_with(".xml");
        package.parts.insert(entry_info.name.clone(), content.clone());
        if is_xml {
            package.content_parts.insert(entry_info.name, content);
        }
    }

    let scanner = OdtScanner;
    let locations = scanner.scan(&package)?;

    let units: Vec<Unit> = locations.into_iter().map(|loc| {
        let source_ref = SourceRef {
            internal_path: loc.file_path.clone(),
            selector: loc.xpath,
            offset: None,
            length: None,
        };
        let mut unit = Unit::new(source_ref, loc.kind, MarkedText::new(&loc.original_text), loc.section);
        unit.id = loc.unit_id;
        unit
    }).collect();

    // Assets
    let mut assets = Vec::new();
    for (name, content) in &package.parts {
        if name.starts_with("Pictures/") || name.starts_with("Images/") {
            let mime = mime_guess::from_path(name).first_or_octet_stream();
            let hash = format!("{:x}", md5::compute(content));
            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: name.clone(),
                mime_type: mime.to_string(),
                size: content.len() as u64,
                hash,
            });
        }
    }

    // Metadados do odf crate
    let metadata = DocumentMetadata {
        title: doc.meta().map(|m| m.title().unwrap_or_default()).filter(|s| !s.is_empty()),
        author: doc.meta().map(|m| m.creator().unwrap_or_default()).filter(|s| !s.is_empty()),
        subject: doc.meta().map(|m| m.subject().unwrap_or_default()).filter(|s| !s.is_empty()),
        description: doc.meta().map(|m| m.description().unwrap_or_default()).filter(|s| !s.is_empty()),
        created: doc.meta().map(|m| m.creation_date().map(|d| d.to_string())).flatten(),
        modified: doc.meta().map(|m| m.modification_date().map(|d| d.to_string())).flatten(),
        ..Default::default()
    };

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de estilos automáticos")
        .with_preserved("Imagens, tabelas, listas")
        .with_preserved("Índices, sumários, bibliografia")
        .with_preserved("Formatação de página")
        .with_not_preserved("Scripts/macros")
        .with_warning("Objetos OLE incorporados podem não ser traduzidos");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, metadata
    )))
}

/// Writer ODT
pub async fn write_odt(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let mut engine = super::super::pgl_fmt_patch::PatchEngine::from_file(source)?;

    for loc in &engine.unit_locations {
        if let Some(translated) = translations.get(&loc.unit_id) {
            engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &translated.raw)?;
        }
    }

    if options.bilingual {
        for loc in &engine.unit_locations {
            if let Some(translated) = translations.get(&loc.unit_id) {
                let bilingual = format!("{} | {}", loc.original_text, translated.raw);
                engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &bilingual)?;
            }
        }
    }

    engine.package.save(out)?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::ODT,
        units_processed: engine.unit_locations.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

use pgl_domain::{Unit, UnitKind, SectionId, MarkedText, SourceRef, UnitId};
use pgl_fmt_core::zip_safe::{open_zip_safe, list_entries_safe, extract_entry_bytes};
use pgl_fmt_patch::OpcPackage;
use std::collections::HashMap;
use zip::ZipArchive;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odt_scanner_creation() {
        let _ = OdtScanner::new();
    }
}