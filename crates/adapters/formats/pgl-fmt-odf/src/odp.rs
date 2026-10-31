//! Scanner e writer para ODP

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat, AssetRef, FidelityReport, DocumentMetadata};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use odf::OdfDocument;

/// Scanner ODP
pub struct OdpScanner;

impl OdpScanner {
    pub fn new() -> Self {
        Self
    }
}

impl FormatScanner for OdpScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        // ODP usa content.xml para slides
        let mut locations = Vec::new();

        if let Some(content) = package.content_parts.get("content.xml") {
            self.scan_content_xml(content, &mut locations)?;
        }

        // Styles também podem ter texto
        if let Some(styles) = package.content_parts.get("styles.xml") {
            self.scan_styles_xml(styles, &mut locations)?;
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "text:p, text:h, text:span, draw:text-box"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["text:h", "text:list-item", "draw:text-box", "presentation:notes"]
    }
}

impl OdpScanner {
    fn scan_content_xml(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        self.walk_odp(&doc.document, "content.xml", locations)
    }

    fn scan_styles_xml(&self, content: &[u8], locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        self.walk_odp(&doc.document, "styles.xml", locations)
    }

    fn walk_odp(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string();

                let kind = match tag_name.as_str() {
                    "p" | "span" => DomainUnitKind::Paragraph,
                    "h" => {
                        let level = attrs.borrow().iter()
                            .find(|a| a.name.local.as_ref() == b"outline-level")
                            .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1);
                        DomainUnitKind::Heading(level)
                    }
                    "list-item" => DomainUnitKind::ListItem,
                    "table-cell" => DomainUnitKind::TableCell,
                    "text-box" => DomainUnitKind::Paragraph,
                    "notes" => DomainUnitKind::Paragraph, // Speaker notes
                    _ => DomainUnitKind::Other,
                };

                let text = self.extract_text(node);
                if !text.trim().is_empty() && !matches!(kind, DomainUnitKind::Other) {
                    let section = DomainSectionId::new();
                    let unit_id = UnitId::new();

                    locations.push(UnitLocation {
                        unit_id,
                        file_path: file_path.to_string(),
                        xpath: format!("//{}", tag_name),
                        text_node_index: 0,
                        original_text: text,
                        kind,
                        section,
                    });
                }

                for child in node.children.borrow().iter() {
                    self.walk_odp(child, file_path, locations)?;
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

/// Abre ODP como SourceDocument
pub async fn open_odp(path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
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

    let scanner = OdpScanner;
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
        .with_preserved("Estrutura de slides e layouts")
        .with_preserved("Formatação de texto (estilos)")
        .with_preserved("Imagens, formas, tabelas")
        .with_preserved("Animações e transições")
        .with_preserved("Notas do apresentador")
        .with_not_preserved("Macros/scripts");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, Default::default()
    )))
}

/// Writer ODP
pub async fn write_odp(
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
                let bilingual = format!("{} / {}", loc.original_text, translated.raw);
                engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &bilingual)?;
            }
        }
    }

    engine.package.save(out)?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::ODP,
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
    fn odp_scanner_creation() {
        let _ = OdpScanner::new();
    }
}