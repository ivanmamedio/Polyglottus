//! Scanner e writer para PPTX

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{default_zip_fidelity};
use std::path::Path;
use std::sync::Arc;
use zip::ZipArchive;
use std::fs::File;

/// Scanner PPTX
pub struct PptxScanner;

impl PptxScanner {
    pub fn new() -> Self {
        Self
    }
}

impl FormatScanner for PptxScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        let mut locations = Vec::new();

        // Slides estão em ppt/slides/slide*.xml
        for (file_path, content) in &package.content_parts {
            if file_path.starts_with("ppt/slides/") && file_path.ends_with(".xml") {
                self.scan_slide(content, file_path, &mut locations)?;
            }
        }

        // Slide masters e layouts
        for (file_path, content) in &package.content_parts {
            if (file_path.starts_with("ppt/slideMasters/") || file_path.starts_with("ppt/slideLayouts/"))
                && file_path.ends_with(".xml") {
                self.scan_slide(content, file_path, &mut locations)?;
            }
        }

        // Notes slides
        for (file_path, content) in &package.content_parts {
            if file_path.starts_with("ppt/notesSlides/") && file_path.ends_with(".xml") {
                self.scan_notes(content, file_path, &mut locations)?;
            }
        }

        Ok(locations)
    }

    fn get_text_selector(&self) -> &'static str {
        "a:t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["p:sp", "p:txBody"]
    }
}

impl PptxScanner {
    fn scan_slide(&self, content: &[u8], file_path: &str, locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        let doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        self.walk_pptx(&doc.document, file_path, locations)
    }

    fn scan_notes(&self, content: &[u8], file_path: &str, locations: &mut Vec<UnitLocation>) -> Result<(), FormatError> {
        self.scan_slide(content, file_path, locations)
    }

    fn walk_pptx(
        &self,
        node: &html5ever::rcdom::Handle,
        file_path: &str,
        locations: &mut Vec<UnitLocation>,
    ) -> Result<(), FormatError> {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string();

                // Text nodes in PPTX are in a:t (text run)
                if tag_name == "t" && name.prefix.as_deref() == Some("a") {
                    // Parent should be a:r (run), grandparent a:p (paragraph)
                    let text = self.extract_text(node);
                    if !text.trim().is_empty() {
                        let section = DomainSectionId::new();
                        let unit_id = UnitId::new();

                        // Tenta detectar se é título (primeiro texto na shape)
                        let kind = DomainUnitKind::Paragraph; // Simplificado

                        let source_ref = SourceRef {
                            internal_path: file_path.to_string(),
                            selector: "//a:t".to_string(),
                            offset: None,
                            length: None,
                        };

                        let mut unit = Unit::new(source_ref, kind, MarkedText::new(&text), section);
                        unit.id = unit_id;

                        locations.push(UnitLocation {
                            unit_id,
                            file_path: file_path.to_string(),
                            xpath: "//a:t".to_string(),
                            text_node_index: 0,
                            original_text: text,
                            kind,
                            section,
                        });
                    }
                }

                for child in node.children.borrow().iter() {
                    self.walk_pptx(child, file_path, locations)?;
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

/// Writer PPTX
pub async fn write_pptx(
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
        format: OutputFormat::PPTX,
        units_processed: engine.unit_locations.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

use pgl_domain::{Unit, UnitKind, SectionId, MarkedText, SourceRef, UnitId};
use pgl_fmt_core::ZipSourceDocument;
use std::collections::HashMap;
use pgl_app::{AssetRef, FidelityReport, DocumentMetadata};

/// Abre PPTX como SourceDocument
pub async fn open_pptx(path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
    let mut archive = pgl_fmt_core::zip_safe::open_zip_safe(path)?;
    let mut package = OpcPackage {
        parts: HashMap::new(),
        content_parts: HashMap::new(),
        relationships: HashMap::new(),
        content_types: Default::default(),
    };

    // Carrega partes
    for entry_info in pgl_fmt_core::zip_safe::list_entries_safe(&archive) {
        if entry_info.is_dir { continue; }
        let content = pgl_fmt_core::zip_safe::extract_entry_bytes(&mut archive, entry_info.name)?;
        let is_xml = entry_info.name.ends_with(".xml");
        package.parts.insert(entry_info.name.clone(), content.clone());
        if is_xml {
            package.content_parts.insert(entry_info.name, content);
        }
    }

    let scanner = PptxScanner;
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
        if name.starts_with("ppt/media/") {
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
        .with_preserved("Estrutura de slides")
        .with_preserved("Formatação de texto (fontes, cores, tamanhos)")
        .with_preserved("Imagens, formas, tabelas")
        .with_preserved("Animações e transições")
        .with_not_preserved("Macros/VBA")
        .with_warning("Efeitos de texto complexos (WordArt) podem perder formatação");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, Default::default()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_scanner_creation() {
        let _ = PptxScanner::new();
    }
}