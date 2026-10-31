//! Scanner e writer para DOCX

use super::super::pgl_fmt_patch::{FormatScanner, UnitLocation, UnitKind, SectionId, OpcPackage};
use pgl_domain::{UnitId, Unit, UnitKind as DomainUnitKind, SectionId as DomainSectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, FormatId, Confidence, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_unit_id, make_source_ref, make_section_id, default_zip_fidelity};
use docx_rs::{read_docx, Docx, Paragraph, Run, Text};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Scanner DOCX
pub struct DocxScanner;

impl DocxScanner {
    pub fn new() -> Self {
        Self
    }

    /// Extrai unidades usando docx-rs
    pub fn extract_units(&self, path: &Path) -> Result<Vec<Unit>, FormatError> {
        let docx = read_docx(path)
            .map_err(|e| FormatError::ParseError(e.to_string()))?;

        let mut units = Vec::new();
        let mut section = DomainSectionId::new();
        let mut para_index = 0;

        // Processa parágrafos do corpo principal
        for para in docx.paragraphs {
            let text = self.paragraph_text(&para);
            if text.trim().is_empty() {
                continue;
            }

            // Detecta tipo baseado no estilo
            let kind = self.detect_kind(&para);

            let unit_id = UnitId::new();
            let source_ref = SourceRef {
                internal_path: "word/document.xml".to_string(),
                selector: format!("//w:body/w:p[{}]", para_index + 1),
                offset: None,
                length: None,
            };

            let mut unit = Unit::new(source_ref, kind, MarkedText::new(&text), section);
            unit.id = unit_id;
            units.push(unit);

            para_index += 1;
        }

        // Processa tabelas
        for (table_idx, table) in docx.tables.iter().enumerate() {
            for (row_idx, row) in table.rows.iter().enumerate() {
                for (cell_idx, cell) in row.cells.iter().enumerate() {
                    for para in &cell.paragraphs {
                        let text = self.paragraph_text(para);
                        if text.trim().is_empty() {
                            continue;
                        }

                        let unit_id = UnitId::new();
                        let source_ref = SourceRef {
                            internal_path: "word/document.xml".to_string(),
                            selector: format!("//w:tbl[{}]/w:tr[{}]/w:tc[{}]/w:p", table_idx + 1, row_idx + 1, cell_idx + 1),
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

        // Processa cabeçalhos/rodapés
        for header in &docx.headers {
            for para in &header.paragraphs {
                let text = self.paragraph_text(para);
                if text.trim().is_empty() {
                    continue;
                }

                let unit_id = UnitId::new();
                let source_ref = SourceRef {
                    internal_path: "word/header.xml".to_string(),
                    selector: format!("//w:p"),
                    offset: None,
                    length: None,
                };

                let mut unit = Unit::new(source_ref, DomainUnitKind::Paragraph, MarkedText::new(&text), section);
                unit.id = unit_id;
                units.push(unit);
            }
        }

        Ok(units)
    }

    fn paragraph_text(&self, para: &Paragraph) -> String {
        para.runs.iter()
            .filter_map(|run| match run {
                Run::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn detect_kind(&self, para: &Paragraph) -> DomainUnitKind {
        // Verifica estilo do parágrafo
        if let Some(style) = &para.property.style {
            if style.val.starts_with("Heading") {
                if let Some(level) = style.val.chars().last().and_then(|c| c.to_digit(10)) {
                    return DomainUnitKind::Heading(level as u8);
                }
                return DomainUnitKind::Heading(1);
            }
        }
        DomainUnitKind::Paragraph
    }
}

impl FormatScanner for DocxScanner {
    fn scan(&self, package: &OpcPackage) -> Result<Vec<UnitLocation>, FormatError> {
        // Usa docx-rs via arquivo temporário (simplificado)
        // Em produção, implementar scanner direto no XML
        Err(FormatError::ParseError("Use extract_units with file path".into()))
    }

    fn get_text_selector(&self) -> &'static str {
        "w:t"
    }

    fn get_context_selectors(&self) -> &'static [&'static str] {
        &["w:pStyle[@w:val='Heading1']", "w:pStyle[@w:val='Heading2']", "w:tbl"]
    }
}

/// Abre DOCX como SourceDocument
pub async fn open_docx(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let scanner = DocxScanner;
    let units = scanner.extract_units(path)?;

    // Assets (imagens em word/media/)
    let assets = extract_docx_assets(path)?;

    // Metadados
    let docx = read_docx(path).map_err(|e| FormatError::ParseError(e.to_string()))?;
    let metadata = DocumentMetadata {
        title: docx.core_properties.title,
        author: docx.core_properties.creator,
        subject: docx.core_properties.subject,
        description: docx.core_properties.description,
        created: docx.core_properties.created.map(|d| d.to_string()),
        modified: docx.core_properties.modified.map(|d| d.to_string()),
        ..Default::default()
    };

    // Archive vazio
    let archive = zip::ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    let fidelity = default_zip_fidelity()
        .with_preserved("Formatação de parágrafos e runs")
        .with_preserved("Estilos, listas, tabelas")
        .with_preserved("Imagens, cabeçalhos, rodapés")
        .with_preserved("Comentários e revisões")
        .with_not_preserved("Macros/VBA")
        .with_warning("Equações complexas podem perder formatação");

    Ok(Box::new(ZipSourceDocument::new(
        archive, units, assets, fidelity, metadata
    )))
}

fn extract_docx_assets(path: &Path) -> Result<Vec<AssetRef>, FormatError> {
    let mut archive = pgl_fmt_core::zip_safe::open_zip_safe(path)?;
    let mut assets = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if name.starts_with("word/media/") && !entry.is_dir() {
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            let content = pgl_fmt_core::zip_safe::extract_entry_bytes(&mut archive, i)?;
            let hash = format!("{:x}", md5::compute(&content));

            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: name,
                mime_type: mime.to_string(),
                size: content.len() as u64,
                hash,
            });
        }
    }

    Ok(assets)
}

/// Escreve DOCX traduzido
pub async fn write_docx(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Usa patch engine do pgl-fmt-patch
    let mut engine = super::super::pgl_fmt_patch::PatchEngine::from_file(source)?;

    for loc in &engine.unit_locations {
        if let Some(translated) = translations.get(&loc.unit_id) {
            engine.package.update_text_node(&loc.file_path, &loc.xpath, loc.text_node_index, &translated.raw)?;
        }
    }

    if options.bilingual {
        // Bilíngue: adiciona tradução após original
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
        format: OutputFormat::DOCX,
        units_processed: engine.unit_locations.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn docx_scanner_creation() {
        let _ = DocxScanner::new();
    }

    #[test]
    fn create_test_docx() {
        let mut f = NamedTempFile::new().unwrap();
        write_minimal_docx(&mut f).unwrap();

        let scanner = DocxScanner;
        let units = scanner.extract_units(f.path()).unwrap();
        assert!(!units.is_empty());
    }

    fn write_minimal_docx(f: &mut NamedTempFile) -> std::io::Result<()> {
        use docx_rs::*;
        let doc = Docx::new()
            .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Hello world")))
            .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Second paragraph")));

        let bytes = docx_rs::to_bytes(&doc).unwrap();
        f.write_all(&bytes)?;
        Ok(())
    }
}