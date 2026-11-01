//! TXT - Texto simples

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::collections::HashMap;
use zip::ZipArchive;
type SourceDoc = Box<dyn SourceDocument + 'static>;


/// Abre TXT como SourceDocument
pub async fn open_txt(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;

    // Divide em parágrafos (linhas vazias)
    let paragraphs: Vec<&str> = content.split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, para) in paragraphs.iter().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("paragraph[{}]", idx),
            offset: None,
            length: Some(para.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(para), section);
        unit.id = unit_id;
        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de parágrafos")
        .with_preserved("Quebras de linha")
        .with_not_preserved("Formatação rica (não existe em TXT)")
        .with_warning("Perde formatação se convertido de outro formato");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve TXT traduzido
pub async fn write_txt(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Reconstrói o texto original com traduções
    let content = tokio::fs::read_to_string(source).await?;
    let paragraphs: Vec<&str> = content.split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut output_paragraphs = Vec::new();

    for (idx, para) in paragraphs.iter().enumerate() {
        let unit_id = UnitId::from_hash(&{
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source.file_name().unwrap_or_default().to_string_lossy().hash(&mut hasher);
            format!("paragraph[{}]", idx).hash(&mut hasher);
            para.hash(&mut hasher);
            hasher.finish().to_le_bytes()
        });

        if let Some(translated) = translations.get(&unit_id) {
            let text = if options.bilingual {
                format!("{}\n\n{}", para, translated.raw)
            } else {
                translated.raw.clone()
            };
            output_paragraphs.push(text);
        } else {
            output_paragraphs.push(para.to_string());
        }
    }

    let output = output_paragraphs.join("\n\n");
    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::TXT,
        units_processed: paragraphs.len(),
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
    use tempfile::NamedTempFile;
    use std::io::Write;
    use std::collections::HashMap;

    #[tokio::test]
    async fn txt_roundtrip() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "Hello world").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "Second paragraph").unwrap();

        let units = open_txt(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 2);
    }
}