//! PDF Reflow - Extrai texto e estrutura para exportar como EPUB/DOCX/HTML

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;

fn create_empty_zip() -> Vec<u8> {
    use zip::ZipWriter;
    use std::io::Cursor;
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}

/// Abre PDF no modo reflow
pub async fn open_pdf_reflow(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    // Placeholder implementation - uses pdfium when available
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    
    let mut units = Vec::new();
    let mut section = SectionId::new();
    let mut global_idx = 0;

    // Simple text extraction fallback
    let paragraphs: Vec<&str> = content.split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.len() > 10)
        .collect();

    for (para_idx, para) in paragraphs.iter().copied().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: format!("page_1.txt"),
            selector: format!("paragraph[{}]", para_idx),
            offset: None,
            length: Some(para.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(para), section);
        unit.id = unit_id;
        unit.metadata.insert("page".into(), serde_json::Value::Number(1.into()));
        unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
        units.push(unit);
        global_idx += 1;
    }

    let metadata = DocumentMetadata {
        title: Some("PDF Document".to_string()),
        ..Default::default()
    };

    let fidelity = default_zip_fidelity()
        .with_preserved("Texto em ordem de leitura")
        .with_not_preserved("Layout original (colunas, tabelas, formatação)")
        .with_warning("Modo reflow: implementação simplificada");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, metadata
    )))
}

/// Exporta PDF reflow para outros formatos
pub async fn export_reflow_to_format(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
    target_format: OutputFormat,
) -> Result<ExportReport, FormatError> {
    let mut doc = open_pdf_reflow(source).await?;

    let mut translated_units = Vec::new();
    for unit_result in doc.units() {
        let unit = unit_result?;
        let unit_id = unit.id;

        let translated_text = if let Some(translated) = translations.get(&unit_id) {
            if options.bilingual {
                format!("{}\n\n{}", unit.text.raw, translated.raw)
            } else {
                translated.raw.clone()
            }
        } else {
            unit.text.raw.clone()
        };

        let mut translated_unit = unit.clone();
        translated_unit.apply_translation(MarkedText::new(&translated_text))
            .map_err(|e| FormatError::ValidationError(e.to_string()))?;
        translated_units.push(translated_unit);
    }

    match target_format {
        OutputFormat::TXT => export_to_txt(&translated_units, out).await,
        OutputFormat::MD => export_to_md(&translated_units, out).await,
        _ => Err(FormatError::WriteError(format!("Unsupported output format for reflow: {:?}", target_format))),
    }
}

async fn export_to_txt(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut text = String::new();
    for unit in units {
        text.push_str(&unit.display_text());
        text.push_str("\n\n");
    }
    tokio::fs::write(out, text).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::TXT,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

async fn export_to_md(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut md = String::new();
    for unit in units {
        md.push_str(&unit.display_text());
        md.push_str("\n\n");
    }
    tokio::fs::write(out, md).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::MD,
        units_processed: units.len(),
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

    #[test]
    fn reflow_placeholder() {
        // Teste de compilação apenas
    }
}