//! Markdown

use uuid::Uuid;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef, MarkError};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use comrak::{parse_document, Arena, ComrakOptions, nodes::NodeValue};
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Internal function to rebuild markdown from AST with translations
fn rebuild_markdown(
    content: &str,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
) -> String {
    let arena = Arena::new();
    let root = parse_document(&arena, content, &ComrakOptions::default());
    
    let mut output = String::new();
    let mut idx = 0;

    fn rebuild<'a>(
        node: &'a comrak::nodes::AstNode<'a>,
        translations: &dyn TranslationLookup,
        output: &mut String,
        options: &ExportOptions,
        idx: &mut usize,
    ) {
        match &node.data.borrow().value {
            NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::CodeBlock(_) | NodeValue::Item(_) => {
                let unit_id = UnitId::from_hash({
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    format!("{:?}", node.data.borrow().value).hash(&mut hasher);
                    idx.hash(&mut hasher);
                    hasher.finish()
                });

                if let Some(translated) = translations.get(&unit_id) {
                    output.push_str(&translated.raw);
                } else {
                    let mut output_buf = Vec::new();
                    comrak::format_commonmark(node, &ComrakOptions::default(), &mut output_buf).ok();
                    output.push_str(String::from_utf8_lossy(&output_buf).as_ref());
                }
                *idx += 1;
            }
            _ => {
                let mut output_buf = Vec::new();
                comrak::format_commonmark(node, &ComrakOptions::default(), &mut output_buf).ok();
                output.push_str(String::from_utf8_lossy(&output_buf).as_ref());
            }
        }

        for child in node.children() {
            rebuild(child, translations, output, options, idx);
        }
    }

    rebuild(&root, translations, &mut output, options, &mut idx);
    output
}

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


/// Abre MD como SourceDocument
pub async fn open_md(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let arena = Arena::new();
    let root = parse_document(&arena, &content, &ComrakOptions::default());

    let mut units = Vec::new();
    let section = SectionId::new();
    let mut idx = 0;

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
                    let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(&text), section, Uuid::nil());
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
                    let mut unit = Unit::new(source_ref, UnitKind::Heading(heading.level as u8), MarkedText::new(&text), section, Uuid::nil());
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
                    let mut unit = Unit::new(source_ref, UnitKind::CodeBlock, MarkedText::new(&text), section, Uuid::nil());
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }
            }
            NodeValue::Item(_) => {
                let text = node_to_text(node);
                if !text.trim().is_empty() {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("listitem[{}]", idx),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, UnitKind::ListItem, MarkedText::new(&text), section, Uuid::nil());
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

    fn node_to_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
        let mut text = String::new();
        fn collect<'a>(node: &'a comrak::nodes::AstNode<'a>, text: &mut String) {
            match &node.data.borrow().value {
                NodeValue::Text(content) => text.push_str(content),
                NodeValue::Code(content) => text.push_str(&content.literal),
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

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

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

    // Rebuild markdown - parse and rebuild in one synchronous function
    // to avoid holding RefCell across await points
    let output = rebuild_markdown(&content, translations, options);

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::MD,
        units_processed: 0, // TODO: count actual units
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}