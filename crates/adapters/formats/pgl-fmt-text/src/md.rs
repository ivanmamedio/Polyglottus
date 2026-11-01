//! Markdown

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use comrak::{parse_document, Arena, ComrakOptions, nodes::NodeValue};
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Abre MD como SourceDocument
pub async fn open_md(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let arena = Arena::new();
    let root = parse_document(&arena, &content, &ComrakOptions::default());

    let mut units = Vec::new();
    let section = SectionId::new();
    let mut idx = 0;

    // Walk AST
    fn walk<'a>(node: &'a comrak::nodes::AstNode<'a>, units: &mut Vec<Unit>, section: SectionId, idx: &mut usize, file_name: &str) {
        match &node.data.borrow().value {
            NodeValue::Paragraph => {
                let text = node_to_text(node);
                if !text.trim().is_empty() {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("paragraph[{}]", idx),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }
            }
            NodeValue::Heading(heading) => {
                let text = node_to_text(node);
                if !text.trim().is_empty() {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("heading[{}]", idx),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, UnitKind::Heading(heading.level as u8), MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }
            }
            NodeValue::CodeBlock(_) => {
                let text = node_to_text(node);
                if !text.trim().is_empty() {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("codeblock[{}]", idx),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, UnitKind::CodeBlock, MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }
            }
            NodeValue::ListItem => {
                let text = node_to_text(node);
                if !text.trim().is_empty() {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("listitem[{}]", idx),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, UnitKind::ListItem, MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }
            }
            _ => {}
        }

        for child in node.children() {
            walk(child, units, section, idx, file_name);
        }
    }

    fn node_to_text(node: &comrak::nodes::AstNode) -> String {
        let mut text = String::new();
        fn collect(node: &comrak::nodes::AstNode, text: &mut String) {
            match &node.data.borrow().value {
                NodeValue::Text(content) | NodeValue::Code(content) => text.push_str(content),
                NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
                _ => {}
            }
            for child in node.children() {
                collect(child, text);
            }
        }
        collect(node, &mut text);
        text
    }

    walk(root, &mut units, section, &mut idx, &path.file_name().unwrap_or_default().to_string_lossy());

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de headings, parágrafos, listas")
        .with_preserved("Blocos de código (não traduzidos)")
        .with_preserved("Links e imagens (URLs preservadas)")
        .with_not_preserved("HTML raw")
        .with_warning("HTML inline não é traduzido");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve MD traduzido
pub async fn write_md(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let arena = Arena::new();
    let root = parse_document(&arena, &content, &ComrakOptions::default());

    // Rebuild with translations
    let mut output = String::new();
    fn rebuild(node: &comrak::nodes::AstNode, translations: &dyn TranslationLookup, output: &mut String, options: &ExportOptions, idx: &mut usize) {
        match &node.data.borrow().value {
            NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::CodeBlock(_) | NodeValue::ListItem => {
                let unit_id = UnitId::from_hash(&{
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    format!("{:?}", node.data.borrow().value).hash(&mut hasher);
                    idx.hash(&mut hasher);
                    hasher.finish().to_le_bytes()
                });

                if let Some(translated) = translations.get(&unit_id) {
                    output.push_str(&translated.raw);
                } else {
                    // Fallback: render original
                    comrak::format_commonmark(node, &mut output, &ComrakOptions::default()).ok();
                }
                *idx += 1;
            }
            _ => {
                comrak::format_commonmark(node, &mut output, &ComrakOptions::default()).ok();
            }
        }

        for child in node.children() {
            rebuild(child, translations, output, options, idx);
        }
    }

    let mut idx = 0;
    rebuild(root, translations, &mut output, options, &mut idx);

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::MD,
        units_processed: idx,
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

    #[tokio::test]
    async fn md_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# Title").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "Paragraph text").unwrap();

        let units = open_md(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert!(!units_vec.is_empty());
    }
}