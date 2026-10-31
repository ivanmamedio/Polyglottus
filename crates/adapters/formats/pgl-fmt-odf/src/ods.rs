//! Scanner e writer para ODS

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat, AssetRef, FidelityReport, DocumentMetadata};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use odf::OdfDocument;

/// Scanner ODS
pub struct OdsScanner;

impl OdsScanner {
    pub fn new() -> Self {
        Self
    }
}

impl FormatScanner for OdsScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        let mut locations = Vec::new();

        // ODS content.xml contém as tabelas
        if let Some(content) = package.content_parts.get("content.xml") {
            self.scan_content_xml(content, &mut locations)?;
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "table:table-cell > text:p"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["table:table-row", "table:table-header-rows"]
    }
}

impl OdsScanner {
    fn scan_content_xml(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        self.walk_ods(&doc.document, "content.xml", locations)
    }

    fn walk_ods(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, .. } => {
                let tag_name = name.local.to_string();

                if tag_name == "table-cell" {
                    // Procura text:p dentro da célula
                    for child in node.children.borrow().iter() {
                        let child_data = child.data.borrow();
                        if let html5ever::rcdom::NodeData::Element { name, .. } = &child_data.node_type {
                            if name.local.as_ref() == b"p" && name.prefix.as_deref() == Some("text") {
                                let text = self.extract_text(child);
                                if !text.trim().is_empty() {
                                    let section = DomainSectionId::new();
                                    let unit_id = UnitId::new();

                                    locations.push(UnitLocation {
                                        unit_id,
                                        file_path: file_path.to_string(),
                                        xpath: "//table:table-cell/text:p".to_string(),
                                        text_node_index: 0,
                                        original_text: text,
                                        kind: DomainUnitKind::TableCell,
                                        section,
                                    });
                                }
                            }
                        }
                    }
                }

                for child in node.children.borrow().iter() {
                    self.walk_ods(child, file_path, locations)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn extract_text(&self, node: &html5ever::rcdom::Handle) -> String {
        let mut text = String::new();
        self.collect_text(node, &mut text);
        text.trim().to_string()
    }

    fn collect_text(&self, node: &html5ever::rcdom::Handle, text: &mut String) {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Text { contents } => {
                text.push_str(&contents.borrow());
            }
            html5ever::rcdom::NodeData::Element { .. } => {
                for child in node.children.borrow().iter() {
                    self.collect_text(child, text);
                }
            }
            _ => {}
        }
    }
}

/// Abre ODS como SourceDocument
pub async fn open_ods(path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
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

    let scanner = OdsScanner;
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

    // Assets (imagens em Pictures/)
    let mut assets = Vec::new();
    for (name, content) in &package.parts {
        if name.starts_with("Pictures/") {
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

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_preserved("Formatação de células")
        .with_preserved("Gráficos")
        .with_not_preserved("Macros/scripts")
        .with_warning("Fórmulas com referências a texto traduzido podem quebrar");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, Default::default()
    )))
}

/// Writer ODS
pub async fn write_ods(
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

    engine.package.save(out)?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::ODS,
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
    fn ods_scanner_creation() {
        let _ = OdsScanner::new();
    }
}